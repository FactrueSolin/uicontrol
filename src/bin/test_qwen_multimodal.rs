//! 测试 Qwen API 原始响应格式（带图片的多模态版本）
//! 模拟 screen_agent 的真实流程：发送截图+消息 → 收到 tool_call → 发送 tool result → 打印原始响应

use anyhow::{Context, Result};
use dotenv::dotenv;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenv();

    let api_base = std::env::var("OPENAI_API_BASE").context("缺少 OPENAI_API_BASE")?;
    let api_key = std::env::var("OPENAI_API_KEY").context("缺少 OPENAI_API_KEY")?;
    let model = std::env::var("OPENAI_MODEL_NAME").context("缺少 OPENAI_MODEL_NAME")?;

    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

    // 创建一个 100x100 红色 PNG 图片作为测试截图
    // 使用 screenshot 模块获取真实截图
    println!("📸 正在截取屏幕...");
    let screenshots = uicontrol::screenshot::take_screenshot(Some(1))
        .expect("截图失败");
    let screenshot_base64 = &screenshots[0].image_base64;
    println!("📸 截图完成，base64 长度: {}", screenshot_base64.len());

    let tools = json!([{
        "type": "function",
        "function": {
            "name": "open_application",
            "description": "打开指定的应用程序",
            "parameters": {
                "type": "object",
                "properties": {
                    "app_name": { "type": "string", "description": "应用程序名称" }
                },
                "required": ["app_name"]
            }
        }
    }]);

    // ===== 第一步：发送带图片的多模态消息 =====
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📤 第一步：发送带图片的多模态消息（模拟 screen_agent）");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let request1 = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "你是一个 macOS 屏幕控制助手。你可以看到当前屏幕截图，并通过工具执行操作来完成用户的任务。"
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "任务目标：打开Safari浏览器\n\n请分析当前屏幕截图，执行下一步操作。"
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
        "tools": tools,
        "tool_choice": "auto"
    });

    let resp1 = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request1)
        .send()
        .await?;

    let status1 = resp1.status();
    let body1 = resp1.text().await?;

    println!("HTTP Status: {}", status1);
    println!("原始响应 JSON:");
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body1) {
        println!("{}", serde_json::to_string_pretty(&parsed)?);
    } else {
        println!("{}", body1);
    }

    // 尝试反序列化第一步
    println!("\n🔍 第一步反序列化测试:");
    match serde_json::from_str::<rig::providers::openai::CompletionResponse>(&body1) {
        Ok(resp) => println!("✅ 成功! model={}, choices={}", resp.model, resp.choices.len()),
        Err(e) => println!("❌ 失败: {}", e),
    }

    // 解析第一步响应
    let resp1_json: serde_json::Value = serde_json::from_str(&body1)?;
    let choice = &resp1_json["choices"][0];
    let tool_calls = &choice["message"]["tool_calls"];

    if tool_calls.is_null() || !tool_calls.is_array() || tool_calls.as_array().unwrap().is_empty() {
        println!("\n⚠️ 第一步没有返回 tool_call，无法继续测试第二步");
        println!("finish_reason: {}", choice["finish_reason"]);
        println!("message.content: {}", choice["message"]["content"]);
        return Ok(());
    }

    let tool_call = &tool_calls[0];
    let tool_call_id = tool_call["id"].as_str().unwrap_or("unknown");
    let function_name = tool_call["function"]["name"].as_str().unwrap_or("unknown");
    let arguments = tool_call["function"]["arguments"].as_str().unwrap_or("{}");

    println!("\n✅ 收到 tool_call: {} {} {}", tool_call_id, function_name, arguments);

    // ===== 第二步：发送 tool result =====
    // 模拟 rig 的行为：把完整的 chat history（包括图片）+ tool result 发回
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📤 第二步：发送 tool result（含完整历史+图片）");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let request2 = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "你是一个 macOS 屏幕控制助手。你可以看到当前屏幕截图，并通过工具执行操作来完成用户的任务。"
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "任务目标：打开Safari浏览器\n\n请分析当前屏幕截图，执行下一步操作。"
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", screenshot_base64),
                            "detail": "auto"
                        }
                    }
                ]
            },
            {
                "role": "assistant",
                "content": choice["message"]["content"],
                "tool_calls": tool_calls
            },
            {
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": "已打开应用: Safari"
            }
        ],
        "tools": tools,
        "tool_choice": "auto"
    });

    let resp2 = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request2)
        .send()
        .await?;

    let status2 = resp2.status();
    let body2 = resp2.text().await?;

    println!("HTTP Status: {}", status2);
    println!("原始响应 JSON:");
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body2) {
        println!("{}", serde_json::to_string_pretty(&parsed)?);
    } else {
        println!("{}", body2);
    }

    // 尝试反序列化第二步
    println!("\n🔍 第二步反序列化测试:");
    match serde_json::from_str::<rig::providers::openai::CompletionResponse>(&body2) {
        Ok(resp) => {
            println!("✅ CompletionResponse 反序列化成功!");
            println!("  model: {}", resp.model);
            println!("  choices 数量: {}", resp.choices.len());
            for (i, c) in resp.choices.iter().enumerate() {
                println!("  choice[{}].finish_reason: {}", i, c.finish_reason);
            }
        }
        Err(e) => {
            println!("❌ CompletionResponse 反序列化失败: {}", e);
            println!("\n🔬 详细错误分析:");
            // 尝试逐字段分析
            let resp2_json: serde_json::Value = serde_json::from_str(&body2)?;
            if let Some(choices) = resp2_json["choices"].as_array() {
                for (i, choice) in choices.iter().enumerate() {
                    println!("  choice[{}].finish_reason 类型: {}", i, type_of_value(&choice["finish_reason"]));
                    println!("  choice[{}].message 字段: {:?}", i, choice["message"].as_object().map(|o| o.keys().collect::<Vec<_>>()));
                }
            }
        }
    }

    // ===== 也测试 rig 的 ApiResponse 枚举 =====
    println!("\n🔍 测试完整的 ApiResponse<CompletionResponse> 反序列化:");
    // 由于 ApiResponse 是 pub(crate)，我们无法直接测试，但可以模拟
    // 先测试 CompletionResponse，再测试 ApiErrorResponse
    let resp2_json: serde_json::Value = serde_json::from_str(&body2)?;
    
    // 检查是否能匹配 ApiErrorResponse（需要有 message 字段）
    if resp2_json.get("message").is_some() {
        println!("  响应有顶层 'message' 字段，可能被误匹配为 ApiErrorResponse");
    } else {
        println!("  响应没有顶层 'message' 字段");
    }

    Ok(())
}

fn type_of_value(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}
