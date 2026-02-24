use anyhow::{Result, anyhow};
use reqwest::Client;
use serde_json::json;
use uicontrol::ai::load_openai_config;
use uicontrol::display::get_stitched_layout;
use uicontrol::screenshot::take_stitched_screenshot;

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_openai_config()?;

    // 1. 获取布局和截图
    let layout = get_stitched_layout()?;
    println!(
        "拼接图尺寸: {}x{}",
        layout.stitched_width, layout.stitched_height
    );

    let (screenshot_base64, _layout_copy) =
        take_stitched_screenshot(&layout).map_err(|e| anyhow!(e.to_string()))?;
    println!("截图 base64 大小: {} KB", screenshot_base64.len() / 1024);

    // 2. 构建 Chat Completions 请求
    let client = Client::new();
    let request_body = json!({
        "model": config.model_name,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "描述一下这张截图中你看到了什么"
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", screenshot_base64),
                            "detail": "auto"
                        }
                    }
                ]
            }
        ],
        "max_tokens": 500
    });

    let request_size = serde_json::to_string(&request_body)?.len() / 1024;
    let api_base = config.api_base.trim_end_matches('/');
    let endpoint = format!("{}/chat/completions", api_base);

    println!("请求体大小: {} KB", request_size);
    println!("发送请求到: {}", endpoint);

    // 3. 发送请求
    let response = client
        .post(&endpoint)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    println!("HTTP 状态码: {}", status);
    println!("响应体 ({} bytes):", body.len());
    println!("{}", body);

    Ok(())
}
