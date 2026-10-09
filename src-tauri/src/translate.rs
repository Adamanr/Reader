//! Перевод через LibreTranslate.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct LibreTranslateOk {
    #[serde(rename = "translatedText")]
    translated_text: String,
}

#[tauri::command]
pub async fn translate_texts(
    texts: Vec<String>,
    source: String,
    target: String,
    api_base: String,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    let base = api_base.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err("Укажите URL сервера перевода (LibreTranslate) в настройках.".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{}/translate", base);
    let mut out: Vec<String> = Vec::with_capacity(texts.len());

    for t in texts {
        let mut body = serde_json::json!({
            "q": t,
            "source": source,
            "target": target,
            "format": "text",
        });
        if let Some(ref k) = api_key {
            let ks = k.trim();
            if !ks.is_empty() {
                body["api_key"] = serde_json::json!(ks);
            }
        }

        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Запрос перевода: {}", e))?;

        let status = resp.status();
        let raw = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            let short: String = raw.chars().take(280).collect();
            return Err(format!("Перевод: HTTP {} — {}", status, short));
        }

        let parsed: LibreTranslateOk = serde_json::from_str(&raw).map_err(|e| {
            format!(
                "Разбор ответа ({}): {}",
                e,
                raw.chars().take(160).collect::<String>()
            )
        })?;
        out.push(parsed.translated_text);
    }

    Ok(out)
}
