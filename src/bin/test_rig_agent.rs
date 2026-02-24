//! 使用 rig 的实际 agent 流程来复现 ApiResponse 反序列化错误
//! 通过 RUST_LOG=trace 捕获 rig 发送的实际请求和收到的响应

use anyhow::{Context as _, Result, anyhow};
use rig::completion::message::{ImageDetail, ImageMediaType, UserContent};
use rig::completion::{Message, Prompt};
use rig::{OneOrMany, client::CompletionClient};

use uicontrol::ai::{get_openai_client, load_openai_config};
use uicontrol::tools::OpenApplicationTool;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化 tracing，捕获 rig 的内部日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rig=trace".parse().unwrap()),
        )
        .init();

    let config = load_openai_config()?;
    let client = get_openai_client()?;

    println!("📋 使用模型: {}", config.model_name);

    // 构建一个简单的 agent，只有一个工具
    let agent = client
        .agent(config.model_name.clone())
        .preamble("你是一个 macOS 屏幕控制助手。请使用工具来完成任务。")
        .default_max_turns(1)
        .tool(OpenApplicationTool)
        .build();

    // 截取屏幕
    println!("📸 正在截取屏幕...");
    let screenshots = uicontrol::screenshot::take_screenshot(Some(1))
        .map_err(|e| anyhow!("截图失败: {}", e))?;
    let screenshot_base64 = &screenshots[0].image_base64;
    println!("📸 截图完成，base64 长度: {}", screenshot_base64.len());

    // 构建多模态消息（和 screen_agent 一样）
    let message = Message::User {
        content: OneOrMany::many(vec![
            UserContent::text("任务目标：打开Safari浏览器\n\n请分析当前屏幕截图，执行下一步操作。"),
            UserContent::image_base64(
                screenshot_base64,
                Some(ImageMediaType::PNG),
                Some(ImageDetail::Auto),
            ),
        ])
        .expect("多模态消息内容不能为空"),
    };

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📤 调用 agent.prompt()（rig 内部处理 tool call）");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    match agent.prompt(message).await {
        Ok(response) => {
            println!("\n✅ agent.prompt() 成功!");
            println!("响应: {}", response);
        }
        Err(e) => {
            println!("\n❌ agent.prompt() 失败!");
            println!("错误类型: {:?}", e);
            println!("错误信息: {}", e);

            // 尝试获取更详细的错误链
            let mut source = std::error::Error::source(&e);
            while let Some(err) = source {
                println!("  caused by: {}", err);
                source = std::error::Error::source(err);
            }
        }
    }

    Ok(())
}
