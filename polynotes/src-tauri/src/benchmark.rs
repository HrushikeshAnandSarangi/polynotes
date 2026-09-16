use polynotes_core::{TranscribeOptions, WhisperContext};
use polynotes_lib::{export, gemini};
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
    run_benchmark_for_model_with_opts(model, audio, TranscribeOptions::default())
}

/// Same as `run_benchmark_for_model` but with caller-supplied
/// `TranscribeOptions`, so the confidence/language overhead benchmarks can
/// reuse it for both the baseline and feature-enabled runs.
fn run_benchmark_for_model_with_opts(
    model: &ModelInfo,
    audio: &[f32],
    opts: TranscribeOptions,
) -> BenchmarkMetrics {
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

// ── Feature overhead benchmarks ──────────────────────────────────────────
// Measure the added cost of opt-in features against the same fixed-language,
// no-extraction baseline documented in benchmarks.md — never accuracy, only
// latency. See benchmarks.md's "Accuracy/quality benchmarking — out of
// scope" note for why.

#[derive(Debug)]
struct OverheadMetrics {
    model_name: String,
    baseline_secs: f64,
    feature_secs: f64,
}

impl OverheadMetrics {
    fn delta_ms(&self) -> f64 {
        (self.feature_secs - self.baseline_secs) * 1000.0
    }

    fn delta_pct(&self) -> f64 {
        if self.baseline_secs > 0.0 {
            ((self.feature_secs - self.baseline_secs) / self.baseline_secs) * 100.0
        } else {
            0.0
        }
    }
}

fn print_overhead_table(title: &str, results: &[OverheadMetrics]) {
    println!();
    println!("  {title}");
    println!("  ┌─────────────────┬────────────┬────────────┬───────────┬──────────┐");
    println!(
        "  │ {:^15} │ {:^10} │ {:^10} │ {:^9} │ {:^8} │",
        "Model", "Baseline", "Feature On", "Delta", "Delta %"
    );
    println!("  ├─────────────────┼────────────┼────────────┼───────────┼──────────┤");
    for r in results {
        println!(
            "  │ {:^15} │ {:^9.2}s │ {:^9.2}s │ {:^+8.0}ms │ {:^+7.1}% │",
            r.model_name,
            r.baseline_secs,
            r.feature_secs,
            r.delta_ms(),
            r.delta_pct()
        );
    }
    println!("  └─────────────────┴────────────┴────────────┴───────────┴──────────┘");
    println!();
}

fn run_confidence_overhead_benchmark(models: &[ModelInfo], audio: &[f32]) {
    println!("  Measuring confidence-extraction overhead (baseline vs extract_confidence=true)...");

    let mut results = Vec::new();
    for model in models {
        print!("  Testing {}... ", model.name);
        std::io::stdout().flush().unwrap();

        let baseline = run_benchmark_for_model_with_opts(model, audio, TranscribeOptions::default());
        let feature = run_benchmark_for_model_with_opts(
            model,
            audio,
            TranscribeOptions {
                extract_confidence: true,
                ..TranscribeOptions::default()
            },
        );
        println!(
            "{:.2}s -> {:.2}s",
            baseline.inference_time_secs, feature.inference_time_secs
        );

        results.push(OverheadMetrics {
            model_name: model.name.clone(),
            baseline_secs: baseline.inference_time_secs,
            feature_secs: feature.inference_time_secs,
        });
    }

    print_overhead_table("Confidence Extraction Overhead", &results);
}

fn run_lang_detect_overhead_benchmark(models: &[ModelInfo], audio: &[f32]) {
    println!("  Measuring language-detection overhead (baseline vs detect_language=true)...");
    println!("  (English-only .en models are skipped: language detection isn't meaningful for them.)");

    let multilingual: Vec<&ModelInfo> = models.iter().filter(|m| !m.name.contains(".en")).collect();
    if multilingual.is_empty() {
        println!("  No multilingual models found — run setup.sh/setup.cmd to download tiny-q5_1 or base-q5_1.");
        return;
    }

    let mut results = Vec::new();
    for model in multilingual {
        print!("  Testing {}... ", model.name);
        std::io::stdout().flush().unwrap();

        let baseline = run_benchmark_for_model_with_opts(model, audio, TranscribeOptions::default());
        let feature = run_benchmark_for_model_with_opts(
            model,
            audio,
            TranscribeOptions {
                detect_language: true,
                ..TranscribeOptions::default()
            },
        );
        println!(
            "{:.2}s -> {:.2}s",
            baseline.inference_time_secs, feature.inference_time_secs
        );

        results.push(OverheadMetrics {
            model_name: model.name.clone(),
            baseline_secs: baseline.inference_time_secs,
            feature_secs: feature.inference_time_secs,
        });
    }

    print_overhead_table("Language Auto-Detection Overhead", &results);
}

const SAMPLE_MARKDOWN_NOTES: &str = "\
# Sample Lecture Notes\n\n\
## Introduction\n\n\
This is a representative ~500-word sample note used only to time export \
generation, not to evaluate note quality.\n\n\
- Point one about the lecture topic\n\
- Point two with a **bold** key term\n\
- Point three referencing a formula or definition\n\n\
## Details\n\n\
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod \
tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim \
veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea \
commodo consequat. Duis aute irure dolor in reprehenderit in voluptate \
velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint \
occaecat cupidatat non proident, sunt in culpa qui officia deserunt \
mollit anim id est laborum.\n\n\
## Summary\n\n\
A closing paragraph summarizing the key takeaways from the session.\n";

fn sample_flashcards() -> Vec<gemini::Flashcard> {
    (0..10)
        .map(|i| gemini::Flashcard {
            question: format!("Sample question {}?", i + 1),
            answer: format!("Sample answer {} with some, punctuation.", i + 1),
        })
        .collect()
}

fn run_export_benchmark() {
    const ITERATIONS: u32 = 20;
    println!("  Timing export generation ({ITERATIONS} iterations, fixed ~500-word sample note + 10 flashcards, no network)...");
    println!();

    let flashcards = sample_flashcards();

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = export::markdown_export("Sample Lecture", SAMPLE_MARKDOWN_NOTES);
    }
    let markdown_avg_ms = start.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = export::anki_csv_export(&flashcards);
    }
    let csv_avg_ms = start.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = export::pdf_export("Sample Lecture", SAMPLE_MARKDOWN_NOTES);
    }
    let pdf_avg_ms = start.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;

    println!("  ┌─────────────────┬────────────┐");
    println!("  │ {:^15} │ {:^10} │", "Format", "Avg time");
    println!("  ├─────────────────┼────────────┤");
    println!("  │ {:^15} │ {:^9.2}ms │", "Markdown", markdown_avg_ms);
    println!("  │ {:^15} │ {:^9.2}ms │", "Anki CSV", csv_avg_ms);
    println!("  │ {:^15} │ {:^9.2}ms │", "PDF", pdf_avg_ms);
    println!("  └─────────────────┴────────────┘");
    println!();
}

const SAMPLE_TRANSCRIPT_FOR_GEMINI: &str = "\
Today we covered the basics of supply and demand. When the price of a good \
increases, quantity demanded typically falls, all else equal. Producers \
respond to higher prices by supplying more. Equilibrium is the price at \
which quantity supplied equals quantity demanded. We also discussed price \
elasticity and how it varies across goods with more or fewer substitutes.";

fn run_gemini_latency_benchmark() {
    let api_key = match env::var("GEMINI_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            println!("  SKIP: GEMINI_API_KEY not set. This benchmark requires a live Gemini API key");
            println!("  and network access, so it never runs in CI. To run it locally:");
            println!("    GEMINI_API_KEY=your-key cargo run --release --bin benchmark -- --gemini-latency");
            return;
        }
    };

    println!("  Calling Gemini Flash with a representative ~60-word transcript...");
    println!("  (Network-dependent — latency will vary with API load/quota/region; not reproducible in CI.)");

    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("  Failed to start async runtime: {e}");
            return;
        }
    };

    let start = Instant::now();
    let result = runtime.block_on(gemini::generate_notes(
        &api_key,
        SAMPLE_TRANSCRIPT_FOR_GEMINI,
        "gemini-2.5-flash",
    ));
    let elapsed = start.elapsed();

    match result {
        Ok(notes) => {
            println!(
                "  Round-trip latency: {:.2}s ({} flashcards generated)",
                elapsed.as_secs_f64(),
                notes.flashcards.len()
            );
        }
        Err(e) => {
            eprintln!("  Gemini call failed: {e}");
        }
    }
    println!();
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
        println!("  --e2e, -e               Run end-to-end latency benchmark (streaming mode)");
        println!("  --confidence-overhead   Measure extract_confidence's added latency vs baseline");
        println!("  --lang-detect-overhead  Measure detect_language's added latency vs baseline");
        println!("  --export                Time Markdown/PDF/Anki-CSV export generation (no network)");
        println!("  --gemini-latency        Time a real Gemini Flash call (needs GEMINI_API_KEY, network)");
        println!("  --help, -h              Show this help message");
        println!();
        println!("Examples:");
        println!("  cargo run --release --bin benchmark                       # Batch benchmark");
        println!("  cargo run --release --bin benchmark --e2e                 # E2E streaming benchmark");
        println!("  cargo run --release --bin benchmark --confidence-overhead");
        println!("  cargo run --release --bin benchmark --export");
        println!("  GEMINI_API_KEY=... cargo run --release --bin benchmark --gemini-latency");
        return;
    }

    // Modes that don't need models/audio at all — handle before the shared
    // model-discovery setup below.
    if args.iter().any(|arg| arg == "--export") {
        print_banner("batch");
        run_export_benchmark();
        return;
    }
    if args.iter().any(|arg| arg == "--gemini-latency") {
        print_banner("batch");
        run_gemini_latency_benchmark();
        return;
    }

    let is_e2e = args.iter().any(|arg| arg == "--e2e" || arg == "-e");
    let is_confidence_overhead = args.iter().any(|arg| arg == "--confidence-overhead");
    let is_lang_detect_overhead = args.iter().any(|arg| arg == "--lang-detect-overhead");

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

    if is_confidence_overhead {
        run_confidence_overhead_benchmark(&models, &audio);
    } else if is_lang_detect_overhead {
        run_lang_detect_overhead_benchmark(&models, &audio);
    } else if is_e2e {
        run_e2e_benchmark(&models, &audio);
    } else {
        run_batch_benchmark(&models, &audio);
    }
}
