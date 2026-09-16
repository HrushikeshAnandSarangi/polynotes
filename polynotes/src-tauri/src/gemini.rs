//! Post-class note generation via the Gemini Flash API.
//!
//! `build_request_body`/`parse_response` are pure and unit-tested without any
//! network access; `generate_notes` is the thin async wrapper that actually
//! calls the API. The API key is supplied per-call from the frontend
//! (stored in the app's localStorage, same as every other setting) and is
//! never persisted on the Rust side.

const GEMINI_MODEL: &str = "gemini-2.5-flash";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Flashcard {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeneratedNotes {
    #[serde(rename = "markdown_notes")]
    pub markdown: String,
    pub flashcards: Vec<Flashcard>,
}

#[derive(Debug)]
pub enum GeminiError {
    RequestFailed(String),
    UnexpectedStatus(u16, String),
    ParseFailed(String),
}

impl std::fmt::Display for GeminiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeminiError::RequestFailed(msg) => write!(f, "Gemini request failed: {msg}"),
            GeminiError::UnexpectedStatus(status, body) => {
                write!(f, "Gemini returned HTTP {status}: {body}")
            }
            GeminiError::ParseFailed(msg) => write!(f, "Failed to parse Gemini response: {msg}"),
        }
    }
}

const NOTES_PROMPT_PREFIX: &str = "\
You are helping a student turn a raw, possibly messy lecture transcript into \
study material. Produce concise, well-structured Markdown notes (headings, \
bullet points, bold for key terms) that capture the substance of the lecture \
without padding. Then produce 5-15 flashcards (question/answer pairs) \
covering the most exam-relevant facts and concepts from the transcript. \
Keep flashcard answers short (one sentence to a few sentences).\n\n\
Transcript:\n";

/// Builds the Gemini `generateContent` request body. Pure — no I/O — so it's
/// directly unit-testable.
pub fn build_request_body(transcript: &str) -> serde_json::Value {
    let prompt = format!("{NOTES_PROMPT_PREFIX}{transcript}");
    serde_json::json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": {
                "type": "OBJECT",
                "properties": {
                    "markdown_notes": { "type": "STRING" },
                    "flashcards": {
                        "type": "ARRAY",
                        "items": {
                            "type": "OBJECT",
                            "properties": {
                                "question": { "type": "STRING" },
                                "answer": { "type": "STRING" }
                            },
                            "required": ["question", "answer"]
                        }
                    }
                },
                "required": ["markdown_notes", "flashcards"]
            }
        }
    })
}

/// Parses a raw Gemini `generateContent` HTTP response body into
/// `GeneratedNotes`. Pure — no I/O — so it's directly unit-testable against a
/// hand-written fixture, no live API key required.
pub fn parse_response(body: &str) -> Result<GeneratedNotes, GeminiError> {
    let raw: serde_json::Value =
        serde_json::from_str(body).map_err(|e| GeminiError::ParseFailed(e.to_string()))?;

    let text = raw
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| {
            GeminiError::ParseFailed("response missing candidates[0].content.parts[0].text".into())
        })?;

    serde_json::from_str::<GeneratedNotes>(text).map_err(|e| GeminiError::ParseFailed(e.to_string()))
}

/// Calls the Gemini Flash API and returns structured notes + flashcards.
/// Network I/O only — the request/response shape is handled entirely by the
/// pure functions above.
pub async fn generate_notes(
    api_key: &str,
    transcript: &str,
    model: &str,
) -> Result<GeneratedNotes, GeminiError> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}"
    );
    let body = build_request_body(transcript);

    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| GeminiError::RequestFailed(e.to_string()))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| GeminiError::RequestFailed(e.to_string()))?;

    if !status.is_success() {
        return Err(GeminiError::UnexpectedStatus(status.as_u16(), text));
    }

    parse_response(&text)
}

#[tauri::command]
pub async fn generate_notes_cmd(transcript: String, api_key: String) -> Result<GeneratedNotes, String> {
    generate_notes(&api_key, &transcript, GEMINI_MODEL)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_request_body_embeds_transcript_and_schema() {
        let body = build_request_body("hello world");
        let text = body["contents"][0]["parts"][0]["text"].as_str().unwrap();
        assert!(text.contains("hello world"));
        assert_eq!(
            body["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert_eq!(
            body["generationConfig"]["responseSchema"]["required"][0],
            "markdown_notes"
        );
    }

    #[test]
    fn parse_response_extracts_structured_notes() {
        let inner = serde_json::json!({
            "markdown_notes": "# Title\n\nSome notes.",
            "flashcards": [{"question": "Q1?", "answer": "A1."}]
        })
        .to_string();
        let outer = serde_json::json!({
            "candidates": [{
                "content": { "parts": [{ "text": inner }] }
            }]
        })
        .to_string();

        let notes = parse_response(&outer).expect("should parse");
        assert_eq!(notes.markdown, "# Title\n\nSome notes.");
        assert_eq!(notes.flashcards.len(), 1);
        assert_eq!(notes.flashcards[0].question, "Q1?");
    }

    #[test]
    fn parse_response_errors_on_missing_candidates() {
        let err = parse_response("{}").unwrap_err();
        assert!(matches!(err, GeminiError::ParseFailed(_)));
    }
}
