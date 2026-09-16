use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread;
use tauri::{AppHandle, Emitter};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use polynotes_core::{WhisperContext, TranscribeOptions};
use webrtc_vad::{Vad, VadMode, SampleRate};

use crate::models::current_model_path;

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
const FRAME_SIZE_MS: u32 = 10;

/// Heuristic cutoff below which a segment's average token confidence is
/// flagged for review. This is not validated against a labeled dataset —
/// see benchmarks.md's "Accuracy/quality benchmarking — out of scope" note.
const LOW_CONFIDENCE_THRESHOLD: f32 = 0.55;

fn get_dynamic_buffer_size() -> usize {
    std::env::var("POLYNOTES_BUFFER_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60) * TARGET_RATE as usize
}

#[derive(Clone, serde::Serialize)]
struct TranscriptSegmentPayload {
    text: String,
    source: String,
    confidence: Option<f32>,
    is_low_confidence: bool,
    language: Option<String>,
}

#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    source: String,
    detect_language: Option<bool>,
    extract_confidence: Option<bool>,
) -> Result<(), String> {
    // App-level defaults (deliberately different from TranscribeOptions::default(),
    // which core/benchmark.rs keep at false/false to preserve the documented
    // realtime numbers): confidence extraction is cheap, so it's on by default;
    // language detection has a measured cost, so it stays opt-in.
    let detect_language = detect_language.unwrap_or(false);
    let extract_confidence = extract_confidence.unwrap_or(true);

    println!("[polynotes] start_transcription source={source} detect_language={detect_language} extract_confidence={extract_confidence}");
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

    let flag_clone = flag.clone();
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
    let model_path2 = current_model_path();

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

        let opts = TranscribeOptions {
            detect_language,
            extract_confidence,
            ..TranscribeOptions::default()
        };

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

                                let is_low_confidence = seg
                                    .avg_confidence
                                    .map(|c| c < LOW_CONFIDENCE_THRESHOLD)
                                    .unwrap_or(false);

                                let payload = TranscriptSegmentPayload {
                                    text,
                                    source: source_clone2.clone(),
                                    confidence: seg.avg_confidence,
                                    is_low_confidence,
                                    language: seg.language,
                                };

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
pub fn stop_transcription() {
    get_flag().store(false, Ordering::SeqCst);
    if let Ok(mut guard) = STREAM_GUARD.lock() {
        if let Some(stream) = guard.0.take() {
            println!("[polynotes] dropping cpal stream");
            drop(stream);
        }
    }

    println!("[polynotes] stop_transcription called");
}
