use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use polynotes_core::{WhisperContext, TranscribeOptions};
use webrtc_vad::{Vad, VadMode, SampleRate};

static RUNNING: OnceLock<Arc<AtomicBool>> = OnceLock::new();

fn get_flag() -> Arc<AtomicBool> {
    RUNNING.get_or_init(|| Arc::new(AtomicBool::new(false))).clone()
}

// Wrapper to explicitly allow dropping the cpal Stream safely from any thread
struct StreamWrapper(Option<cpal::Stream>);
unsafe impl Send for StreamWrapper {}
unsafe impl Sync for StreamWrapper {}

static STREAM_GUARD: Mutex<StreamWrapper> = Mutex::new(StreamWrapper(None));

static SAMPLE_BUFFER: std::sync::Mutex<Vec<i16>> = std::sync::Mutex::new(Vec::new());
static SPEECH_BUFFER: std::sync::Mutex<Vec<f32>> = std::sync::Mutex::new(Vec::new());

const GAIN_FACTOR: f32 = 6.0;
const TARGET_RATE: u32 = 16000;

/// Persisted whisper model path — set via `set_model_path` command from the frontend.
static MODEL_PATH: Mutex<String> = Mutex::new(String::new());

/// Compile-time default: resolved by Cargo from `.cargo/config.toml` using
/// `relative = true`, producing a clean absolute path to the bundled model.
/// Mirrors the approach used in `core/src/tests.rs`.
const DEFAULT_MODEL_PATH: &str = env!("WHISPER_MODEL_PATH");

// ── Model path commands ────────────────────────────────────────────────────

#[tauri::command]
fn set_model_path(path: String) {
    if let Ok(mut guard) = MODEL_PATH.lock() {
        *guard = path;
    }
}

#[tauri::command]
fn get_model_path() -> String {
    MODEL_PATH.lock().map(|g| g.clone()).unwrap_or_default()
}

#[derive(Clone, serde::Serialize)]
struct DownloadProgress {
    downloaded: u64,
    total: u64,
}

#[tauri::command]
async fn download_model(app: AppHandle) -> Result<String, String> {
    let url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";
    
    // Create the models directory in app_data_dir
    let app_data = app.path().app_data_dir().map_err(|e: tauri::Error| e.to_string())?;
    let models_dir = app_data.join("models");
    tokio::fs::create_dir_all(&models_dir).await.map_err(|e: std::io::Error| e.to_string())?;
    
    let dest_path = models_dir.join("ggml-base.bin");
    
    // Setup request
    let client = reqwest::Client::new();
    let res = client.get(url).send().await.map_err(|e| format!("Failed to connect: {}", e))?;
    let total_size = res.content_length().unwrap_or(141_000_000); // Base model is ~141MB
    
    let mut file = tokio::fs::File::create(&dest_path).await.map_err(|e: std::io::Error| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();
    
    use futures_util::StreamExt;
    
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e: reqwest::Error| format!("Error while downloading: {}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await.map_err(|e: std::io::Error| e.to_string())?;
        downloaded += chunk.len() as u64;
        
        // Emit progress to frontend
        let _ = app.emit("download_progress", DownloadProgress {
            downloaded,
            total: total_size
        });
    }
    
    // Automatically set it as the active model
    let path_str = dest_path.to_string_lossy().to_string();
    set_model_path(path_str.clone());
    
    Ok(path_str)
}

// ── Transcription commands ─────────────────────────────────────────────────

#[tauri::command]
async fn start_transcription(app: AppHandle, source: String) -> Result<(), String> {
    println!("[polynotes] start_transcription source={source}");
    let flag = get_flag();

    // Ensure we are truly stopped before restarting
    stop_transcription();
    // Brief sleep to let any existing threads exit and file handles release
    std::thread::sleep(std::time::Duration::from_millis(150));

    flag.store(true, Ordering::SeqCst);

    let host = cpal::default_host();
    let (device, config) = if source == "app-audio" {
        let dev = host.default_output_device().ok_or("No output device found")?;
        let cfg = dev.default_output_config().map_err(|e| format!("Failed to get loopback config: {}", e))?;
        (dev, cfg)
    } else {
        let dev = host.default_input_device().ok_or("No input device found")?;
        let cfg = dev.default_input_config().map_err(|e| format!("Failed to get input config: {}", e))?;
        (dev, cfg)
    };

    let sample_rate = config.sample_rate().0;
    // Prevent division by zero and chunks(0) panic
    let channels = (config.channels() as usize).max(1);

    let _vad_sample_rate = SampleRate::Rate16kHz; // We will always resample to 16kHz

    let _frame_size = (sample_rate as usize * 30) / 1000;
    let err_fn = |err| eprintln!("stream error: {}", err);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| {
                if let Ok(mut i16_buf) = SAMPLE_BUFFER.lock() {
                    for chunk in data.chunks(channels) {
                        let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                        let mut boosted = mono * GAIN_FACTOR;
                        if boosted.is_nan() { boosted = 0.0; } // Prevent NaN clamp panic
                        let sample_i16 = (boosted * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                        i16_buf.push(sample_i16);
                    }
                }
            },
            err_fn,
            None,
        ).map_err(|e| e.to_string())?,

        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &_| {
                if let Ok(mut i16_buf) = SAMPLE_BUFFER.lock() {
                    for chunk in data.chunks(channels) {
                        let mono_i32: i32 = chunk.iter().map(|&s| s as i32).sum::<i32>() / channels as i32;
                        let boosted = (mono_i32 as f32 * GAIN_FACTOR) as i32;
                        let mono_i16 = boosted.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                        i16_buf.push(mono_i16);
                    }
                }
            },
            err_fn,
            None,
        ).map_err(|e| e.to_string())?,

        _ => return Err("unsupported sample format".into()),
    };

    stream.play().map_err(|e| e.to_string())?;

    if let Ok(mut guard) = STREAM_GUARD.lock() {
        guard.0 = Some(stream);
    }

    // Snapshot the model path — fall back to the compile-time default (same location
    // the unit tests use) when the user hasn't configured one via Settings.
    let model_path: String = {
        let guard = MODEL_PATH.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_empty() {
            DEFAULT_MODEL_PATH.to_string()
        } else {
            guard.clone()
        }
    };

    let flag_clone = flag.clone();
    let app_clone = app.clone();

    std::thread::spawn(move || {
        let mut vad = Vad::new();
        vad.set_mode(VadMode::Quality); // Mode 0 is more lenient and better for general speech

        // Normalize path for Windows (C++ fopen can be picky about slashes)
        let normalized_path = model_path.replace('/', "\\");

        // Explicit check before passing to C++
        if !std::path::Path::new(&normalized_path).exists() {
             let err_msg = format!("Model file not found at '{}'. Please check your Settings.", normalized_path);
             eprintln!("[polynotes] {}", err_msg);
             let _ = app_clone.emit("transcription_error", err_msg);
             return;
        }

        // Load the whisper model from the user-configured path
        let whisper = match WhisperContext::new(&normalized_path) {
            Ok(ctx) => ctx,
            Err(e) => {
                eprintln!("[polynotes] Failed to load whisper model from '{}': {:?}", normalized_path, e);
                if let Err(emit_err) = app_clone.emit(
                    "transcription_error",
                    format!("Failed to load model from '{}'. (Error: {:?})", normalized_path, e),
                ) {
                    eprintln!("[polynotes] emit error: {emit_err}");
                }
                return;
            }
        };

        println!("[polynotes] thread: internal model loading finished. loop start.");
        let mut silence_frames = 0u32;
        let mut loop_iterations = 0u64;
        let mut total_samples_processed = 0u64;
        
        // 30ms frame size for 16kHz
        let target_frame_size = (TARGET_RATE as usize * 30) / 1000; // 480 samples
        // Flush to whisper after ~1 second of consecutive silence
        const SILENCE_FLUSH_THRESHOLD: u32 = 33;
        // Safety flush if speech buffer gets very large (~30 seconds)
        const MAX_SPEECH_SAMPLES: usize = 16000 * 30;

        loop {
            loop_iterations += 1;
            if !flag_clone.load(Ordering::SeqCst) {
                 println!("[polynotes] thread: loop exit condition met (flag is false after {} iterations)", loop_iterations);
                 break;
            }

            // Drain and resample as many 30ms frames as possible
            loop {
                // Prevent underflow by asserting native_needed is at least 1
                let native_needed = ((sample_rate as usize * 30) / 1000).max(1);
                
                let frame_ready = {
                    if let Ok(buf) = SAMPLE_BUFFER.lock() {
                        buf.len() >= native_needed
                    } else {
                        false
                    }
                };
                if !frame_ready { break; }

                let native_samples: Vec<i16> = {
                    if let Ok(mut buf) = SAMPLE_BUFFER.lock() {
                        buf.drain(..native_needed).collect()
                    } else {
                        vec![0; native_needed]
                    }
                };

                // Simple Linear Resampler: Native rate -> 16000Hz
                let mut resampled_i16 = Vec::with_capacity(target_frame_size);
                let mut resampled_f32 = Vec::with_capacity(target_frame_size);
                
                for i in 0..target_frame_size {
                    let pos = (i as f32 * native_needed as f32) / target_frame_size as f32;
                    let mut low = pos.floor() as usize;
                    if low >= native_needed { low = native_needed.saturating_sub(1); }
                    let high = (low + 1).min(native_needed.saturating_sub(1));
                    let weight = pos - low as f32;
                    
                    let s_low = native_samples[low] as f32;
                    let s_high = native_samples[high] as f32;
                    let val = s_low * (1.0 - weight) + s_high * weight;
                    
                    resampled_i16.push(val as i16);
                    resampled_f32.push(val / i16::MAX as f32);
                }

                let is_speech_vad = vad.is_voice_segment(&resampled_i16).unwrap_or(false);
                
                let sum_sq: f32 = resampled_f32.iter().map(|&x| x * x).sum();
                let rms = (sum_sq / resampled_f32.len() as f32).sqrt();
                
                // Hybrid detection: Use VAD but fallback to RMS if it's clearly loud
                // RMS 0.02 (after gain) is a reasonable threshold for "something is happening"
                let is_speech = is_speech_vad || rms > 0.02;

                total_samples_processed += native_needed as u64;

                if is_speech {
                    if silence_frames > 0 { 
                        let cause = if is_speech_vad { "VAD" } else { "RMS" };
                        println!("[polynotes] VAD: speech detected (via {}, RMS={:.4}, after {} silence frames)", cause, rms, silence_frames); 
                    }
                    silence_frames = 0;
                    if let Ok(mut sb) = SPEECH_BUFFER.lock() {
                        sb.extend_from_slice(&resampled_f32);
                    }
                } else {
                    silence_frames += 1;
                    if silence_frames % 50 == 0 {
                        let slen = SPEECH_BUFFER.lock().map(|b| b.len()).unwrap_or(0);
                        println!("[polynotes] VAD filtered frame. RMS={:.6} (speech_buf={} samples)", rms, slen);
                    }
                }
            }

            if loop_iterations % 66 == 0 {
                let s1 = SAMPLE_BUFFER.lock().map(|b| b.len()).unwrap_or(0);
                let s2 = SPEECH_BUFFER.lock().map(|b| b.len()).unwrap_or(0);
                println!("[polynotes] diag: native_buf={} speech_buf={} samples_total={}", s1, s2, total_samples_processed);
            }

            std::thread::sleep(std::time::Duration::from_millis(30));

            // Decide whether to flush accumulated speech to whisper
            let should_flush = {
                if let Ok(sb) = SPEECH_BUFFER.lock() {
                    let has_audio = !sb.is_empty();
                    let long_silence = silence_frames >= SILENCE_FLUSH_THRESHOLD;
                    let too_long = sb.len() >= MAX_SPEECH_SAMPLES;
                    has_audio && (long_silence || too_long)
                } else {
                    false
                }
            };

            if should_flush {
                silence_frames = 0;
                println!("[polynotes] flushing audio to whisper...");

                let audio: Vec<f32> = {
                    if let Ok(mut sb) = SPEECH_BUFFER.lock() {
                        std::mem::take(&mut *sb)
                    } else {
                        Vec::new()
                    }
                };

                let opts = TranscribeOptions::default();
                match whisper.transcribe_segments(&audio, opts) {
                    Ok(segments) => {
                        let samples = audio.len();
                        let seconds = samples as f32 / 16000.0;
                        println!("[polynotes] transcribing {:.2}s of audio ({} samples). found {} segments", seconds, samples, segments.len());

                        for seg in segments {
                            let text = seg.text.trim().to_string();
                            if text.is_empty() { continue; }

                            let prefix = if source == "app-audio" { "[App Audio]" } else { "[Mic]" };
                            let payload = format!("{} {}", prefix, text);

                            if let Err(e) = app_clone.emit("transcription_segment", payload) {
                                eprintln!("[polynotes] emit error: {e}");
                            }
                        }
                    }
                    Err(e) => eprintln!("[polynotes] transcription error: {e}"),
                }
            }
        }

        println!("[polynotes] transcription polling ended");
    });

    Ok(())
}

#[tauri::command]
fn stop_transcription() {
    get_flag().store(false, Ordering::SeqCst);
    if let Ok(mut guard) = STREAM_GUARD.lock() {
        if let Some(stream) = guard.0.take() {
            println!("[polynotes] dropping cpal stream");
            drop(stream);
        }
    }
    // Clear buffers to prevent stale audio or memory leaks
    if let Ok(mut buf) = SAMPLE_BUFFER.lock() { buf.clear(); }
    if let Ok(mut buf) = SPEECH_BUFFER.lock() { buf.clear(); }

    println!("[polynotes] stop_transcription called and buffers cleared");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            start_transcription,
            stop_transcription,
            set_model_path,
            get_model_path,
            download_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
