use polynotes_core::{TranscribeOptions, WhisperContext};
use std::env;
use std::io::Write;
use std::time::Instant;

const SAMPLE_RATE: u32 = 16000;
const TEST_DURATION_SECS: f64 = 30.0;
const E2E_TEST_DURATION_SECS: f64 = 10.0;
const CHUNK_SIZE_MS: u32 = 300;

#[derive(Debug, Clone)]
struct ModelInfo {
    name: String,
    path: String,
    expected_speed: String,
}

#[derive(Debug)]
struct BenchmarkMetrics {
    model_name: String,
    inference_time_secs: f64,
    realtime_factor: f64,
    expected_speed: String,
}

#[derive(Debug)]
struct E2EBenchmarkMetrics {
    model_name: String,
    chunk_latency_ms: f64,
    total_time_secs: f64,
    realtime_factor: f64,
    throughput_chunks_per_sec: f64,
    chunks_processed: usize,
    expected_speed: String,
}

fn generate_synthetic_speech_audio(duration_secs: f64, sample_rate: u32) -> Vec<f32> {
    let num_samples = (sample_rate as f64 * duration_secs) as usize;
    let mut audio = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f64 / sample_rate as f64;

        let f0 = 120.0 + 50.0 * (t * 0.5).sin();
        let f1 = 500.0 + 100.0 * (t * 2.0).sin();
        let f2 = 1500.0 + 200.0 * (t * 1.5).sin();
        let f3 = 2500.0 + 150.0 * (t * 3.0).sin();

        let syllable_rate = 4.0;
        let amplitude = 0.3 + 0.2 * (t * syllable_rate * std::f64::consts::PI).sin();

        let sample = amplitude
            * (0.5 * (2.0 * std::f64::consts::PI * f0 * t).sin()
                + 0.3 * (2.0 * std::f64::consts::PI * f1 * t).sin()
                + 0.15 * (2.0 * std::f64::consts::PI * f2 * t).sin()
                + 0.05 * (2.0 * std::f64::consts::PI * f3 * t).sin());

        let noise = (i as f64 * 0.1).sin() * 0.02;
        let sample = (sample + noise).clamp(-1.0, 1.0);

        audio.push(sample as f32);
    }

    audio
}

fn find_available_models() -> Vec<ModelInfo> {
    let base_path = "core/whisper.cpp/models";
    let mut models = Vec::new();

    let candidates = vec![
        ("tiny.en-q5_1", "ggml-tiny.en-q5_1.bin", "30 MB", "English"),
        ("base.en-q5_1", "ggml-base.en-q5_1.bin", "76 MB", "English"),
        ("tiny-q5_1", "ggml-tiny-q5_1.bin", "32 MB", "Multi"),
        ("base-q5_1", "ggml-base-q5_1.bin", "60 MB", "Multi"),
    ];

    for (name, filename, size, lang) in candidates {
        let full_path = format!("{}/{}", base_path, filename);
        if std::path::Path::new(&full_path).exists() {
            models.push(ModelInfo {
                name: name.to_string(),
                path: full_path,
                expected_speed: format!("{} ({})", size, lang),
            });
        }
    }

    models
}

fn run_benchmark_for_model(model: &ModelInfo, audio: &[f32]) -> BenchmarkMetrics {
    let whisper = match WhisperContext::new(&model.path) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("✗ Failed to load model {}: {:?}", model.name, e);
            return BenchmarkMetrics {
                model_name: model.name.clone(),
                inference_time_secs: 0.0,
                realtime_factor: 0.0,
                expected_speed: model.expected_speed.clone(),
            };
        }
    };

    let opts = TranscribeOptions::default();

    let start = Instant::now();
    let result = whisper.transcribe_segments(audio, opts);
    let elapsed = start.elapsed();

    let time_secs = elapsed.as_secs_f64();
    let realtime_factor = if time_secs > 0.0 {
        TEST_DURATION_SECS / time_secs
    } else {
        0.0
    };

    if let Err(e) = result {
        eprintln!("⚠ Transcription error: {:?}", e);
    }

    BenchmarkMetrics {
        model_name: model.name.clone(),
        inference_time_secs: time_secs,
        realtime_factor,
        expected_speed: model.expected_speed.clone(),
    }
}

fn run_e2e_benchmark_for_model(model: &ModelInfo, audio: &[f32]) -> E2EBenchmarkMetrics {
    let whisper = match WhisperContext::new(&model.path) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("✗ Failed to load model {}: {:?}", model.name, e);
            return E2EBenchmarkMetrics {
                model_name: model.name.clone(),
                chunk_latency_ms: 0.0,
                total_time_secs: 0.0,
                realtime_factor: 0.0,
                throughput_chunks_per_sec: 0.0,
                chunks_processed: 0,
                expected_speed: model.expected_speed.clone(),
            };
        }
    };

    let opts = TranscribeOptions::default();

    let chunk_samples = (SAMPLE_RATE as f64 * CHUNK_SIZE_MS as f64 / 1000.0) as usize;
    let total_chunks = audio.len() / chunk_samples;

    let mut total_latency_ms: f64 = 0.0;
    let mut chunks_processed = 0;

    let overall_start = Instant::now();

    for chunk_idx in 0..total_chunks {
        let start = chunk_idx * chunk_samples;
        let end = (start + chunk_samples).min(audio.len());
        let chunk = &audio[start..end];

        if chunk.is_empty() {
            continue;
        }

        let chunk_start = Instant::now();
        let _result = whisper.transcribe_segments(chunk, opts.clone());
        let chunk_elapsed = chunk_start.elapsed();

        total_latency_ms += chunk_elapsed.as_secs_f64() * 1000.0;
        chunks_processed += 1;
    }

    let total_time = overall_start.elapsed();
    let total_time_secs = total_time.as_secs_f64();

    let avg_chunk_latency_ms = if chunks_processed > 0 {
        total_latency_ms / chunks_processed as f64
    } else {
        0.0
    };

    let throughput = if total_time_secs > 0.0 {
        chunks_processed as f64 / total_time_secs
    } else {
        0.0
    };

    let realtime_factor = if total_time_secs > 0.0 {
        TEST_DURATION_SECS / total_time_secs
    } else {
        0.0
    };

    E2EBenchmarkMetrics {
        model_name: model.name.clone(),
        chunk_latency_ms: avg_chunk_latency_ms,
        total_time_secs,
        realtime_factor,
        throughput_chunks_per_sec: throughput,
        chunks_processed,
        expected_speed: model.expected_speed.clone(),
    }
}

fn print_banner(mode: &str) {
    println!();
    if mode == "e2e" {
        println!(
            "╔═══════════════════════════════════════════════════════════════════════════════╗"
        );
        println!("║              POLYNOTES END-TO-END LATENCY BENCHMARK                     ║");
        println!("║                    Real-time Streaming Test                             ║");
        println!(
            "╚═══════════════════════════════════════════════════════════════════════════════╝"
        );
    } else {
        println!(
            "╔═══════════════════════════════════════════════════════════════════════════════╗"
        );
        println!("║                    POLYNOTES WHISPER BENCHMARK                           ║");
        println!("║                         Performance Analysis                             ║");
        println!(
            "╚═══════════════════════════════════════════════════════════════════════════════╝"
        );
    }
    println!();
}

fn print_results_table(results: &[BenchmarkMetrics]) {
    let cpu_cores = num_cpus::get_physical();
    let header = "  ┌─────────────────┬────────────┬────────────────┬───────────┬─────────┐";
    let separator = "  ├─────────────────┼────────────┼────────────────┼───────────┼─────────┤";
    let footer = "  └─────────────────┴────────────┴────────────────┴───────────┴─────────┘";

    println!(
        "  CPU: AMD Ryzen 5 5600H ({} cores) | Test Duration: {:.0}s",
        cpu_cores, TEST_DURATION_SECS
    );
    println!();
    println!("{}", header);
    println!(
        "  │ {:^15} │ {:^10} │ {:^14} │ {:^9} │ {:^7} │",
        "Model", "Time", "Realtime", "Size", "Type"
    );
    println!("{}", separator);

    for r in results {
        if r.inference_time_secs > 0.0 {
            let rt_factor = r.realtime_factor;
            let status = if rt_factor >= 1.0 { "✓" } else { "✗" };
            println!(
                "  │ {:^15} │ {:^10.2}s │ {:^11.1}x {:^2} │ {:^9} │ {:^7} │",
                r.model_name,
                r.inference_time_secs,
                rt_factor,
                status,
                r.expected_speed
                    .split('(')
                    .next()
                    .unwrap_or(&r.expected_speed),
                r.expected_speed
                    .split('(')
                    .nth(1)
                    .map(|s| s.replace(")", ""))
                    .unwrap_or_default()
            );
        } else {
            println!(
                "  │ {:^15} │ {:^10} │ {:^14} │ {:^9} │ {:^7} │",
                r.model_name,
                "FAILED",
                "-",
                r.expected_speed
                    .split('(')
                    .next()
                    .unwrap_or(&r.expected_speed),
                r.expected_speed
                    .split('(')
                    .nth(1)
                    .map(|s| s.replace(")", ""))
                    .unwrap_or_default()
            );
        }
    }
    println!("{}", footer);
    println!();
}

fn print_e2e_results_table(results: &[E2EBenchmarkMetrics]) {
    let cpu_cores = num_cpus::get_physical();
    let header = "  ┌─────────────────┬──────────────┬────────────┬────────────┬─────────────┐";
    let separator = "  ├─────────────────┼──────────────┼────────────┼────────────┼─────────────┤";
    let footer = "  └─────────────────┴──────────────┴────────────┴────────────┴─────────────┘";

    println!(
        "  CPU: AMD Ryzen 5 5600H ({} cores) | Audio: {:.0}s | Chunk: {}ms",
        cpu_cores, E2E_TEST_DURATION_SECS, CHUNK_SIZE_MS
    );
    println!();
    println!("{}", header);
    println!(
        "  │ {:^15} │ {:^12} │ {:^10} │ {:^10} │ {:^11} │",
        "Model", "Chunk Latency", "Total Time", "Realtime", "Throughput"
    );
    println!("{}", separator);

    for r in results {
        if r.total_time_secs > 0.0 {
            let rt_factor = r.realtime_factor;
            let status = if rt_factor >= 1.0 { "✓" } else { "✗" };
            println!(
                "  │ {:^15} │ {:^10.0}ms │ {:^8.2}s │ {:^8.1}x {:^2} │ {:^9.1}/s  │",
                r.model_name,
                r.chunk_latency_ms,
                r.total_time_secs,
                rt_factor,
                status,
                r.throughput_chunks_per_sec
            );
        } else {
            println!(
                "  │ {:^15} │ {:^12} │ {:^10} │ {:^10} │ {:^11} │",
                r.model_name, "FAILED", "-", "-", "-"
            );
        }
    }
    println!("{}", footer);
    println!();
}

fn print_summary(results: &[BenchmarkMetrics]) {
    let successful: Vec<_> = results.iter().filter(|r| r.realtime_factor > 0.0).collect();

    if !successful.is_empty() {
        let best = successful
            .iter()
            .max_by(|a, b| a.realtime_factor.partial_cmp(&b.realtime_factor).unwrap())
            .unwrap();

        println!(
            "  ╔═══════════════════════════════════════════════════════════════════════════════╗"
        );
        println!(
            "  ║  BEST PERFORMANCE                                                            ║"
        );
        println!(
            "  ╠═══════════════════════════════════════════════════════════════════════════════╣"
        );
        println!(
            "  ║  Model:        {}                                                          ║",
            best.model_name
        );
        println!(
            "  ║  Inference:    {:.2}s                                                       ║",
            best.inference_time_secs
        );
        println!(
            "  ║  Realtime:     {:.1}x                                                       ║",
            best.realtime_factor
        );
        println!(
            "  ║  Speedup:      {:.1}x faster than real-time                                 ║",
            best.realtime_factor
        );
        println!(
            "  ╚═══════════════════════════════════════════════════════════════════════════════╝"
        );
        println!();

        let all_realtime = successful.iter().all(|r| r.realtime_factor >= 1.0);
        if all_realtime {
            println!("  ✓ All models achieved realtime performance (>1x)");
        } else {
            println!("  ⚠ Some models did not achieve realtime performance");
        }
    }
    println!();

    println!("  ╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("  ║  BENCHMARK SUMMARY                                                           ║");
    println!("  ╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "  ║  Test Duration:  {:.0}s                                                       ║",
        TEST_DURATION_SECS
    );
    println!(
        "  ║  CPU Cores:      {}                                                          ║",
        num_cpus::get_physical()
    );
    println!(
        "  ║  Models Tested:  {}                                                          ║",
        successful.len()
    );
    if !successful.is_empty() {
        let avg_factor: f64 =
            successful.iter().map(|r| r.realtime_factor).sum::<f64>() / successful.len() as f64;
        println!(
            "  ║  Avg Realtime:    {:.1}x                                                       ║",
            avg_factor
        );
    }
    println!("  ╚═══════════════════════════════════════════════════════════════════════════════╝");
    println!();
}

fn print_e2e_summary(results: &[E2EBenchmarkMetrics]) {
    let successful: Vec<_> = results.iter().filter(|r| r.total_time_secs > 0.0).collect();

    if !successful.is_empty() {
        let best = successful
            .iter()
            .max_by(|a, b| a.realtime_factor.partial_cmp(&b.realtime_factor).unwrap())
            .unwrap();

        println!(
            "  ╔═══════════════════════════════════════════════════════════════════════════════╗"
        );
        println!(
            "  ║  BEST E2E PERFORMANCE                                                        ║"
        );
        println!(
            "  ╠═══════════════════════════════════════════════════════════════════════════════╣"
        );
        println!(
            "  ║  Model:          {}                                                          ║",
            best.model_name
        );
        println!(
            "  ║  Chunk Latency:  {:.0}ms                                                     ║",
            best.chunk_latency_ms
        );
        println!(
            "  ║  Total Time:     {:.2}s                                                       ║",
            best.total_time_secs
        );
        println!(
            "  ║  Realtime:       {:.1}x                                                       ║",
            best.realtime_factor
        );
        println!(
            "  ║  Throughput:     {:.1} chunks/s                                               ║",
            best.throughput_chunks_per_sec
        );
        println!(
            "  ╚═══════════════════════════════════════════════════════════════════════════════╝"
        );
        println!();

        let all_realtime = successful.iter().all(|r| r.realtime_factor >= 1.0);
        if all_realtime {
            println!("  ✓ All models achieved realtime performance (>1x)");
        } else {
            println!("  ⚠ Some models did not achieve realtime performance");
        }
    }
    println!();

    println!("  ╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("  ║  E2E BENCHMARK SUMMARY                                                       ║");
    println!("  ╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "  ║  Audio Duration: {:.0}s                                                       ║",
        E2E_TEST_DURATION_SECS
    );
    println!(
        "  ║  Chunk Size:     {}ms                                                         ║",
        CHUNK_SIZE_MS
    );
    println!(
        "  ║  CPU Cores:      {}                                                          ║",
        num_cpus::get_physical()
    );
    println!(
        "  ║  Models Tested:  {}                                                          ║",
        successful.len()
    );
    if !successful.is_empty() {
        let avg_factor: f64 =
            successful.iter().map(|r| r.realtime_factor).sum::<f64>() / successful.len() as f64;
        println!(
            "  ║  Avg Realtime:   {:.1}x                                                       ║",
            avg_factor
        );
    }
    println!("  ╚═══════════════════════════════════════════════════════════════════════════════╝");
    println!();
}

fn run_batch_benchmark(models: &[ModelInfo], audio: &[f32]) {
    println!("  Running batch benchmarks (30s audio processed as single chunk)...");
    println!();

    let mut results = Vec::new();

    for model in models {
        print!("  Testing {}... ", model.name);
        std::io::stdout().flush().unwrap();

        let metrics = run_benchmark_for_model(model, audio);

        if metrics.inference_time_secs > 0.0 {
            println!(
                "{:.2}s ({:.1}x realtime)",
                metrics.inference_time_secs, metrics.realtime_factor
            );
        } else {
            println!("FAILED");
        }

        results.push(metrics);
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!();

    print_results_table(&results);
    print_summary(&results);
}

fn run_e2e_benchmark(models: &[ModelInfo], audio: &[f32]) {
    println!(
        "  Running E2E benchmarks (streaming {}ms chunks)...",
        CHUNK_SIZE_MS
    );
    println!();

    let mut results = Vec::new();

    for model in models {
        print!("  Testing {}... ", model.name);
        std::io::stdout().flush().unwrap();

        let metrics = run_e2e_benchmark_for_model(model, audio);

        if metrics.total_time_secs > 0.0 {
            println!(
                "{:.2}s total ({:.0}ms/chunk, {:.1}x realtime)",
                metrics.total_time_secs, metrics.chunk_latency_ms, metrics.realtime_factor
            );
        } else {
            println!("FAILED");
        }

        results.push(metrics);
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!();

    print_e2e_results_table(&results);
    print_e2e_summary(&results);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Show help if requested
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Polynotes Benchmark");
        println!();
        println!("Usage:");
        println!("  cargo run --release --bin benchmark [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --e2e, -e       Run end-to-end latency benchmark (streaming mode)");
        println!("  --help, -h      Show this help message");
        println!();
        println!("Examples:");
        println!("  cargo run --release --bin benchmark        # Batch benchmark");
        println!("  cargo run --release --bin benchmark --e2e  # E2E streaming benchmark");
        return;
    }

    let is_e2e = args.iter().any(|arg| arg == "--e2e" || arg == "-e");

    print_banner(if is_e2e { "e2e" } else { "batch" });

    let models = find_available_models();

    if models.is_empty() {
        eprintln!("  ✗ No models found!");
        eprintln!();
        eprintln!("  Please ensure you have whisper models in:");
        eprintln!("    - core/whisper.cpp/models/");
        eprintln!();
        eprintln!("  Run setup.cmd (Windows) or bash setup.sh (Linux/Mac)");
        std::process::exit(1);
    }

    println!("  Found {} available models:", models.len());
    for m in &models {
        println!("    - {} ({})", m.name, m.expected_speed);
    }
    println!();

    println!("  Generating synthetic test audio...");
    let e2e_duration = if is_e2e {
        E2E_TEST_DURATION_SECS
    } else {
        TEST_DURATION_SECS
    };
    let audio = generate_synthetic_speech_audio(e2e_duration, SAMPLE_RATE);
    println!(
        "  ✓ Generated {:.2} seconds of audio ({} samples)",
        audio.len() as f64 / SAMPLE_RATE as f64,
        audio.len()
    );
    println!();

    if is_e2e {
        run_e2e_benchmark(&models, &audio);
    } else {
        run_batch_benchmark(&models, &audio);
    }
}
