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

## Running the benchmarks

```bash
# Download models first, if you haven't already
./setup.cmd   # Windows
bash setup.sh # Linux/macOS

# Batch benchmark (recommended — reflects real app performance)
cargo run --release --bin benchmark

# End-to-end streaming benchmark
cargo run --release --bin benchmark -- --e2e

# Show all options
cargo run --release --bin benchmark -- --help
```
