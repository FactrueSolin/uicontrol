//! 测试 Qwen API 原始响应格式
//! 模拟 rig 的两步流程：发送消息 → 收到 tool_call → 发送 tool result → 打印原始响应
//! 用于诊断 ApiResponse 反序列化失败的根因

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

    // 定义一个简单的工具
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

    // ===== 第一步：发送用户消息，期望得到 tool_call =====
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📤 第一步：发送用户消息（期望触发 tool_call）");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let request1 = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "你是一个 macOS 屏幕控制助手。请使用工具来完成任务。"
            },
            {
                "role": "user",
                "content": "请打开 Safari 浏览器"
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
    // 格式化打印
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body1) {
        println!("{}", serde_json::to_string_pretty(&parsed)?);
    } else {
        println!("{}", body1);
    }

    // 解析第一步响应，提取 tool_call 信息
    let resp1_json: serde_json::Value = serde_json::from_str(&body1)?;
    let choice = &resp1_json["choices"][0];
    let tool_calls = &choice["message"]["tool_calls"];

    if tool_calls.is_null() || !tool_calls.is_array() || tool_calls.as_array().unwrap().is_empty() {
        println!("\n⚠️ 第一步没有返回 tool_call，无法继续测试第二步");
        println!("finish_reason: {}", choice["finish_reason"]);
        return Ok(());
    }

    let tool_call = &tool_calls[0];
    let tool_call_id = tool_call["id"].as_str().unwrap_or("unknown");
    let function_name = tool_call["function"]["name"].as_str().unwrap_or("unknown");
    let arguments = tool_call["function"]["arguments"].as_str().unwrap_or("{}");

    println!("\n✅ 收到 tool_call:");
    println!("  id: {}", tool_call_id);
    println!("  function: {}", function_name);
    println!("  arguments: {}", arguments);

    // ===== 第二步：发送 tool result，捕获原始响应 =====
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📤 第二步：发送 tool result（这是 rig 反序列化失败的步骤）");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let request2 = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "你是一个 macOS 屏幕控制助手。请使用工具来完成任务。"
            },
            {
                "role": "user",
                "content": "请打开 Safari 浏览器"
            },
            {
                "role": "assistant",
                "content": choice["message"]["content"],
                "tool_calls": tool_calls
            },
            {
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": format!("已打开应用: Safari")
            }
        ],
        "tools": tools,
        "tool_choice": "auto"
    });

    println!("请求 JSON:");
    println!("{}", serde_json::to_string_pretty(&request2)?);

    let resp2 = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request2)
        .send()
        .await?;

    let status2 = resp2.status();
    let body2 = resp2.text().await?;

    println!("\nHTTP Status: {}", status2);
    println!("原始响应 JSON:");
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body2) {
        println!("{}", serde_json::to_string_pretty(&parsed)?);
    } else {
        println!("{}", body2);
    }

    // ===== 尝试用 rig 的结构体反序列化 =====
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔍 尝试用 rig 的 CompletionResponse 结构体反序列化");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // 尝试反序列化为 rig 的 CompletionResponse
    match serde_json::from_str::<rig::providers::openai::CompletionResponse>(&body2) {
        Ok(resp) => {
            println!("✅ CompletionResponse 反序列化成功!");
            println!("  model: {}", resp.model);
            println!("  choices 数量: {}", resp.choices.len());
        }
        Err(e) => {
            println!("❌ CompletionResponse 反序列化失败: {}", e);
        }
    }

    // 也单独测试第一步的响应
    println!("\n🔍 也测试第一步响应的反序列化:");
    match serde_json::from_str::<rig::providers::openai::CompletionResponse>(&body1) {
        Ok(resp) => {
            println!("✅ 第一步 CompletionResponse 反序列化成功!");
            println!("  model: {}", resp.model);
        }
        Err(e) => {
            println!("❌ 第一步 CompletionResponse 反序列化失败: {}", e);
        }
    }

    // ===== 逐字段分析差异 =====
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔬 逐字段分析（对比 rig 期望的格式）");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let resp2_json: serde_json::Value = serde_json::from_str(&body2)?;

    // 检查关键字段
    println!("顶层字段: {:?}", resp2_json.as_object().map(|o| o.keys().collect::<Vec<_>>()));
    println!("id 类型: {}", type_of_value(&resp2_json["id"]));
    println!("object 类型: {}", type_of_value(&resp2_json["object"]));
    println!("created 类型: {}", type_of_value(&resp2_json["created"]));
    println!("model 类型: {}", type_of_value(&resp2_json["model"]));

    if let Some(choices) = resp2_json["choices"].as_array() {
        for (i, choice) in choices.iter().enumerate() {
            println!("\nchoice[{}]:", i);
            println!("  index: {} (类型: {})", choice["index"], type_of_value(&choice["index"]));
            println!("  finish_reason: {} (类型: {})", choice["finish_reason"], type_of_value(&choice["finish_reason"]));
            println!("  message.role: {}", choice["message"]["role"]);
            println!("  message.content: {} (类型: {})", choice["message"]["content"], type_of_value(&choice["message"]["content"]));
            println!("  message.tool_calls: {} (类型: {})", choice["message"]["tool_calls"], type_of_value(&choice["message"]["tool_calls"]));
            println!("  logprobs: {} (类型: {})", choice["logprobs"], type_of_value(&choice["logprobs"]));
        }
    }

    if !resp2_json["usage"].is_null() {
        let usage = &resp2_json["usage"];
        println!("\nusage:");
        println!("  prompt_tokens: {} (类型: {})", usage["prompt_tokens"], type_of_value(&usage["prompt_tokens"]));
        println!("  total_tokens: {} (类型: {})", usage["total_tokens"], type_of_value(&usage["total_tokens"]));
        println!("  completion_tokens: {} (类型: {})", usage["completion_tokens"], type_of_value(&usage["completion_tokens"]));
        println!("  所有字段: {:?}", usage.as_object().map(|o| o.keys().collect::<Vec<_>>()));
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
