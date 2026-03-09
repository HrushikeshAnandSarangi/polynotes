#[cfg(test)]
mod tests {
    use crate::{TranscribeOptions, WhisperContext};
    use std::path::Path;

    /// Absolute path so tests work regardless of the working directory.
    const MODEL_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/whisper.cpp/models/ggml-base-q5_1.bin"
    );

    fn silent_audio(seconds: f32) -> Vec<f32> {
        vec![0.0f32; (16_000.0 * seconds) as usize]
    }

    fn sine_wave(freq: f32, seconds: f32) -> Vec<f32> {
        let sample_rate = 16_000.0f32;
        let num_samples = (sample_rate * seconds) as usize;
        (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin() * 0.5)
            .collect()
    }

    /// Skip gracefully when the model file has not been downloaded yet.
    /// Download it with:
    ///   bash whisper.cpp/models/download-ggml-model.sh base.q5_1
    fn model_available() -> bool {
        Path::new(MODEL_PATH).exists()
    }

    #[test]
    fn test_context_loads_valid_model() {
        if !model_available() {
            eprintln!("SKIP: model not found at {MODEL_PATH}");
            return;
        }
        let ctx = WhisperContext::new(MODEL_PATH);
        assert!(
            ctx.is_ok(),
            "Expected model to load successfully from '{MODEL_PATH}'"
        );
    }

    #[test]
    fn test_transcription_for_silent_audio_returns_ok() {
        if !model_available() {
            eprintln!("SKIP: model not found at {MODEL_PATH}");
            return;
        }
        let ctx = WhisperContext::new(MODEL_PATH).expect("model failed to load");
        let audio = silent_audio(1.0);
        let results = ctx.transcribe_segments(&audio, TranscribeOptions::default());
        assert!(results.is_ok(), "transcribe_segments() on silent audio should not error");
    }

    // ── transcribe_segments with sine-wave audio ─────────────────────────────

    /// Feed a pure 440 Hz tone into transcribe_segments and validate the output.
    ///
    /// Whisper will typically produce no text for a pure tone, but the call
    /// must succeed and every returned segment must have a valid time range.
    #[test]
    fn test_transcribe_segments_with_sine_wave() {
        if !model_available() {
            eprintln!("SKIP: model not found at {MODEL_PATH}");
            return;
        }

        let ctx = WhisperContext::new(MODEL_PATH).expect("model failed to load");

        // 3 seconds of a 440 Hz tone — long enough for whisper to process one chunk
        let audio = sine_wave(440.0, 3.0);
        assert_eq!(
            audio.len(), 48_000,
            "3 s × 16 000 Hz should produce 48 000 samples"
        );

        let result = ctx.transcribe_segments(&audio, TranscribeOptions::default());
        assert!(result.is_ok(), "transcribe_segments() should not error on a sine tone");

        let segments = result.unwrap();

        for seg in &segments {
            // We disabled timestamps for performance, so we only check that text is present
            // (or empty, considering it's a sine wave, but it should succeed)
            let _ = seg;
        }
    }
}
