# Polynotes — Feature Documentation

> An offline-capable, multilingual lecture assistant built for real-world Indian academic speech.
> Built with Tauri 2.0, SolidJS, Rust, and whisper.cpp.

---

## Table of Contents

1. [Real-Time Multilingual Transcription](#1-real-time-multilingual-transcription)
2. [Voice Activity Detection (VAD) Gate](#2-voice-activity-detection-vad-gate)
3. [Push to Talk](#3-push-to-talk)
4. [Audio Input Modes](#4-audio-input-modes)
5. [Multilingual Code-Switching Handling](#5-multilingual-code-switching-handling)
6. [Translate-to-English Pipeline](#6-translate-to-english-pipeline)
7. [Temporal Anchoring](#7-temporal-anchoring)
8. [Confusion Detection](#8-confusion-detection)
9. [Post-Class Note Generation — Gemini API](#9-post-class-note-generation--gemini-api)
10. [Post-Class Note Generation — Local LLM](#10-post-class-note-generation--local-llm)
11. [Formula and Spoken Math Detection](#11-formula-and-spoken-math-detection)
12. [Prerequisite Flagging](#12-prerequisite-flagging)
13. [Exam Question Prediction](#13-exam-question-prediction)
14. [Silence and Emphasis Detection](#14-silence-and-emphasis-detection)
15. [Lecture Continuity Across Sessions](#15-lecture-continuity-across-sessions)
16. [Lecture Coherence Scoring](#16-lecture-coherence-scoring)
17. [Export Pipeline](#17-export-pipeline)
18. [First-Launch Model Download](#18-first-launch-model-download)
19. [Model Selection](#19-model-selection)
20. [Cross-Platform Build](#20-cross-platform-build)
21. [Android Support](#21-android-support)

---

## 1. Real-Time Multilingual Transcription

**Status:** Core Feature — MVP  
**Tech:** whisper.cpp via Rust FFI (bindgen), ggml-base-q5_0 model

Polynotes transcribes live lecture audio in real time using whisper.cpp — a high-performance C++ inference engine for OpenAI's Whisper model. The transcription pipeline runs entirely on-device with no internet connection required.

Supported languages include Hindi, Bengali, Telugu, Tamil, Odia, and all other languages in the Whisper multilingual model's training data.

**Pipeline:**
```
Microphone → Audio Buffer (10ms chunks) → VAD Gate → 
Chunk Accumulator (3-5s) → whisper.cpp inference → 
Transcription output → SolidJS UI
```

**Key design decision:** Audio is accumulated into 3-5 second chunks before inference rather than feeding whisper every 10ms. This gives the model enough context for accurate transcription while maintaining a responsive real-time feel.

---

## 2. Voice Activity Detection (VAD) Gate

**Status:** Core Feature — MVP  
**Tech:** `webrtc-vad` Rust crate (WebRTC VAD algorithm)

A lightweight Voice Activity Detection gate sits before the whisper.cpp inference engine. It processes audio in 10ms frames and only forwards speech segments to whisper, discarding silence and background noise entirely.

This prevents whisper from wasting compute cycles on empty audio, reduces hallucination on silence, and significantly improves battery life on laptops.

**Why WebRTC VAD over alternatives:**
- Runs in microseconds per frame — negligible CPU overhead
- No ML model required — pure DSP algorithm
- Proven in production by Google Chrome
- Available as a Rust crate with no additional FFI work

---

## 3. Push to Talk

**Status:** Core Feature — MVP  
**Tech:** Tauri global shortcut API

Hold a configurable hotkey (default: `Space`) to record, release to stop. Zero algorithmic complexity, immediate response, works reliably in any noise environment.

This is the recommended mode for noisy environments like crowded classrooms or labs where background VAD struggles to distinguish speech from ambient noise.

**Configurable hotkey** — users can reassign to any key combination in settings.

---

## 4. Audio Input Modes

**Status:** Core Feature — MVP

Three input modes selectable from settings:

| Mode | Description | Best For |
|------|-------------|----------|
| **Auto (WebRTC VAD)** | Automatically detects and captures speech | Quiet classrooms, default |
| **Push to Talk** | Hold hotkey to record | Noisy environments |
| **Continuous** | Always recording, no gating | Power users, controlled environments |

---

## 5. Multilingual Code-Switching Handling

**Status:** v1 Feature — Post-MVP  
**Tech:** Chunked inference with per-segment language detection

Indian academic lectures rarely stay in one language. Professors naturally switch mid-sentence between Hindi, English, and regional languages — a phenomenon called code-switching.

Polynotes detects language switches between whisper inference chunks using per-segment language probability scores from the whisper C API. When a switch is detected the pipeline adjusts the language hint for the next chunk rather than committing to a single language for the entire session.

**Example handled correctly:**
> *"So the time complexity of this algorithm है O(n log n), and ye basically means..."*

Standard whisper tools fail on this. Polynotes handles it.

**Why this matters:** This is an unsolved problem in existing transcription tools and the primary reason Polynotes exists as a distinct product rather than a whisper wrapper.

---

## 6. Translate-to-English Pipeline

**Status:** Core Feature — MVP  
**Tech:** whisper.cpp built-in translation flag

Whisper handles both transcription and translation to English in a single inference pass — no separate translation model required. When translate mode is enabled, Polynotes outputs English text regardless of the spoken input language.

This keeps the bundle lean — no additional model download, no separate inference step, no added latency.

**Language parameter in Rust:**
```rust
params.set_language(Some("auto")); // auto-detect source language
params.set_translate(true);        // output in English
```

---

## 7. Temporal Anchoring

**Status:** v1 Feature — Post-MVP  
**Tech:** whisper.cpp timestamp output, SolidJS reactive UI

Every transcribed sentence is tagged with the exact timestamp from the source audio at which it was spoken. In the notes view, each bullet point is clickable — clicking jumps the audio playback to that exact moment in the recorded lecture.

This means students can instantly navigate to any point in a lecture from their notes without scrubbing through audio manually.

**whisper.cpp provides this for free** — timestamp data is part of the standard inference output. The work is surfacing it meaningfully in the UI.

---

## 8. Confusion Detection

**Status:** v1 Feature — Post-MVP  
**Tech:** whisper.cpp per-segment confidence scores via C API

whisper.cpp exposes a confidence score for every transcribed segment. Low confidence segments typically indicate mumbling, heavy accent, fast speech, or technical jargon the model struggled with.

Polynotes flags these segments visually in the UI with a warning indicator and marks them in exported notes as `[unclear — review this section]`.

This prevents students from confidently writing down incorrectly transcribed technical terms.

**Confidence threshold** — configurable in settings, default 0.6.

---

## 9. Post-Class Note Generation — Gemini API

**Status:** v1 Feature — Post-MVP  
**Tech:** Gemini Flash API via `reqwest` in Rust backend

After class, the full transcription is passed to Gemini Flash with a structured prompt that extracts key concepts, definitions, important points, and a brief summary. Output is formatted as clean structured markdown notes.

Gemini Flash handles long lecture transcriptions well — a 1 hour lecture is approximately 8,000 tokens, costing fractions of a cent per session on free tier.

**User provides their own API key** — stored locally, never sent to Polynotes servers.

**Prompt structure:**
```
You are a lecture note assistant. Convert this raw transcription 
into structured notes with: key concepts, definitions, important 
points, and a brief summary. Preserve technical terminology exactly.

Transcription: {transcription_text}
```

---

## 10. Post-Class Note Generation — Local LLM

**Status:** v2 Feature  
**Tech:** llama.cpp C API via Rust FFI, phi-3-mini-q4 or gemma-2-2b-q4

For users who want fully offline note generation with no API key, Polynotes supports local LLM inference via llama.cpp — the same FFI pattern used for whisper.cpp.

Recommended models:

| Model | Size | Speed | Quality |
|-------|------|-------|---------|
| `phi-3-mini-q4` | ~2.3gb | Medium | Best |
| `gemma-2-2b-q4` | ~1.5gb | Fast | Good |
| `qwen2.5-1.5b-q4` | ~900mb | Fastest | Acceptable |

Processing a 1 hour lecture takes approximately 30-60 seconds on modern laptop hardware. Acceptable for post-class batch use.

---

## 11. Formula and Spoken Math Detection

**Status:** v2 Feature  
**Tech:** LLM prompt engineering + LaTeX rendering

Professors frequently dictate mathematical expressions verbally. Polynotes detects spoken math patterns in transcriptions and converts them to LaTeX notation in exported notes.

**Example:**
> *"The integral from zero to infinity of e to the power minus x dx equals one"*  
> → `$\int_0^{\infty} e^{-x} dx = 1$`

Particularly valuable for mathematics, physics, and engineering lectures.

---

## 12. Prerequisite Flagging

**Status:** v2 Feature  
**Tech:** LLM prompt engineering

During note generation the LLM identifies concepts mentioned in the lecture that were not explained — indicating the professor assumed prior knowledge. These are flagged in notes as assumed prerequisites with a reference to where they were previously covered if known.

**Example output:**
```
⚠️ Prerequisite: Fourier Transform — mentioned without explanation.
   See: Lecture 4 — Signal Processing Fundamentals
```

---

## 13. Exam Question Prediction

**Status:** v2 Feature  
**Tech:** Gemini API or local LLM

After note generation, the LLM analyzes emphasis patterns in the transcription — repeated concepts, professor verbal emphasis markers like "this is important", "remember this" — and generates a list of likely exam questions.

**Output format:**
```markdown
## Likely Exam Questions

1. Explain the time complexity of merge sort and justify with a recurrence relation.
2. What is the difference between stable and unstable sorting algorithms?
3. Derive the worst-case scenario for quicksort.
```

---

## 14. Silence and Emphasis Detection

**Status:** v2 Feature  
**Tech:** Pure DSP on audio stream — no ML required

Two acoustic signals that indicate importance in lectures:

- **Long pauses before a statement** — professors naturally pause before important points
- **Amplitude spikes** — vocal emphasis on key words

These signals are detected directly from the audio buffer using simple DSP — no additional model required. Segments following long pauses or containing amplitude spikes are weighted higher in note generation, ensuring important points are never buried.

---

## 15. Lecture Continuity Across Sessions

**Status:** v2 Feature  
**Tech:** Local SQLite database, embedding similarity

Polynotes remembers what was covered in previous lectures of the same course. When generating notes for a new lecture it contextualizes new content against the existing course knowledge base.

**Example annotation in notes:**
> *"This builds on Dijkstra's algorithm covered in Lecture 3 — October 12th"*

Courses are user-defined. Each lecture is tagged to a course on recording start.

---

## 16. Lecture Coherence Scoring

**Status:** v2 Feature  
**Tech:** Local knowledge graph, embedding similarity

After several lectures in a course, Polynotes builds a local concept graph. Each new lecture is scored for coherence — how well it connects to prior material. Gaps in the graph indicate topics the professor skipped or the student missed.

This feature is research-worthy and distinct enough to present at a student symposium or publish as a short technical writeup.

---

## 17. Export Pipeline

**Status:** v1 Feature — Post-MVP  
**Tech:** File system via Tauri API

One-click export to multiple formats:

| Format | Use Case |
|--------|----------|
| **Markdown** | Obsidian, Notion, general purpose |
| **PDF** | Printing, sharing |
| **Anki Flashcards** | Spaced repetition study |
| **Plain Text** | Universal fallback |

**Anki export** is the highest value format for students — automatically converting lecture notes into spaced repetition flashcards eliminates hours of manual card creation.

---

## 18. First-Launch Model Download

**Status:** Core Feature — MVP  
**Tech:** `reqwest` in Rust backend, progress bar in SolidJS UI

The whisper model is not bundled with the application binary. On first launch Polynotes downloads the selected model directly from Hugging Face and caches it locally.

This keeps the installable binary under 25mb on all platforms while giving users model choice.

**Download URLs:**
```
Base (default):  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_0.bin
Small (upgrade): https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_0.bin
```

A progress bar is shown during download. The app is fully functional immediately after download completes.

---

## 19. Model Selection

**Status:** Core Feature — MVP

Users can switch between whisper models in settings. Larger models provide better accuracy at the cost of inference speed and disk space.

| Model | Size | Speed | Multilingual Accuracy |
|-------|------|-------|-----------------------|
| `ggml-tiny-q5_0` | ~31mb | Fastest | Poor for non-English |
| `ggml-base-q5_0` | ~57mb | Fast | Good — **default** |
| `ggml-small-q5_0` | ~100mb | Medium | Best for multilingual |

**Recommendation shown in UI:** Base for most users, Small for heavy Hindi/regional language use.

---

## 20. Cross-Platform Build

**Status:** Core Feature — MVP  
**Tech:** GitHub Actions, Tauri tauri-apps/tauri-action

Polynotes ships native binaries for all three desktop platforms via GitHub Actions CI:

| Platform | Format | Build Runner |
|----------|--------|--------------|
| Windows | `.msi` installer | `windows-latest` + MSVC |
| macOS | `.dmg` | `macos-latest` + CoreML |
| Linux | `.AppImage` | `ubuntu-latest` |

Aggressive caching of `~/.cargo`, `target/`, and `whisper.cpp/build/` keeps CI build times under 10 minutes.

---

## 21. Android Support

**Status:** v2 Feature  
**Tech:** Tauri 2.0 Android target, Android NDK

Tauri 2.0 supports native Android builds. Polynotes Android will support:

- Real-time transcription via on-device whisper.cpp compiled for ARM64
- Push to talk input mode
- Note export to local storage
- Background model download on first launch

**Expected APK size:** ~25mb binary + ~57mb model downloaded on first launch.

Android build requires cross-compilation of whisper.cpp for `aarch64-linux-android` via Android NDK. Scheduled for v2 after desktop is stable.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                   SolidJS Frontend                   │
│         Real-time UI • Settings • Export             │
└──────────────────────┬──────────────────────────────┘
                       │ Tauri IPC (invoke / events)
┌──────────────────────▼──────────────────────────────┐
│                   Rust Backend                       │
│                                                      │
│  ┌─────────┐   ┌──────────┐   ┌──────────────────┐  │
│  │  Audio  │ → │ WebRTC   │ → │   Chunk          │  │
│  │ Capture │   │   VAD    │   │   Accumulator    │  │
│  └─────────┘   └──────────┘   └────────┬─────────┘  │
│                                        │             │
│  ┌─────────────────────────────────────▼──────────┐  │
│  │              whisper.cpp (FFI)                  │  │
│  │         ggml-base-q5_0 • multilingual           │  │
│  └─────────────────────────────────────┬──────────┘  │
│                                        │             │
│  ┌─────────────────────────────────────▼──────────┐  │
│  │           Note Generation                       │  │
│  │     Gemini Flash API  •  Local LLM (v2)         │  │
│  └─────────────────────────────────────┬──────────┘  │
│                                        │             │
│  ┌─────────────────────────────────────▼──────────┐  │
│  │           Local Storage (SQLite)                │  │
│  │    Transcripts • Notes • Course History         │  │
│  └────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

---

## Versioning

| Version | Scope |
|---------|-------|
| **MVP** | Features 1, 2, 3, 4, 6, 18, 19, 20 |
| **v1** | Features 5, 7, 8, 9, 17 |
| **v2** | Features 10, 11, 12, 13, 14, 15, 16, 21 |

---

*Polynotes — Built for how Indian students actually learn.*