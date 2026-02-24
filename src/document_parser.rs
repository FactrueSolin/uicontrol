use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::json;

use crate::ai::load_openai_config;

fn read_image_as_base64(image_path: &str) -> Result<String> {
    let bytes =
        std::fs::read(image_path).with_context(|| format!("读取图片文件失败: {image_path}"))?;
    Ok(STANDARD.encode(bytes))
}

fn detect_mime_type(image_path: &str) -> &str {
    let extension = std::path::Path::new(image_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

async fn call_vision_api(image_base64: &str, mime_type: &str, prompt: &str) -> Result<String> {
    let config = load_openai_config()?;
    let endpoint = format!("{}/chat/completions", config.api_base.trim_end_matches('/'));
    let image_data_url = format!("data:{mime_type};base64,{image_base64}");

    let payload = json!({
        "model": config.model_name,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": image_data_url
                        }
                    },
                    {
                        "type": "text",
                        "text": prompt
                    }
                ]
            }
        ]
    });

    let client = reqwest::Client::new();
    let response = client
        .post(endpoint)
        .bearer_auth(config.api_key)
        .json(&payload)
        .send()
        .await
        .context("调用视觉模型 API 失败")?
        .error_for_status()
        .context("视觉模型 API 返回错误状态")?;

    let body: serde_json::Value = response.json().await.context("解析视觉模型 API 响应失败")?;

    let content = body
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .context("视觉模型响应中缺少 choices[0].message.content")?;

    Ok(content.to_string())
}

pub async fn image_to_html(image_path: &str) -> Result<String> {
    let image_base64 = read_image_as_base64(image_path)?;
    let mime_type = detect_mime_type(image_path);
    let prompt = "qwenvl html";

    call_vision_api(&image_base64, mime_type, prompt).await
}

pub async fn image_to_markdown(image_path: &str) -> Result<String> {
    let image_base64 = read_image_as_base64(image_path)?;
    let mime_type = detect_mime_type(image_path);
    let prompt = "qwenvl markdown";

    call_vision_api(&image_base64, mime_type, prompt).await
}
