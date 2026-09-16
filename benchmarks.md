# Benchmarks

All numbers below were captured on an **AMD Ryzen 5 5600H** (6 physical cores). Two benchmark modes are measured: **batch**, which reflects Polynotes' actual production pipeline (VAD-gated, chunked inference), and **end-to-end (E2E) streaming**, a stress test that processes fixed 300ms chunks with no VAD gating to show the cost of naive streaming.

## Batch Transcription (30s synthetic audio)

| Model | Time | Realtime factor | Size | Type |
|---|---:|---:|---:|---|
| `tiny.en-q5_1` | 1.10s | **27.3x** | 30 MB | English |
| `base.en-q5_1` | 2.62s | **11.4x** | 76 MB | English |
| `tiny-q5_1` | 13.97s | 2.1x | 32 MB | Multilingual |
| `base-q5_1` | 24.30s | 1.2x | 60 MB | Multilingual |

All four models clear realtime (>1x). Average realtime factor across the set: **10.5x**.

### Optimization parameters

The batch benchmark uses these speed-optimized inference settings:

```rust
TranscribeOptions {
    language: "en",        // English default for speed
    audio_ctx: 256,        // Minimal context = ~3x faster
    beam_size: 1,          // Greedy sampling
    best_of: 1,            // Single sample
    max_len: 0,             // No limit
    no_context: true,      // No cross-chunk context
    n_threads: 6,           // Physical CPU cores
}
```

## End-to-End Streaming Latency (10s audio, 300ms chunks)

| Model | Chunk latency | Total time | Realtime factor | Notes |
|---|---:|---:|---:|---|
| `tiny.en-q5_1` | ~2461ms | ~81.2s | 0.4x | Too slow for naive streaming |
| `base.en-q5_1` | ~2690ms | ~88.8s | 0.3x | Too slow for naive streaming |

Neither model reaches realtime in this mode — that's expected, and it's why Polynotes doesn't transcribe raw 300ms chunks in production. See below.

### Why E2E is slower than batch

1. **Per-chunk overhead** — each 300ms chunk requires a full encoder pass.
2. **No batching** — Whisper can't optimize across chunks.
3. **Fixed costs** — model loading and memory allocation happen per chunk, not once.

### The production strategy: VAD + batch processing

Polynotes never runs the naive per-chunk mode in the app itself. Instead it:

1. Uses **WebRTC VAD** to process audio only when speech is detected.
2. Accumulates **larger chunks** (1–5s) before running inference.
3. Lets Whisper **batch** across that larger chunk instead of streaming token-by-token.

This is what gets production latency down to **2–4 seconds end-to-end**, and what the batch numbers above actually reflect — not the E2E numbers.

## v1 Feature Overhead

Confusion detection, code-switching, note generation, and export are new in v1. These tables measure **added latency only** — feature off vs feature on, against the same baseline as the tables above. There is no accuracy/quality benchmark here; see the callout at the end of this section for why.

### Confidence extraction (`extract_confidence`)

| Model | Baseline | Feature on | Delta |
|---|---:|---:|---:|
| `tiny.en-q5_1` | 1.01s | 0.98s | -3.0% |
| `base.en-q5_1` | 2.39s | 2.34s | -1.8% |
| `tiny-q5_1` | 12.17s | 12.05s | -0.9% |
| `base-q5_1` | 20.13s | 20.16s | +0.2% |

All deltas are within run-to-run noise. Reading `whisper_full_get_token_p` is a CPU-side read of a distribution `whisper_full` already computed — no extra inference pass — so this matches the design expectation. This is why Polynotes enables it by default in the app.

### Language auto-detection (`detect_language`)

| Model | Baseline | Feature on | Delta |
|---|---:|---:|---:|
| `tiny-q5_1` | 12.24s | 17.33s | **+41.6%** |
| `base-q5_1` | 20.09s | 33.70s | **+67.8%** |

`.en` (English-only) models are skipped — language detection isn't meaningful for them. Unlike confidence extraction, this is a **real, substantial cost**: triggering whisper.cpp's internal auto-detect path (a null `language` pointer) is not free, contrary to what reusing the existing code path might suggest — it measurably changes the decode behavior. This is exactly why Polynotes ships this feature **off by default**, as a Settings opt-in, rather than bundling it into the default pipeline the way confidence extraction is.

### Export generation (Markdown / PDF / Anki CSV)

Fixed ~500-word sample note + 10 flashcards, 20 iterations, no network:

| Format | Avg time |
|---|---:|
| Markdown | <0.01ms |
| Anki CSV | <0.01ms |
| PDF | 298.52ms |

Markdown and CSV are just string/byte formatting. PDF goes through printpdf's HTML layout engine (`PdfDocument::from_html`), which does real work (parsing, layout, font subsetting) — ~300ms is a one-time cost per export, not per keystroke, so it's not user-visible as lag.

### Gemini Flash note-generation latency (manual, network-dependent)

Not reproducible in CI — it needs a live API key and real network access, and will vary with API load, quota, and region. Run it yourself with:

```bash
GEMINI_API_KEY=your-key cargo run --release --bin benchmark -- --gemini-latency
```

It round-trips a representative ~60-word transcript and reports wall-clock latency. The `build-benchmark` CI job never sets `GEMINI_API_KEY`, so this mode always skips cleanly (exit 0) in CI.

### Accuracy/quality benchmarking — out of scope

The tables above measure latency only. Two things are deliberately **not** benchmarked here, because doing so honestly would require test data that doesn't exist yet:

- **Language-detection accuracy** — there is no labeled corpus of code-switched lecture audio with ground-truth per-segment language spans to score `detect_language`'s output against.
- **Note/flashcard quality** — there is no human-graded rubric or reference-notes set to score Gemini's output against (e.g. a ROUGE-style comparison or expert grading).

Fabricating precision/recall or quality scores without that data would be actively misleading. If this is worth adding later, it needs a labeled multilingual lecture corpus (for language detection) and a graded reference set (for note quality) — both are data-collection projects in their own right, not benchmarking-code changes.

## Running the benchmarks

```bash
# Download models first, if you haven't already
./setup.cmd   # Windows
bash setup.sh # Linux/macOS

# Batch benchmark (recommended — reflects real app performance)
cargo run --release --bin benchmark

# End-to-end streaming benchmark
cargo run --release --bin benchmark -- --e2e

# v1 feature overhead
cargo run --release --bin benchmark -- --confidence-overhead
cargo run --release --bin benchmark -- --lang-detect-overhead
cargo run --release --bin benchmark -- --export
GEMINI_API_KEY=your-key cargo run --release --bin benchmark -- --gemini-latency

# Show all options
cargo run --release --bin benchmark -- --help
```
