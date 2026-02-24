use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ai::load_openai_config;
use crate::display::{get_stitched_layout, StitchedLayout};
use crate::screenshot::take_stitched_screenshot;

const MAX_ROUNDS: usize = 20;

const PREAMBLE: &str = r#"你是一个 macOS 屏幕控制助手。你可以看到当前屏幕截图，并通过工具执行操作来完成用户的任务。

⚠️ 坐标规则非常重要：
- 所有鼠标相关工具的坐标都是归一化坐标，范围 [0, 999]
- x=0 表示截图最左，x=999 表示截图最右
- y=0 表示截图最上，y=999 表示截图最下
- 你只需要基于当前截图给出 [0,999] 坐标，不需要关心多屏映射细节

## 可用工具

### 鼠标操作
- click(x, y) - 左键单击指定坐标
- right_click(x, y) - 右键点击指定坐标，打开右键菜单
- double_click(x, y) - 双击指定坐标，用于打开文件或选中文字
- hover(x, y) - 将鼠标移到指定坐标，不点击，用于触发悬停菜单或提示
- scroll(x, y, direction, clicks) - 在指定坐标滚动，direction 为 up/down
- drag(from_x, from_y, to_x, to_y) - 从起点拖拽到终点

### 键盘操作
- type_text(text) - 输入文本
- press_key(key) - 按下单个按键（如 enter, tab, escape, backspace, space, up, down, left, right, f1-f12）
- hotkey(modifiers, key) - 组合键操作（modifiers 为数组，如 [\"command\"], [\"command\", \"shift\"]）

### 应用管理
- open_application(app_name) - 打开应用
- close_application(app_name) - 关闭应用
- focus_application(app_name) - 聚焦应用到前台
- list_applications() - 列出已安装应用
- list_running_applications() - 列出运行中应用

### 任务控制
- task_complete(summary) - 任务完成时调用，summary 为完成摘要

## 操作规则
1. 仔细分析屏幕截图，确定需要操作的元素位置
2. 每次只执行一个有意义的操作步骤
3. 操作后等待屏幕更新，观察结果再决定下一步
4. 任务完成后必须调用 task_complete
"#;

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: AssistantMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AssistantMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: FunctionCall,
}

#[derive(Debug, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}

pub struct ScreenAgentV2;

impl ScreenAgentV2 {
    pub async fn run(task_goal: &str) -> Result<String> {
        let config = load_openai_config()?;
        let http_client = Client::new();
        let api_base = config.api_base.trim_end_matches('/');
        let api_url = format!("{}/chat/completions", api_base);

        let tools = get_tools_definition();
        let mut messages: Vec<Value> = vec![json!({"role": "system", "content": PREAMBLE})];

        let mut task_completed = false;
        let mut final_result = String::new();

        for round in 1..=MAX_ROUNDS {
            println!("--- 第 {} 轮 ---", round);

            let layout = get_stitched_layout().context("获取屏幕布局失败")?;
            let (screenshot_base64, _) = take_stitched_screenshot(&layout)
                .map_err(|e| anyhow!("多屏拼接截图失败: {}", e))?;

            let instruction = if round == 1 {
                format!("任务目标：{}\n\n请分析当前屏幕截图，执行下一步操作。", task_goal)
            } else {
                format!(
                    "任务目标：{}\n\n请分析当前屏幕截图，继续执行下一步操作。如果任务已完成，请调用 task_complete。",
                    task_goal
                )
            };

            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📤 [第 {} 轮] 发送给 AI 的输入：", round);
            println!("{}", instruction);
            println!("[截图已附带]");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

            messages.push(json!({
                "role": "user",
                "content": [
                    {"type": "text", "text": instruction},
                    {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", screenshot_base64)}}
                ]
            }));

            loop {
                let request_body = json!({
                    "model": config.model_name,
                    "messages": messages,
                    "tools": tools,
                    "enable_thinking": false
                });

                println!("📤 发送请求...");
                let response = http_client
                    .post(&api_url)
                    .header("Authorization", format!("Bearer {}", config.api_key))
                    .header("Content-Type", "application/json")
                    .json(&request_body)
                    .send()
                    .await
                    .context("请求 OpenAI 兼容接口失败")?;

                let status = response.status();
                let body = response.text().await.context("读取响应体失败")?;

                if !status.is_success() {
                    println!("❌ API 错误 ({}): {}", status, body);
                    final_result = format!("API 错误 ({}): {}", status, body);
                    break;
                }

                let chat_response: ChatResponse =
                    serde_json::from_str(&body).with_context(|| format!("响应 JSON 解析失败: {}", body))?;

                if let Some(usage) = &chat_response.usage {
                    println!(
                        "📊 Token 用量: prompt={}, completion={}, total={}",
                        usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                    );
                }

                let choice = chat_response
                    .choices
                    .first()
                    .ok_or_else(|| anyhow!("响应中不存在 choices[0]"))?;
                let message = &choice.message;

                if let Some(reasoning) = &message.reasoning_content {
                    println!("🧠 reasoning_content: {}", reasoning);
                }
                if let Some(content) = &message.content {
                    println!("📥 [第 {} 轮] AI 的输出：", round);
                    println!("{}", content);
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                }
                if let Some(finish_reason) = &choice.finish_reason {
                    println!("🏁 finish_reason: {}", finish_reason);
                }

                if let Some(tool_calls) = &message.tool_calls {
                    let assistant_content = message
                        .content
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null);

                    messages.push(json!({
                        "role": "assistant",
                        "content": assistant_content,
                        "tool_calls": tool_calls.iter().map(|tc| json!({
                            "id": tc.id,
                            "type": tc.call_type,
                            "function": {
                                "name": tc.function.name,
                                "arguments": tc.function.arguments
                            }
                        })).collect::<Vec<_>>()
                    }));

                    for tc in tool_calls {
                        let result = execute_tool(&tc.function.name, &tc.function.arguments, &layout)
                            .unwrap_or_else(|e| format!("工具执行失败: {}", e));

                        if let Some(summary) = result.strip_prefix("TASK_COMPLETE: ") {
                            task_completed = true;
                            final_result = summary.to_string();
                        }

                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": tc.id,
                            "content": result
                        }));
                    }

                    if task_completed {
                        return Ok(final_result);
                    }

                    // 每执行完一轮工具后立即回到外层循环，刷新截图。
                    break;
                }

                if let Some(content) = &message.content {
                    final_result = content.clone();
                }
                messages.push(json!({"role": "assistant", "content": message.content}));
                break;
            }

            if task_completed {
                return Ok(final_result);
            }

            prune_old_image_messages(&mut messages);
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        Ok(format!(
            "达到最大轮次 {}，最后结果: {}",
            MAX_ROUNDS, final_result
        ))
    }
}

fn execute_tool(name: &str, args: &str, layout: &StitchedLayout) -> Result<String> {
    let args: Value = serde_json::from_str(args)
        .with_context(|| format!("工具参数不是合法 JSON: {}", args))?;

    match name {
        "click" => {
            let norm_x = get_f64(&args, "x")?;
            let norm_y = get_f64(&args, "y")?;
            let (gx, gy) = normalized_to_global(layout, norm_x, norm_y)?;
            println!("[工具调用] click | 输入: norm_x={}, norm_y={}", norm_x, norm_y);
            crate::mouse::click(gx, gy).map_err(|e| anyhow!(e))?;
            let output = format!(
                "已点击归一化坐标 ({:.0}, {:.0}) -> 全局坐标 ({}, {})",
                norm_x, norm_y, gx, gy
            );
            println!("[工具结果] click | 输出: {}", output);
            Ok(output)
        }
        "right_click" => {
            let norm_x = get_f64(&args, "x")?;
            let norm_y = get_f64(&args, "y")?;
            let (gx, gy) = normalized_to_global(layout, norm_x, norm_y)?;
            println!(
                "[工具调用] right_click | 输入: norm_x={}, norm_y={}",
                norm_x, norm_y
            );
            crate::mouse::right_click(gx, gy).map_err(|e| anyhow!(e))?;
            let output = format!(
                "已右键点击归一化坐标 ({:.0}, {:.0}) -> 全局坐标 ({}, {})",
                norm_x, norm_y, gx, gy
            );
            println!("[工具结果] right_click | 输出: {}", output);
            Ok(output)
        }
        "double_click" => {
            let norm_x = get_f64(&args, "x")?;
            let norm_y = get_f64(&args, "y")?;
            let (gx, gy) = normalized_to_global(layout, norm_x, norm_y)?;
            println!(
                "[工具调用] double_click | 输入: norm_x={}, norm_y={}",
                norm_x, norm_y
            );
            crate::mouse::double_click(gx, gy).map_err(|e| anyhow!(e))?;
            let output = format!(
                "已双击归一化坐标 ({:.0}, {:.0}) -> 全局坐标 ({}, {})",
                norm_x, norm_y, gx, gy
            );
            println!("[工具结果] double_click | 输出: {}", output);
            Ok(output)
        }
        "hover" => {
            let norm_x = get_f64(&args, "x")?;
            let norm_y = get_f64(&args, "y")?;
            let (gx, gy) = normalized_to_global(layout, norm_x, norm_y)?;
            println!("[工具调用] hover | 输入: norm_x={}, norm_y={}", norm_x, norm_y);
            crate::mouse::hover(gx, gy).map_err(|e| anyhow!(e))?;
            let output = format!(
                "已悬停归一化坐标 ({:.0}, {:.0}) -> 全局坐标 ({}, {})",
                norm_x, norm_y, gx, gy
            );
            println!("[工具结果] hover | 输出: {}", output);
            Ok(output)
        }
        "scroll" => {
            let norm_x = get_f64(&args, "x")?;
            let norm_y = get_f64(&args, "y")?;
            let direction = get_optional_str(&args, "direction").unwrap_or("down");
            let clicks = get_optional_i32(&args, "clicks").unwrap_or(3);
            let (gx, gy) = normalized_to_global(layout, norm_x, norm_y)?;
            println!(
                "[工具调用] scroll | 输入: norm_x={}, norm_y={}, direction={}, clicks={}",
                norm_x, norm_y, direction, clicks
            );
            crate::mouse::scroll(gx, gy, direction, clicks).map_err(|e| anyhow!(e))?;
            let output = format!(
                "已在 ({:.0}, {:.0}) 滚动 {} {} 格",
                norm_x, norm_y, direction, clicks
            );
            println!("[工具结果] scroll | 输出: {}", output);
            Ok(output)
        }
        "drag" => {
            let from_x = get_f64(&args, "from_x")?;
            let from_y = get_f64(&args, "from_y")?;
            let to_x = get_f64(&args, "to_x")?;
            let to_y = get_f64(&args, "to_y")?;
            let (gfx, gfy) = normalized_to_global(layout, from_x, from_y)?;
            let (gtx, gty) = normalized_to_global(layout, to_x, to_y)?;
            println!(
                "[工具调用] drag | 输入: from=({}, {}), to=({}, {})",
                from_x, from_y, to_x, to_y
            );
            crate::mouse::drag(gfx, gfy, gtx, gty).map_err(|e| anyhow!(e))?;
            let output = format!(
                "已从 ({:.0}, {:.0}) 拖拽到 ({:.0}, {:.0})",
                from_x, from_y, to_x, to_y
            );
            println!("[工具结果] drag | 输出: {}", output);
            Ok(output)
        }
        "type_text" => {
            let text = get_str(&args, "text")?;
            println!("[工具调用] type_text | 输入: text={}", text);
            crate::keyboard::type_text(text).map_err(|e| anyhow!(e))?;
            let output = format!("已输入文本: {}", text);
            println!("[工具结果] type_text | 输出: {}", output);
            Ok(output)
        }
        "press_key" => {
            let key = get_str(&args, "key")?;
            println!("[工具调用] press_key | 输入: key={}", key);
            crate::keyboard::press_key(key).map_err(|e| anyhow!(e))?;
            let output = format!("已按下按键: {}", key);
            println!("[工具结果] press_key | 输出: {}", output);
            Ok(output)
        }
        "hotkey" => {
            let modifiers = get_string_array(&args, "modifiers")?;
            let key = get_str(&args, "key")?;
            let modifier_refs: Vec<&str> = modifiers.iter().map(String::as_str).collect();
            println!(
                "[工具调用] hotkey | 输入: modifiers={:?}, key={}",
                modifiers, key
            );
            crate::keyboard::hotkey(&modifier_refs, key).map_err(|e| anyhow!(e))?;
            let output = format!("已执行组合键: {:?}+{}", modifiers, key);
            println!("[工具结果] hotkey | 输出: {}", output);
            Ok(output)
        }
        "open_application" => {
            let app_name = get_str(&args, "app_name")?;
            println!("[工具调用] open_application | 输入: app_name={}", app_name);
            crate::app_manager::open_application(app_name).map_err(|e| anyhow!(e.to_string()))?;
            let output = format!("已打开应用: {}", app_name);
            println!("[工具结果] open_application | 输出: {}", output);
            Ok(output)
        }
        "close_application" => {
            let app_name = get_str(&args, "app_name")?;
            println!("[工具调用] close_application | 输入: app_name={}", app_name);
            crate::app_manager::close_application(app_name)
                .map_err(|e| anyhow!(e.to_string()))?;
            let output = format!("已关闭应用: {}", app_name);
            println!("[工具结果] close_application | 输出: {}", output);
            Ok(output)
        }
        "focus_application" => {
            let app_name = get_str(&args, "app_name")?;
            println!("[工具调用] focus_application | 输入: app_name={}", app_name);
            crate::app_manager::focus_application(app_name)
                .map_err(|e| anyhow!(e.to_string()))?;
            let output = format!("已聚焦应用: {}", app_name);
            println!("[工具结果] focus_application | 输出: {}", output);
            Ok(output)
        }
        "list_applications" => {
            println!("[工具调用] list_applications");
            let apps = crate::app_manager::list_applications().map_err(|e| anyhow!(e.to_string()))?;
            let output = serde_json::to_string(&apps)?;
            println!("[工具结果] list_applications | 输出: {}", output);
            Ok(output)
        }
        "list_running_applications" => {
            println!("[工具调用] list_running_applications");
            let apps =
                crate::app_manager::list_running_applications().map_err(|e| anyhow!(e.to_string()))?;
            let output = serde_json::to_string(&apps)?;
            println!("[工具结果] list_running_applications | 输出: {}", output);
            Ok(output)
        }
        "task_complete" => {
            let summary = get_optional_str(&args, "summary").unwrap_or("任务完成");
            println!("[工具调用] task_complete | 输入: summary={}", summary);
            let output = format!("TASK_COMPLETE: {}", summary);
            println!("[工具结果] task_complete | 输出: {}", output);
            Ok(output)
        }
        _ => Err(anyhow!("未知工具: {}", name)),
    }
}

fn get_tools_definition() -> Value {
    json!([
        {"type": "function", "function": {"name": "click", "description": "在屏幕指定坐标执行鼠标左键单击", "parameters": {"type": "object", "properties": {"x": {"type": "number", "description": "归一化 X 坐标 [0, 999]"}, "y": {"type": "number", "description": "归一化 Y 坐标 [0, 999]"}}, "required": ["x", "y"]}}},
        {"type": "function", "function": {"name": "right_click", "description": "在屏幕指定坐标执行鼠标右键点击", "parameters": {"type": "object", "properties": {"x": {"type": "number", "description": "归一化 X 坐标 [0, 999]"}, "y": {"type": "number", "description": "归一化 Y 坐标 [0, 999]"}}, "required": ["x", "y"]}}},
        {"type": "function", "function": {"name": "double_click", "description": "在屏幕指定坐标执行鼠标双击", "parameters": {"type": "object", "properties": {"x": {"type": "number", "description": "归一化 X 坐标 [0, 999]"}, "y": {"type": "number", "description": "归一化 Y 坐标 [0, 999]"}}, "required": ["x", "y"]}}},
        {"type": "function", "function": {"name": "hover", "description": "将鼠标移动到屏幕指定坐标，不点击。用于触发悬停菜单或提示", "parameters": {"type": "object", "properties": {"x": {"type": "number", "description": "归一化 X 坐标 [0, 999]"}, "y": {"type": "number", "description": "归一化 Y 坐标 [0, 999]"}}, "required": ["x", "y"]}}},
        {"type": "function", "function": {"name": "scroll", "description": "在屏幕指定坐标执行滚轮操作", "parameters": {"type": "object", "properties": {"x": {"type": "number", "description": "归一化 X 坐标 [0, 999]"}, "y": {"type": "number", "description": "归一化 Y 坐标 [0, 999]"}, "direction": {"type": "string", "description": "滚动方向: up 或 down"}, "clicks": {"type": "integer", "description": "滚动格数，默认 3"}}, "required": ["x", "y", "direction"]}}},
        {"type": "function", "function": {"name": "drag", "description": "从起点坐标拖拽到终点坐标", "parameters": {"type": "object", "properties": {"from_x": {"type": "number", "description": "起点归一化 X 坐标 [0, 999]"}, "from_y": {"type": "number", "description": "起点归一化 Y 坐标 [0, 999]"}, "to_x": {"type": "number", "description": "终点归一化 X 坐标 [0, 999]"}, "to_y": {"type": "number", "description": "终点归一化 Y 坐标 [0, 999]"}}, "required": ["from_x", "from_y", "to_x", "to_y"]}}},
        {"type": "function", "function": {"name": "type_text", "description": "输入文本内容", "parameters": {"type": "object", "properties": {"text": {"type": "string", "description": "要输入的文本"}}, "required": ["text"]}}},
        {"type": "function", "function": {"name": "press_key", "description": "按下单个按键", "parameters": {"type": "object", "properties": {"key": {"type": "string", "description": "按键名称，如 enter, tab, escape, backspace, space, up, down, left, right, f1-f12"}}, "required": ["key"]}}},
        {"type": "function", "function": {"name": "hotkey", "description": "执行组合键操作", "parameters": {"type": "object", "properties": {"modifiers": {"type": "array", "items": {"type": "string"}, "description": "修饰键数组，如 [\"command\"], [\"command\", \"shift\"]"}, "key": {"type": "string", "description": "主按键"}}, "required": ["modifiers", "key"]}}},
        {"type": "function", "function": {"name": "open_application", "description": "打开指定应用程序", "parameters": {"type": "object", "properties": {"app_name": {"type": "string", "description": "应用程序名称"}}, "required": ["app_name"]}}},
        {"type": "function", "function": {"name": "close_application", "description": "关闭指定应用程序", "parameters": {"type": "object", "properties": {"app_name": {"type": "string", "description": "应用程序名称"}}, "required": ["app_name"]}}},
        {"type": "function", "function": {"name": "focus_application", "description": "将指定应用程序聚焦到前台", "parameters": {"type": "object", "properties": {"app_name": {"type": "string", "description": "应用程序名称"}}, "required": ["app_name"]}}},
        {"type": "function", "function": {"name": "list_applications", "description": "列出系统中已安装的应用程序", "parameters": {"type": "object", "properties": {}}}},
        {"type": "function", "function": {"name": "list_running_applications", "description": "列出当前正在运行的应用程序", "parameters": {"type": "object", "properties": {}}}},
        {"type": "function", "function": {"name": "task_complete", "description": "任务完成时调用此工具", "parameters": {"type": "object", "properties": {"summary": {"type": "string", "description": "任务完成摘要"}}, "required": ["summary"]}}}
    ])
}

fn normalized_to_global(layout: &StitchedLayout, norm_x: f64, norm_y: f64) -> Result<(i32, i32)> {
    layout
        .normalized_to_global(norm_x, norm_y)
        .map_err(|e| anyhow!("坐标转换失败: {}", e))
}

fn get_f64(value: &Value, key: &str) -> Result<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("缺少或无效参数: {} (number)", key))
}

fn get_str<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("缺少或无效参数: {} (string)", key))
}

fn get_optional_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn get_optional_i32(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|v| i32::try_from(v).ok())
}

fn get_string_array(value: &Value, key: &str) -> Result<Vec<String>> {
    let arr = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("缺少或无效参数: {} (array)", key))?;

    arr.iter()
        .map(|v| {
            v.as_str()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow!("参数 {} 包含非字符串元素", key))
        })
        .collect()
}

fn prune_old_image_messages(messages: &mut [Value]) {
    let mut image_user_indexes = Vec::new();
    for (idx, msg) in messages.iter().enumerate() {
        let is_user = msg.get("role").and_then(Value::as_str) == Some("user");
        let has_image = msg
            .get("content")
            .and_then(Value::as_array)
            .map(|items| {
                items.iter().any(|item| {
                    item.get("type").and_then(Value::as_str) == Some("image_url")
                })
            })
            .unwrap_or(false);

        if is_user && has_image {
            image_user_indexes.push(idx);
        }
    }

    if image_user_indexes.len() <= 1 {
        return;
    }

    let keep_last = *image_user_indexes.last().unwrap_or(&0);
    for idx in image_user_indexes {
        if idx == keep_last {
            continue;
        }
        if let Some(content) = messages
            .get_mut(idx)
            .and_then(|m| m.get_mut("content"))
            .and_then(Value::as_array_mut)
        {
            content.retain(|item| item.get("type").and_then(Value::as_str) != Some("image_url"));
        }
    }
}
