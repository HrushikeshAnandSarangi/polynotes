//! Export logic for generated notes: Markdown, PDF, and an Anki-importable
//! CSV of flashcards. These are plain functions (no Tauri types) so
//! `benchmark.rs` can exercise them directly, and so they're unit-testable
//! without a running app.
//!
//! Anki export is CSV, not a native `.apkg` package: Anki's own File > Import
//! dialog accepts plain CSV directly (map columns to Front/Back), while
//! `.apkg` is an under-documented zipped-SQLite format that the sparse,
//! lightly-maintained Rust crates for it get subtly wrong. CSV is the choice
//! that reliably works with the Anki version installed today.

use std::collections::BTreeMap;
use crate::gemini::Flashcard;

#[derive(Debug)]
pub enum ExportError {
    Csv(String),
    Pdf(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::Csv(msg) => write!(f, "CSV export failed: {msg}"),
            ExportError::Pdf(msg) => write!(f, "PDF export failed: {msg}"),
        }
    }
}

/// Wraps the Gemini-generated Markdown body under a title heading.
pub fn markdown_export(title: &str, markdown_body: &str) -> String {
    format!("# {title}\n\n{markdown_body}\n")
}

/// Anki-compatible CSV: no header row (Anki's plain-CSV import expects raw
/// Front,Back rows), correct quoting via the `csv` crate for answers that
/// contain commas, quotes, or newlines.
pub fn anki_csv_export(flashcards: &[Flashcard]) -> Result<String, ExportError> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());

    for card in flashcards {
        writer
            .write_record([&card.question, &card.answer])
            .map_err(|e| ExportError::Csv(e.to_string()))?;
    }

    let bytes = writer
        .into_inner()
        .map_err(|e| ExportError::Csv(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| ExportError::Csv(e.to_string()))
}

/// Renders the notes as a simple HTML document and hands it to printpdf's
/// HTML-to-PDF layout engine (automatic page-breaking, no manual text
/// positioning needed).
pub fn pdf_export(title: &str, markdown_body: &str) -> Result<Vec<u8>, ExportError> {
    let mut html_body = String::new();
    pulldown_cmark::html::push_html(&mut html_body, pulldown_cmark::Parser::new(markdown_body));

    let html = format!(
        r#"<html>
<body style="padding:10mm;font-family:sans-serif;">
<h1>{title}</h1>
{html_body}
</body>
</html>"#,
        title = html_escape(title),
    );

    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = printpdf::GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        ..Default::default()
    };

    let mut warnings = Vec::new();
    let doc = printpdf::PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings)
        .map_err(|e| ExportError::Pdf(e.to_string()))?;

    Ok(doc.save(&printpdf::PdfSaveOptions::default(), &mut warnings))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ── thin Tauri command wrappers ──────────────────────────────────────────
// The frontend picks `dest_path` via @tauri-apps/plugin-dialog's save()
// dialog; these commands never choose a path themselves.

#[tauri::command]
pub async fn export_markdown(title: String, markdown_body: String, dest_path: String) -> Result<(), String> {
    let content = markdown_export(&title, &markdown_body);
    tokio::fs::write(&dest_path, content).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_pdf(title: String, markdown_body: String, dest_path: String) -> Result<(), String> {
    let bytes = pdf_export(&title, &markdown_body).map_err(|e| e.to_string())?;
    tokio::fs::write(&dest_path, bytes).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_anki_csv(flashcards: Vec<Flashcard>, dest_path: String) -> Result<(), String> {
    let content = anki_csv_export(&flashcards).map_err(|e| e.to_string())?;
    tokio::fs::write(&dest_path, content).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_export_includes_title_and_body() {
        let out = markdown_export("My Notes", "Some content.");
        assert!(out.starts_with("# My Notes\n\n"));
        assert!(out.contains("Some content."));
    }

    #[test]
    fn anki_csv_export_quotes_commas_and_newlines() {
        let cards = vec![Flashcard {
            question: "What, exactly?".to_string(),
            answer: "A, B, and C\nline two".to_string(),
        }];
        let csv = anki_csv_export(&cards).expect("csv export should succeed");
        assert!(csv.contains("\"What, exactly?\""));
        assert!(csv.contains("\"A, B, and C\nline two\""));

        // Round-trip through the csv crate's own reader to prove it's valid CSV,
        // not just a string that happens to contain the right substrings.
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(csv.as_bytes());
        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(&record[0], "What, exactly?");
        assert_eq!(&record[1], "A, B, and C\nline two");
    }

    #[test]
    fn pdf_export_produces_valid_pdf_bytes() {
        let bytes = pdf_export("Title", "# Heading\n\nSome body text.").expect("pdf export should succeed");
        assert!(bytes.starts_with(b"%PDF-"), "output should start with the PDF magic header");
    }
}
