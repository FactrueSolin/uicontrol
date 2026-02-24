use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rig::completion::message::{ImageDetail, ImageMediaType, UserContent};
use rig::completion::{Message, Prompt};
use rig::{OneOrMany, client::CompletionClient};

use crate::ai::{get_openai_client, load_openai_config};

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

fn mime_to_image_media_type(mime_type: &str) -> ImageMediaType {
    match mime_type {
        "image/png" => ImageMediaType::PNG,
        "image/jpeg" => ImageMediaType::JPEG,
        "image/gif" => ImageMediaType::GIF,
        "image/webp" => ImageMediaType::WEBP,
        _ => ImageMediaType::PNG,
    }
}

async fn call_vision_api(image_base64: &str, mime_type: &str, prompt: &str) -> Result<String> {
    let config = load_openai_config()?;
    let client = get_openai_client()?;
    let agent = client.agent(config.model_name).build();

    let media_type = mime_to_image_media_type(mime_type);
    let message = Message::User {
        content: OneOrMany::many(vec![
            UserContent::text(prompt),
            UserContent::image_base64(image_base64, Some(media_type), Some(ImageDetail::Auto)),
        ])
        .expect("多模态消息内容不能为空"),
    };

    let response = agent
        .prompt(message)
        .await
        .context("调用视觉模型 API 失败")?;

    Ok(response)
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
