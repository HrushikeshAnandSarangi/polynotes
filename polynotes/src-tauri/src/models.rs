use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

pub(crate) static MODEL_PATH: Mutex<String> = Mutex::new(String::new());
pub(crate) const DEFAULT_MODEL_PATH: &str = env!("WHISPER_MODEL_PATH");

pub(crate) fn current_model_path() -> String {
    let guard = MODEL_PATH.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_empty() {
        DEFAULT_MODEL_PATH.to_string()
    } else {
        guard.clone()
    }
}

#[tauri::command]
pub fn set_model_path(path: String) {
    if let Ok(mut guard) = MODEL_PATH.lock() {
        *guard = path;
    }
}

#[tauri::command]
pub fn get_model_path() -> String {
    MODEL_PATH.lock().map(|g| g.clone()).unwrap_or_default()
}

#[derive(Clone, serde::Serialize)]
struct DownloadProgress {
    downloaded: u64,
    total: u64,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u32,
    pub url: String,
    pub quantization: String,
    pub description: String,
}

fn get_available_models_list() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "tiny.en-q5_1".to_string(),
            name: "Tiny English (q5_1)".to_string(),
            size_mb: 30,
            url: "ggml-tiny.en-q5_1.bin".to_string(),
            quantization: "q5_1".to_string(),
            description: "Fastest. English only.".to_string(),
        },
        ModelInfo {
            id: "base.en-q5_1".to_string(),
            name: "Base English (q5_1)".to_string(),
            size_mb: 76,
            url: "ggml-base.en-q5_1.bin".to_string(),
            quantization: "q5_1".to_string(),
            description: "Balanced. English only.".to_string(),
        },
        ModelInfo {
            id: "tiny-q5_1".to_string(),
            name: "Tiny (q5_1)".to_string(),
            size_mb: 32,
            url: "ggml-tiny-q5_1.bin".to_string(),
            quantization: "q5_1".to_string(),
            description: "Fastest. Multilingual.".to_string(),
        },
        ModelInfo {
            id: "base-q5_1".to_string(),
            name: "Base (q5_1)".to_string(),
            size_mb: 60,
            url: "ggml-base-q5_1.bin".to_string(),
            quantization: "q5_1".to_string(),
            description: "Balanced. Multilingual.".to_string(),
        },
    ]
}

#[tauri::command]
pub fn get_available_models() -> Vec<ModelInfo> {
    get_available_models_list()
}

#[tauri::command]
pub async fn download_model(app: AppHandle, model_id: String) -> Result<String, String> {
    // Find the model in our list
    let models = get_available_models_list();
    let model = models
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Unknown model: {}", model_id))?;

    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        model.url
    );

    let app_data = app.path().app_data_dir().map_err(|e: tauri::Error| e.to_string())?;
    let models_dir = app_data.join("models");
    tokio::fs::create_dir_all(&models_dir).await.map_err(|e: std::io::Error| e.to_string())?;

    let dest_path = models_dir.join(&model.url);

    // Check if already downloaded
    if dest_path.exists() {
        let path_str = dest_path.to_string_lossy().to_string();
        set_model_path(path_str.clone());
        return Ok(path_str);
    }

    let client = reqwest::Client::new();
    let res = client.get(&url).send().await.map_err(|e| format!("Failed to connect: {}", e))?;
    let total_size = res.content_length().unwrap_or((model.size_mb as u64) * 1_000_000);

    let mut file = tokio::fs::File::create(&dest_path).await.map_err(|e: std::io::Error| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    use futures_util::StreamExt;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e: reqwest::Error| format!("Error while downloading: {}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await.map_err(|e: std::io::Error| e.to_string())?;
        downloaded += chunk.len() as u64;

        let _ = app.emit("download_progress", DownloadProgress {
            downloaded,
            total: total_size
        });
    }

    let path_str = dest_path.to_string_lossy().to_string();
    set_model_path(path_str.clone());

    Ok(path_str)
}
