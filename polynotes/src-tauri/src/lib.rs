use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use polynotes_core::{WhisperContext, TranscribeOptions};
use webrtc_vad::{Vad, VadMode, SampleRate};

static RUNNING: OnceLock<Arc<AtomicBool>> = OnceLock::new();

fn get_flag() -> Arc<AtomicBool> {
    RUNNING.get_or_init(|| Arc::new(AtomicBool::new(false))).clone()
}

struct StreamWrapper(Option<cpal::Stream>);
unsafe impl Send for StreamWrapper {}
unsafe impl Sync for StreamWrapper {}

static STREAM_GUARD: Mutex<StreamWrapper> = Mutex::new(StreamWrapper(None));

const GAIN_FACTOR: f32 = 6.0;
const TARGET_RATE: u32 = 16000;

const BATCH_FRAMES: usize = 10;
const FRAME_SIZE_MS: u32 = 30;

static MODEL_PATH: Mutex<String> = Mutex::new(String::new());
const DEFAULT_MODEL_PATH: &str = env!("WHISPER_MODEL_PATH");

fn get_dynamic_buffer_size() -> usize {
    std::env::var("POLYNOTES_BUFFER_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60) * TARGET_RATE as usize
}

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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u32,
    pub url: String,
    pub quantization: String,
    pub description: String,
}

fn get_available_models_list() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "tiny-q5_1".to_string(),
            name: "Tiny (Quantized)".to_string(),
            size_mb: 31,
            url: "ggml-tiny-q5_1.bin".to_string(),
            quantization: "q5_1".to_string(),
            description: "Fastest, lowest accuracy. Good for testing.".to_string(),
        },
        ModelInfo {
            id: "tiny".to_string(),
            name: "Tiny".to_string(),
            size_mb: 75,
            url: "ggml-tiny.bin".to_string(),
            quantization: "none".to_string(),
            description: "Fast, lower accuracy. Good for testing.".to_string(),
        },
        ModelInfo {
            id: "base-q5_1".to_string(),
            name: "Base (Quantized)".to_string(),
            size_mb: 57,
            url: "ggml-base-q5_1.bin".to_string(),
            quantization: "q5_1".to_string(),
            description: "Recommended: Balanced speed and accuracy.".to_string(),
        },
        ModelInfo {
            id: "base".to_string(),
            name: "Base".to_string(),
            size_mb: 142,
            url: "ggml-base.bin".to_string(),
            quantization: "none".to_string(),
            description: "Standard base model. Higher accuracy, slower.".to_string(),
        },
        ModelInfo {
            id: "small-q5_1".to_string(),
            name: "Small (Quantized)".to_string(),
            size_mb: 181,
            url: "ggml-small-q5_1.bin".to_string(),
            quantization: "q5_1".to_string(),
            description: "Higher accuracy, requires more resources.".to_string(),
        },
        ModelInfo {
            id: "small".to_string(),
            name: "Small".to_string(),
            size_mb: 466,
            url: "ggml-small.bin".to_string(),
            quantization: "none".to_string(),
            description: "Highest accuracy, requires good hardware.".to_string(),
        },
    ]
}

#[tauri::command]
fn get_available_models() -> Vec<ModelInfo> {
    get_available_models_list()
}

#[tauri::command]
async fn download_model(app: AppHandle, model_id: String) -> Result<String, String> {
    // Find the model in our list
    let models = get_available_models_list();
    let model = models
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Unknown model: {}", model_id))?;
    
    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        model.url
    );
    
    let app_data = app.path().app_data_dir().map_err(|e: tauri::Error| e.to_string())?;
    let models_dir = app_data.join("models");
    tokio::fs::create_dir_all(&models_dir).await.map_err(|e: std::io::Error| e.to_string())?;
    
    let dest_path = models_dir.join(&model.url);
    
    // Check if already downloaded
    if dest_path.exists() {
        let path_str = dest_path.to_string_lossy().to_string();
        set_model_path(path_str.clone());
        return Ok(path_str);
    }
    
    let client = reqwest::Client::new();
    let res = client.get(&url).send().await.map_err(|e| format!("Failed to connect: {}", e))?;
    let total_size = res.content_length().unwrap_or((model.size_mb as u64) * 1_000_000);
    
    let mut file = tokio::fs::File::create(&dest_path).await.map_err(|e: std::io::Error| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();
    
    use futures_util::StreamExt;
    
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e: reqwest::Error| format!("Error while downloading: {}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await.map_err(|e: std::io::Error| e.to_string())?;
        downloaded += chunk.len() as u64;
        
        let _ = app.emit("download_progress", DownloadProgress {
            downloaded,
            total: total_size
        });
    }
    
    let path_str = dest_path.to_string_lossy().to_string();
    set_model_path(path_str.clone());
    
    Ok(path_str)
}

#[tauri::command]
async fn start_transcription(app: AppHandle, source: String) -> Result<(), String> {
    println!("[polynotes] start_transcription source={source}");
    let flag = get_flag();

    stop_transcription();
    thread::sleep(std::time::Duration::from_millis(150));

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
    let channels = (config.channels() as usize).max(1);

    let _vad_sample_rate = SampleRate::Rate16kHz;

    let sample_buffer: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::with_capacity(get_dynamic_buffer_size())));
    let speech_buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(get_dynamic_buffer_size())));

    let sample_buffer_clone = sample_buffer.clone();

    let (transcription_tx, transcription_rx) = mpsc::channel::<Vec<f32>>();

    let err_fn = |err| eprintln!("stream error: {}", err);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| {
                let mut buf = match sample_buffer_clone.lock() {
                    Ok(b) => b,
                    Err(_) => return,
                };
                for chunk in data.chunks(channels) {
                    let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                    let mut boosted = mono * GAIN_FACTOR;
                    if boosted.is_nan() { boosted = 0.0; }
                    let sample_i16 = (boosted * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                    buf.push(sample_i16);
                }
            },
            err_fn,
            None,
        ).map_err(|e| e.to_string())?,

        cpal::SampleFormat::I16 => {
            let sample_buffer_i16 = sample_buffer.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| {
                    let mut buf = match sample_buffer_i16.lock() {
                        Ok(b) => b,
                        Err(_) => return,
                    };
                    for chunk in data.chunks(channels) {
                        let mono_i32: i32 = chunk.iter().map(|&s| s as i32).sum::<i32>() / channels as i32;
                        let boosted = (mono_i32 as f32 * GAIN_FACTOR) as i32;
                        let mono_i16 = boosted.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                        buf.push(mono_i16);
                    }
                },
                err_fn,
                None,
            ).map_err(|e| e.to_string())?
        },

        _ => return Err("unsupported sample format".into()),
    };

    stream.play().map_err(|e| e.to_string())?;

    if let Ok(mut guard) = STREAM_GUARD.lock() {
        guard.0 = Some(stream);
    }

    let _model_path: String = {
        let guard = MODEL_PATH.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_empty() {
            DEFAULT_MODEL_PATH.to_string()
        } else {
            guard.clone()
        }
    };

    let flag_clone = flag.clone();
    let _app_clone = app.clone();
    let _source_clone = source.clone();
    let sample_buf_clone = sample_buffer.clone();
    let speech_buf_clone = speech_buffer.clone();

    thread::spawn(move || {
        println!("[polynotes] processing thread: starting.");

        let target_frame_size = (TARGET_RATE as usize * FRAME_SIZE_MS as usize) / 1000;
        let native_frame_size = ((sample_rate as usize * FRAME_SIZE_MS as usize) / 1000).max(1);
        
        let silence_threshold = 33usize; // 1 second - better for accuracy
        let max_speech_samples = 16000 * 30;

        let mut silence_frames: usize = 0;

        let mut vad = Vad::new();
        let _ = vad.set_mode(VadMode::Aggressive);

        loop {
            if !flag_clone.load(Ordering::SeqCst) {
                println!("[polynotes] processing loop: exit signal received");
                break;
            }

            let available = {
                match sample_buf_clone.lock() {
                    Ok(buf) => buf.len(),
                    Err(_) => {
                        thread::sleep(std::time::Duration::from_millis(10));
                        continue;
                    }
                }
            };

            if available < native_frame_size {
                thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }

            let frames_to_process = (available / native_frame_size).min(BATCH_FRAMES);
            let total_native_samples = (frames_to_process * native_frame_size).min(available);

            let native_batch: Vec<i16> = {
                match sample_buf_clone.lock() {
                    Ok(mut buf) => buf.drain(..total_native_samples).collect(),
                    Err(_) => {
                        continue;
                    }
                }
            };

            if native_batch.len() < native_frame_size {
                continue;
            }

            let mut filtered: Vec<i16> = Vec::with_capacity(native_batch.len());
            for i in 0..native_batch.len() {
                let prev = if i > 0 { native_batch[i - 1] } else { native_batch[0] };
                let curr = native_batch[i];
                let next = if i < native_batch.len() - 1 { native_batch[i + 1] } else { native_batch[i] };
                let avg = ((prev as i32 + curr as i32 + next as i32) / 3) as i16;
                filtered.push(avg);
            }

            for frame_idx in 0..frames_to_process {
                let frame_start = frame_idx * native_frame_size;
                let frame_end = (frame_start + native_frame_size).min(filtered.len());
                if frame_start >= filtered.len() { break; }
                let frame_slice = &filtered[frame_start..frame_end];
                
                let mut resampled_frame: Vec<f32> = Vec::with_capacity(target_frame_size);
                
                for i in 0..target_frame_size {
                    let pos = (i as f32 * frame_slice.len() as f32) / target_frame_size as f32;
                    let mut low = pos.floor() as usize;
                    if low >= frame_slice.len() { low = frame_slice.len().saturating_sub(1); }
                    let high = (low + 1).min(frame_slice.len().saturating_sub(1));
                    let weight = pos - low as f32;
                    
                    let s_low = frame_slice[low] as f32;
                    let s_high = frame_slice[high] as f32;
                    let val = s_low * (1.0 - weight) + s_high * weight;
                    
                    resampled_frame.push(val / i16::MAX as f32);
                }
                
                let vad_input: Vec<i16> = resampled_frame.iter().map(|&f| (f * i16::MAX as f32) as i16).collect();
                let is_speech_vad = vad.is_voice_segment(&vad_input).unwrap_or(false);
                
                let sum_sq: f32 = resampled_frame.iter().map(|&x| x * x).sum();
                let rms = (sum_sq / resampled_frame.len() as f32).sqrt();
                let is_speech = is_speech_vad || rms > 0.02;

                if is_speech {
                    silence_frames = 0;
                    if let Ok(mut sb) = speech_buf_clone.lock() {
                        sb.extend_from_slice(&resampled_frame);
                    }
                } else {
                    silence_frames += 1;
                }
            }

            let speech_len = {
                match speech_buf_clone.lock() {
                    Ok(sb) => sb.len(),
                    Err(_) => 0,
                }
            };

            if speech_len >= max_speech_samples || (silence_frames >= silence_threshold && speech_len > 0) {
                if speech_len > 0 {
                    let audio: Vec<f32> = {
                        match speech_buf_clone.lock() {
                            Ok(mut sb) => std::mem::take(&mut *sb),
                            Err(_) => Vec::new(),
                        }
                    };
                    silence_frames = 0;

                    if !audio.is_empty() {
                        if let Err(e) = transcription_tx.send(audio) {
                            eprintln!("[polynotes] failed to send to transcription: {:?}", e);
                        }
                    }
                }
            }
        }

        println!("[polynotes] processing loop ended");
    });

    let flag_clone2 = flag.clone();
    let app_clone2 = app.clone();
    let source_clone2 = source.clone();

    let model_path2 = {
        let guard = MODEL_PATH.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_empty() {
            DEFAULT_MODEL_PATH.to_string()
        } else {
            guard.clone()
        }
    };

    thread::spawn(move || {
        let normalized_path = model_path2.replace('/', "\\");
        
        // Validate model file exists before loading
        if !std::path::Path::new(&normalized_path).exists() {
            let err_msg = format!("Model file not found at '{}'. Please check your Settings.", normalized_path);
            eprintln!("[polynotes] {}", err_msg);
            let _ = app_clone2.emit("transcription_error", err_msg);
            return;
        }
        
        println!("[polynotes] transcription thread: loading whisper model...");
        
        let whisper = match WhisperContext::new(&normalized_path) {
            Ok(ctx) => ctx,
            Err(e) => {
                eprintln!("[polynotes] transcription thread: failed to load whisper: {:?}", e);
                let _ = app_clone2.emit("transcription_error", format!("Failed to load model: {:?}", e));
                return;
            }
        };
        
        println!("[polynotes] transcription thread: model loaded, ready.");
        
        let opts = TranscribeOptions::default();
        
        while flag_clone2.load(Ordering::SeqCst) {
            match transcription_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(audio) => {
                    // Check if we should still be running before processing
                    if !flag_clone2.load(Ordering::SeqCst) {
                        break;
                    }
                    
                    println!("[polynotes] transcribing {:.2}s of audio", audio.len() as f32 / 16000.0);
                    
                    match whisper.transcribe_segments(&audio, opts.clone()) {
                        Ok(segments) => {
                            for seg in segments {
                                let text = seg.text.trim().to_string();
                                if text.is_empty() { continue; }

                                // Double-check flag before emit
                                if !flag_clone2.load(Ordering::SeqCst) {
                                    break;
                                }

                                let prefix = if source_clone2 == "app-audio" { "[App Audio]" } else { "[Mic]" };
                                let payload = format!("{} {}", prefix, text);

                                if let Err(e) = app_clone2.emit("transcription_segment", payload) {
                                    // Don't error log on emit failures during shutdown
                                    if flag_clone2.load(Ordering::SeqCst) {
                                        eprintln!("[polynotes] emit error: {:?}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            if flag_clone2.load(Ordering::SeqCst) {
                                eprintln!("[polynotes] transcription error: {:?}", e);
                            }
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    println!("[polynotes] transcription channel disconnected");
                    break;
                }
            }
        }
        
        println!("[polynotes] transcription thread ended");
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

    println!("[polynotes] stop_transcription called");
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
            get_available_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
