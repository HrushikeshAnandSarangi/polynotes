use polynotes_core::{TranscribeOptions, WhisperContext};
use std::io::Write;
use std::time::Instant;

const SAMPLE_RATE: u32 = 16000;
const TEST_DURATION_SECS: f64 = 30.0;

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

fn print_banner() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                    POLYNOTES WHISPER BENCHMARK                           ║");
    println!("║                         Performance Analysis                             ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝");
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

fn main() {
    print_banner();

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
    let audio = generate_synthetic_speech_audio(TEST_DURATION_SECS, SAMPLE_RATE);
    println!(
        "  ✓ Generated {:.2} seconds of audio ({} samples)",
        audio.len() as f64 / SAMPLE_RATE as f64,
        audio.len()
    );
    println!();

    println!("  Running benchmarks...");
    println!();

    let mut results = Vec::new();

    for model in &models {
        print!("  Testing {}... ", model.name);
        std::io::Write::flush(&mut std::io::stdout()).unwrap();

        let metrics = run_benchmark_for_model(model, &audio);

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
