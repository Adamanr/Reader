//! Локальная озвучка: Piper и espeak-ng.

use crate::storage::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};

pub(crate) fn command_exists(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Доступные локальные движки озвучки.
#[tauri::command]
pub async fn tts_engines() -> Vec<String> {
    let mut out = Vec::new();
    if command_exists("piper") {
        out.push("piper".to_string());
    }
    if command_exists("espeak-ng") {
        out.push("espeak-ng".to_string());
    }
    out
}

/// Озвучка фрагмента локальным движком: WAV в base64.
/// `voice` — путь к модели `.onnx` для Piper или имя голоса espeak-ng.
#[tauri::command]
pub async fn tts_synthesize(
    engine: String,
    text: String,
    voice: Option<String>,
    rate: f32,
) -> Result<String, String> {
    use std::io::Write;
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Пустой текст".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let tmp = std::env::temp_dir().join(format!(
            "reader-tts-{}-{}.wav",
            std::process::id(),
            now_ms()
        ));
        let rate = rate.clamp(0.5, 3.0);
        let mut cmd = match engine.as_str() {
            "piper" => {
                let model = voice
                    .filter(|v| !v.trim().is_empty())
                    .ok_or("Укажите модель Piper (.onnx) в настройках")?;
                let mut c = std::process::Command::new("piper");
                c.arg("--model")
                    .arg(model.trim())
                    .arg("--output_file")
                    .arg(&tmp)
                    .arg("--length_scale")
                    .arg(format!("{:.2}", 1.0 / rate));
                c
            }
            "espeak-ng" => {
                let mut c = std::process::Command::new("espeak-ng");
                c.arg("-v")
                    .arg(
                        voice
                            .filter(|v| !v.trim().is_empty())
                            .unwrap_or_else(|| "ru".into()),
                    )
                    .arg("-s")
                    .arg(format!("{}", (175.0 * rate) as u32))
                    .arg("--stdin")
                    .arg("-w")
                    .arg(&tmp);
                c
            }
            _ => return Err("Неизвестный движок озвучки".to_string()),
        };
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Не удалось запустить {engine}: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("{engine}: {}", err.trim()));
        }
        let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&tmp);
        Ok(STANDARD.encode(bytes))
    })
    .await
    .map_err(|e| e.to_string())?
}
