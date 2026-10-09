//! OpenAI-совместимый API (LM Studio, Ollama): чат, список моделей, эмбеддинги.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

/// Как TranslateBooksWithLLMs OpenAI: для локального OpenAI-совместимого API отключаем thinking.
pub(crate) fn llm_local_disable_thinking(base: &str) -> bool {
    let b = base.to_lowercase();
    let local = b.contains("127.0.0.1") || b.contains("localhost");
    let official = b.contains("api.openai.com");
    local && !official
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiChatChoice {
    message: OpenAiChatMsgBody,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiChatMsgBody {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiModelList {
    data: Vec<OpenAiModelEntry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiModelEntry {
    id: String,
}

/// OpenAI-совместимый чат (LM Studio, Ollama `/v1`).
#[tauri::command]
pub async fn llm_chat_completion(
    base_url: String,
    model: String,
    messages: Vec<LlmMessage>,
    temperature: f64,
    max_tokens: Option<u32>,
) -> Result<String, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("Укажите URL API (например http://127.0.0.1:1234/v1)".into());
    }
    let url = format!("{}/chat/completions", base);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(900))
        .build()
        .map_err(|e| e.to_string())?;

    let mut payload = serde_json::json!({
        "model": model,
        "messages": serde_json::to_value(&messages).map_err(|e| e.to_string())?,
        "temperature": temperature,
        "stream": false,
    });
    if let Some(mt) = max_tokens {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("max_tokens".into(), serde_json::json!(mt));
        }
    }
    if llm_local_disable_thinking(base) {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("thinking".into(), serde_json::json!(false));
            obj.insert("enable_thinking".into(), serde_json::json!(false));
            obj.insert(
                "chat_template_kwargs".into(),
                serde_json::json!({ "enable_thinking": false }),
            );
        }
    }

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("LLM: {}", e))?;

    let status = resp.status();
    let raw = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let short: String = raw.chars().take(400).collect();
        return Err(format!("LLM: HTTP {} — {}", status, short));
    }

    let parsed: OpenAiChatResponse = serde_json::from_str(&raw).map_err(|e| {
        format!(
            "LLM JSON ({}): {}",
            e,
            raw.chars().take(200).collect::<String>()
        )
    })?;

    let content = parsed
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    Ok(content)
}

/// GET `{base_url}/models`.
#[tauri::command]
pub async fn llm_list_models(base_url: String) -> Result<Vec<String>, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("Укажите URL API".into());
    }
    let url = format!("{}/models", base);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Модели: {}", e))?;

    let status = resp.status();
    let raw = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let short: String = raw.chars().take(400).collect();
        return Err(format!("Модели: HTTP {} — {}", status, short));
    }

    let parsed: OpenAiModelList = serde_json::from_str(&raw).map_err(|e| {
        format!(
            "Список моделей ({}): {}",
            e,
            raw.chars().take(200).collect::<String>()
        )
    })?;

    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbedding>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiEmbedding {
    embedding: Vec<f32>,
    #[serde(default)]
    index: usize,
}

/// OpenAI-совместимые эмбеддинги (`/embeddings`) — для «созвездия цитат».
#[tauri::command]
pub async fn llm_embeddings(
    base_url: String,
    model: String,
    inputs: Vec<String>,
) -> Result<Vec<Vec<f32>>, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("Укажите URL API".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(format!("{base}/embeddings"))
        .json(&serde_json::json!({ "model": model, "input": inputs }))
        .send()
        .await
        .map_err(|e| format!("Эмбеддинги: {e}"))?;
    let status = resp.status();
    let raw = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let short: String = raw.chars().take(400).collect();
        return Err(format!("Эмбеддинги: HTTP {status} — {short}"));
    }
    let mut parsed: OpenAiEmbeddingResponse =
        serde_json::from_str(&raw).map_err(|e| format!("Эмбеддинги JSON: {e}"))?;
    parsed.data.sort_by_key(|d| d.index);
    Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
}
