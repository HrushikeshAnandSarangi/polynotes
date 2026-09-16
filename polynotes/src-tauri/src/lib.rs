pub mod export;
pub mod gemini;
mod models;
mod transcription;

use models::{download_model, get_available_models, get_model_path, set_model_path};
use transcription::{start_transcription, stop_transcription};
use gemini::generate_notes_cmd;
use export::{export_anki_csv, export_markdown, export_pdf};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            start_transcription,
            stop_transcription,
            set_model_path,
            get_model_path,
            download_model,
            get_available_models,
            generate_notes_cmd,
            export_markdown,
            export_pdf,
            export_anki_csv,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
