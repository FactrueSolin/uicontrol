use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio::time::Duration;

use crate::ai::load_openai_config;
use crate::config::AppConfig;
use crate::display::{get_stitched_layout, StitchedLayout};
use crate::screenshot::take_stitched_screenshot;

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
- page_down() - 向下翻页（模拟 Space），适用于浏览器等页面滚动
- page_up() - 向上翻页（模拟 Shift+Space），适用于浏览器等页面滚动
- scroll(x, y, direction, clicks) - 在指定坐标滚动（鼠标滚轮，作为补充手段）
- drag(from_x, from_y, to_x, to_y) - 从起点拖拽到终点

### 键盘操作
- type_text(text) - 输入文本
- press_key(key) - 按下单个按键（如 enter, tab, escape, backspace, space, up, down, left, right, f1-f12）
- hotkey(modifiers, key) - 组合键操作（modifiers 为数组，如 [\"command\"], [\"command\", \"shift\"]）

### 应用管理
- open_application(app_name) - 打开应用
- close_application(app_name) - 关闭应用
- focus_application(app_name) - 聚焦应用到前台

### 任务控制
- wait(seconds) - 等待指定秒数，用于等待页面加载、动画完成等场景
- ask_human(question) - 向人类提问或请求帮助。当你不确定如何操作、需要人类协助完成某个步骤、或需要人类提供额外信息时使用此工具。
- task_complete(summary) - 任务完成时调用，summary 为完成摘要

### 系统工具
- read_clipboard() - 读取系统剪贴板中的文本内容

## 操作规则
1. 仔细分析屏幕截图，确定需要操作的元素位置
2. 你可以在一次回复中调用多个工具，它们会按顺序依次执行，尽量一次性多调用工具以提升操作效率。
3. 操作后等待屏幕更新，观察结果再决定下一步
4. 任务完成后必须调用 task_complete
5. 控制应用时优先使用快捷键（hotkey / press_key）完成操作，仅在快捷键不可用或无法确定时再使用鼠标。
6. 需要页面滚动时，优先使用 page_down/page_up；仅在必须精确滚轮控制时使用 scroll。
"#;

const SCREEN_UNCHANGED_HINT: &str =
    "[系统提示] 页面未发生显著变化。请基于当前屏幕继续分析并执行下一步操作。";

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

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<StreamToolCall>>,
}

#[derive(Debug, Deserialize)]
struct StreamToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    function: Option<StreamFunction>,
}

#[derive(Debug, Deserialize)]
struct StreamFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Default)]
struct PendingToolCall {
    id: Option<String>,
    call_type: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[derive(Debug, Serialize, Clone)]
struct SessionChatLog {
    task: String,
    start_time: String,
    rounds: Vec<RoundChatLog>,
}

#[derive(Debug, Serialize, Clone)]
struct RoundChatLog {
    round: usize,
    input_text: String,
    ai_thinking: String,
    ai_response: String,
    tool_calls: Vec<ToolCallLog>,
}

#[derive(Debug, Serialize, Clone)]
struct ToolCallLog {
    name: String,
    arguments: Value,
    result: String,
}

pub struct ScreenAgentV2;

impl ScreenAgentV2 {
    pub async fn run(task_goal: &str) -> Result<String> {
        let app_config = AppConfig::load().context("加载应用配置失败")?;
        let config = load_openai_config()?;
        let session_timestamp = now_unix_timestamp();
        let chat_log_path = build_chat_log_path(&session_timestamp);
        let session_chat_log = Arc::new(Mutex::new(SessionChatLog {
            task: task_goal.to_string(),
            start_time: session_timestamp,
            rounds: Vec::new(),
        }));

        let ctrlc_log = Arc::clone(&session_chat_log);
        let ctrlc_path = chat_log_path.clone();
        let ctrlc_handle = tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                eprintln!("\n⚠️ 收到 Ctrl+C 信号，正在保存聊天记录...");
                if let Err(e) = save_chat_log_from_shared(&ctrlc_path, &ctrlc_log).await {
                    eprintln!("❌ Ctrl+C 保存聊天记录失败: {}", e);
                }
                eprintln!("👋 进程已退出");
                std::process::exit(130);
            }
        });

        let http_client = Client::new();
        let api_base = config.api_base.trim_end_matches('/');
        let api_url = format!("{}/chat/completions", api_base);
        let max_rounds: usize = app_config.agent_max_rounds;

        let installed_apps = match crate::app_manager::list_applications() {
            Ok(apps) => apps,
            Err(e) => {
                eprintln!("⚠️ 获取已安装应用列表失败: {}", e);
                Vec::new()
            }
        };

        let running_apps = match crate::app_manager::list_running_applications() {
            Ok(apps) => apps,
            Err(e) => {
                eprintln!("⚠️ 获取运行中应用列表失败: {}", e);
                Vec::new()
            }
        };

        let app_context = format!(
            "{}\n\n{}",
            format_app_list_section("系统已安装的应用程序", &installed_apps),
            format_app_list_section("当前正在运行的应用程序", &running_apps)
        );

        let user_prompt_path = "prompt/user.md";
        let system_prompt = if let Ok(user_content) = std::fs::read_to_string(user_prompt_path) {
            let user_content = user_content.trim();
            if !user_content.is_empty() {
                println!("📝 已加载用户自定义提示词 ({})", user_prompt_path);
                format!("{}\n\n{}\n\n{}", PREAMBLE, app_context, user_content)
            } else {
                format!("{}\n\n{}", PREAMBLE, app_context)
            }
        } else {
            format!("{}\n\n{}", PREAMBLE, app_context)
        };

        let tools = get_tools_definition();
        let mut messages: Vec<Value> = vec![json!({"role": "system", "content": system_prompt})];

        let mut task_completed = false;
        let mut final_result = String::new();
        let mut round: usize = 1;
        let mut last_screenshot_base64: Option<String> = None;
        let mut last_focused_app: Option<String> = None;

        loop {
            if max_rounds > 0 && round > max_rounds {
                println!("⏸️  已达到最大轮次 ({})，暂停等待人类指示", max_rounds);
                send_notification(
                    "⚠️ 已达到最大轮次",
                    &format!("已执行 {} 轮，等待人类指示", max_rounds),
                );
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("请输入新的指示（直接回车或输入 quit 退出）：");

                let mut human_input = String::new();
                std::io::stdin()
                    .read_line(&mut human_input)
                    .unwrap_or_default();
                let human_input = human_input.trim();

                if human_input.is_empty() || human_input == "quit" || human_input == "exit" {
                    println!("👋 用户选择退出");
                    break;
                }

                messages.push(json!({
                    "role": "user",
                    "content": format!("用户补充指示：{}", human_input)
                }));

                round = 1;
                println!("▶️  继续执行，轮次已重置");
                continue;
            }

            println!("--- 第 {} 轮 ---", round);

            let layout = get_stitched_layout().context("获取屏幕布局失败")?;
            let (screenshot_base64, _) = take_stitched_screenshot(&layout)
                .map_err(|e| anyhow!("多屏拼接截图失败: {}", e))?;

            let mut attach_screenshot = true;
            if let Some(last_base64) = last_screenshot_base64.as_deref() {
                match crate::image_diff::compare_images_base64(last_base64, &screenshot_base64) {
                    Ok(diff_percent) => {
                        println!("🖼️ 截图差异: {:.4}% (阈值: 0.1000%)", diff_percent);
                        if diff_percent <= 0.1 {
                            attach_screenshot = false;
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️ 截图差异比对失败，回退为正常注入截图: {}", e);
                    }
                }
            }
            last_screenshot_base64 = Some(screenshot_base64.clone());

            let instruction = if round == 1 {
                format!(
                    "任务目标：{}\n\n请分析当前屏幕截图，执行下一步操作。",
                    task_goal
                )
            } else {
                format!(
                    "任务目标：{}\n\n请分析当前屏幕截图，继续执行下一步操作。如果任务已完成，请调用 task_complete。",
                    task_goal
                )
            };

            let focused_app_name = get_focused_app_name();
            let app_shortcuts_context = match focused_app_name.as_deref() {
                Some(app_name) => {
                    let should_inject_shortcuts = match last_focused_app.as_deref() {
                        None => true,
                        Some(last_app_name) => last_app_name != app_name,
                    };

                    if should_inject_shortcuts {
                        build_focused_app_shortcuts_context(app_name)
                    } else {
                        None
                    }
                }
                None => None,
            };
            last_focused_app = focused_app_name;

            let final_instruction = if let Some(shortcuts_context) = app_shortcuts_context {
                format!("{}\n\n{}", instruction, shortcuts_context)
            } else {
                instruction.clone()
            };

            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📤 [第 {} 轮] 发送给 AI 的输入：", round);
            println!("{}", final_instruction);
            if attach_screenshot {
                println!("[截图已附带]");
            } else {
                println!("{}", SCREEN_UNCHANGED_HINT);
            }
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

            let round_input_text = if attach_screenshot {
                final_instruction.clone()
            } else {
                format!("{}\n\n{}", final_instruction, SCREEN_UNCHANGED_HINT)
            };

            let mut round_chat_log = RoundChatLog {
                round,
                input_text: round_input_text,
                ai_thinking: String::new(),
                ai_response: String::new(),
                tool_calls: Vec::new(),
            };

            let mut user_content = vec![json!({"type": "text", "text": final_instruction})];
            if attach_screenshot {
                user_content.push(json!({
                    "type": "image_url",
                    "image_url": {"url": format!("data:image/png;base64,{}", screenshot_base64)}
                }));
            } else {
                user_content.push(json!({"type": "text", "text": SCREEN_UNCHANGED_HINT}));
            }

            messages.push(json!({
                "role": "user",
                "content": user_content
            }));

            loop {
                let request_body = json!({
                    "model": config.model_name,
                    "messages": messages,
                    "tools": tools,
                    "enable_thinking": true,
                    "thinking_budget": -1,
                    "stream": true
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

                if !status.is_success() {
                    let body = response.text().await.context("读取响应体失败")?;
                    println!("❌ API 错误 ({}): {}", status, body);
                    final_result = format!("API 错误 ({}): {}", status, body);
                    break;
                }

                let mut sse_buffer = String::new();
                let mut stream_done = false;
                let mut full_content = String::new();
                let mut full_reasoning = String::new();
                let mut pending_tool_calls: Vec<PendingToolCall> = Vec::new();
                let mut usage: Option<Usage> = None;
                let mut finish_reason: Option<String> = None;
                let mut reasoning_printed = false;
                let mut content_printed = false;

                let mut response = response;
                while let Some(chunk) = response.chunk().await.context("读取流式响应失败")?
                {
                    let chunk_text = String::from_utf8_lossy(&chunk);
                    sse_buffer.push_str(&chunk_text);

                    while let Some(newline_pos) = sse_buffer.find('\n') {
                        let raw_line = sse_buffer[..newline_pos].trim_end_matches('\r').to_string();
                        sse_buffer.drain(..=newline_pos);

                        let line = raw_line.trim();
                        if line.is_empty() || !line.starts_with("data: ") {
                            continue;
                        }

                        let data = &line[6..];
                        if data == "[DONE]" {
                            stream_done = true;
                            break;
                        }

                        let stream_chunk: StreamChunk = serde_json::from_str(data)
                            .with_context(|| format!("流式 chunk JSON 解析失败: {}", data))?;

                        if stream_chunk.usage.is_some() {
                            usage = stream_chunk.usage;
                        }

                        for choice in stream_chunk.choices {
                            if let Some(fr) = choice.finish_reason {
                                finish_reason = Some(fr);
                            }

                            let delta = choice.delta;
                            let _ = &delta.role;

                            if let Some(reasoning) = delta.reasoning_content {
                                if !reasoning_printed {
                                    print!("💭 ");
                                    reasoning_printed = true;
                                }
                                print!("{}", reasoning);
                                std::io::stdout().flush().ok();
                                full_reasoning.push_str(&reasoning);
                            }

                            if let Some(content) = delta.content {
                                if !content_printed {
                                    println!("📥 [第 {} 轮] AI 的输出：", round);
                                    content_printed = true;
                                }
                                print!("{}", content);
                                std::io::stdout().flush().ok();
                                full_content.push_str(&content);
                            }

                            if let Some(tool_calls) = delta.tool_calls {
                                for tc in tool_calls {
                                    merge_stream_tool_call(&mut pending_tool_calls, tc);
                                }
                            }
                        }
                    }

                    if stream_done {
                        break;
                    }
                }

                if reasoning_printed {
                    println!();
                }
                if content_printed {
                    println!();
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                }

                if let Some(usage) = &usage {
                    println!(
                        "📊 Token 用量：prompt={}, completion={}, total={}",
                        usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                    );
                }

                if let Some(finish_reason) = &finish_reason {
                    println!("🏁 finish_reason: {}", finish_reason);
                }

                let tool_calls = finalize_stream_tool_calls(pending_tool_calls);
                if !tool_calls.is_empty() {
                    for tc in &tool_calls {
                        println!(
                            "🧰 tool_call: id={}, type={}, name={}, arguments={}",
                            tc.id, tc.call_type, tc.function.name, tc.function.arguments
                        );
                    }
                }

                let message = AssistantMessage {
                    content: if full_content.is_empty() {
                        None
                    } else {
                        Some(full_content)
                    },
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    reasoning_content: if full_reasoning.is_empty() {
                        None
                    } else {
                        Some(full_reasoning)
                    },
                };

                round_chat_log.ai_thinking = message.reasoning_content.clone().unwrap_or_default();
                round_chat_log.ai_response = message.content.clone().unwrap_or_default();

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

                    for (idx, tc) in tool_calls.iter().enumerate() {
                        let result =
                            execute_tool(&tc.function.name, &tc.function.arguments, &layout)
                                .await
                                .unwrap_or_else(|e| format!("工具执行失败: {}", e));

                        let arguments = serde_json::from_str::<Value>(&tc.function.arguments)
                            .unwrap_or_else(|_| Value::String(tc.function.arguments.clone()));
                        round_chat_log.tool_calls.push(ToolCallLog {
                            name: tc.function.name.clone(),
                            arguments,
                            result: result.clone(),
                        });

                        if let Some(summary) = result.strip_prefix("TASK_COMPLETE: ") {
                            task_completed = true;
                            final_result = summary.to_string();
                            send_notification("✅ 任务已完成", summary);
                        }

                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": tc.id,
                            "content": result
                        }));

                        if idx + 1 < tool_calls.len() {
                            tokio::time::sleep(Duration::from_secs(3)).await;
                        }
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

            {
                let mut chat_log = session_chat_log.lock().await;
                chat_log.rounds.push(round_chat_log);
            }

            if task_completed {
                ctrlc_handle.abort();
                save_chat_log_from_shared(&chat_log_path, &session_chat_log).await?;
                return Ok(final_result);
            }

            tokio::time::sleep(Duration::from_secs(3)).await;
            round += 1;
        }

        ctrlc_handle.abort();
        save_chat_log_from_shared(&chat_log_path, &session_chat_log).await?;

        Ok(format!(
            "达到最大轮次 {}，最后结果: {}",
            max_rounds, final_result
        ))
    }
}

async fn execute_tool(name: &str, args: &str, layout: &StitchedLayout) -> Result<String> {
    let args: Value =
        serde_json::from_str(args).with_context(|| format!("工具参数不是合法 JSON: {}", args))?;

    match name {
        "click" => {
            let (norm_x, norm_y) = extract_coordinates(&args, "x", "y", "click")?;
            let (gx, gy) = normalized_to_global(layout, norm_x, norm_y)?;
            println!(
                "[工具调用] click | 输入: norm_x={}, norm_y={}",
                norm_x, norm_y
            );
            crate::mouse::click(gx, gy).map_err(|e| anyhow!(e))?;
            let output = format!(
                "已点击归一化坐标 ({:.0}, {:.0}) -> 全局坐标 ({}, {})",
                norm_x, norm_y, gx, gy
            );
            println!("[工具结果] click | 输出: {}", output);
            Ok(output)
        }
        "right_click" => {
            let (norm_x, norm_y) = extract_coordinates(&args, "x", "y", "right_click")?;
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
            let (norm_x, norm_y) = extract_coordinates(&args, "x", "y", "double_click")?;
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
            let (norm_x, norm_y) = extract_coordinates(&args, "x", "y", "hover")?;
            let (gx, gy) = normalized_to_global(layout, norm_x, norm_y)?;
            println!(
                "[工具调用] hover | 输入: norm_x={}, norm_y={}",
                norm_x, norm_y
            );
            crate::mouse::hover(gx, gy).map_err(|e| anyhow!(e))?;
            let output = format!(
                "已悬停归一化坐标 ({:.0}, {:.0}) -> 全局坐标 ({}, {})",
                norm_x, norm_y, gx, gy
            );
            println!("[工具结果] hover | 输出: {}", output);
            Ok(output)
        }
        "scroll" => {
            let (norm_x, norm_y) = extract_coordinates(&args, "x", "y", "scroll")?;
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
        "page_down" => {
            println!("[工具调用] page_down | 输入: (无参数)");
            crate::keyboard::press_key("space").map_err(|e| anyhow!(e))?;
            let output = "已执行向下翻页（Space）".to_string();
            println!("[工具结果] page_down | 输出: {}", output);
            Ok(output)
        }
        "page_up" => {
            println!("[工具调用] page_up | 输入: (无参数)");
            crate::keyboard::hotkey(&["shift"], "space").map_err(|e| anyhow!(e))?;
            let output = "已执行向上翻页（Shift+Space）".to_string();
            println!("[工具结果] page_up | 输出: {}", output);
            Ok(output)
        }
        "drag" => {
            let (from_x, from_y) =
                extract_coordinates(&args, "from_x", "from_y", "drag(from)")?;
            let (to_x, to_y) = extract_coordinates(&args, "to_x", "to_y", "drag(to)")?;
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
            crate::app_manager::close_application(app_name).map_err(|e| anyhow!(e.to_string()))?;
            let output = format!("已关闭应用: {}", app_name);
            println!("[工具结果] close_application | 输出: {}", output);
            Ok(output)
        }
        "focus_application" => {
            let app_name = get_str(&args, "app_name")?;
            println!("[工具调用] focus_application | 输入: app_name={}", app_name);
            crate::app_manager::focus_application(app_name).map_err(|e| anyhow!(e.to_string()))?;
            let output = format!("已聚焦应用: {}", app_name);
            println!("[工具结果] focus_application | 输出: {}", output);
            Ok(output)
        }
        "wait" => {
            let seconds: u64 = args["seconds"].as_u64().unwrap_or(3);
            let seconds = seconds.min(30);
            println!("[工具调用] wait | 等待 {} 秒", seconds);
            tokio::time::sleep(Duration::from_secs(seconds)).await;
            let output = format!("已等待 {} 秒", seconds);
            println!("[工具结果] wait | {}", output);
            Ok(output)
        }
        "ask_human" => {
            let question = args["question"].as_str().unwrap_or("需要你的帮助");
            send_notification("💬 AI 需要你的帮助", question);
            println!("[工具调用] ask_human");
            println!("🙋 AI 请求人类帮助：");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", question);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("请输入你的回复（输入完成后按回车）：");

            let mut human_input = String::new();
            std::io::stdin()
                .read_line(&mut human_input)
                .unwrap_or_default();
            let human_input = human_input.trim().to_string();

            let result = if human_input.is_empty() {
                "人类未提供回复".to_string()
            } else {
                format!("人类回复：{}", human_input)
            };
            println!("[工具结果] ask_human | {}", result);
            Ok(result)
        }
        "task_complete" => {
            let summary = get_optional_str(&args, "summary").unwrap_or("任务完成");
            println!("[工具调用] task_complete | 输入: summary={}", summary);
            let output = format!("TASK_COMPLETE: {}", summary);
            println!("[工具结果] task_complete | 输出: {}", output);
            Ok(output)
        }
        "read_clipboard" => {
            println!("[工具调用] read_clipboard");
            let output = std::process::Command::new("pbpaste").output();
            match output {
                Ok(out) => {
                    let content = String::from_utf8_lossy(&out.stdout).to_string();
                    if content.is_empty() {
                        let result = "剪贴板为空".to_string();
                        println!("[工具结果] read_clipboard | {}", result);
                        Ok(result)
                    } else {
                        println!(
                            "[工具结果] read_clipboard | 内容长度: {} 字符",
                            content.len()
                        );
                        Ok(content)
                    }
                }
                Err(e) => {
                    let result = format!("读取剪贴板失败: {}", e);
                    println!("[工具结果] read_clipboard | {}", result);
                    Ok(result)
                }
            }
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
        {"type": "function", "function": {"name": "page_down", "description": "向下翻页（模拟空格键），适用于浏览器等应用", "parameters": {"type": "object", "properties": {}, "required": []}}},
        {"type": "function", "function": {"name": "page_up", "description": "向上翻页（模拟 Shift+空格键），适用于浏览器等应用", "parameters": {"type": "object", "properties": {}, "required": []}}},
        {"type": "function", "function": {"name": "scroll", "description": "在屏幕指定坐标执行滚轮操作", "parameters": {"type": "object", "properties": {"x": {"type": "number", "description": "归一化 X 坐标 [0, 999]"}, "y": {"type": "number", "description": "归一化 Y 坐标 [0, 999]"}, "direction": {"type": "string", "description": "滚动方向: up 或 down"}, "clicks": {"type": "integer", "description": "滚动格数，默认 3"}}, "required": ["x", "y", "direction"]}}},
        {"type": "function", "function": {"name": "drag", "description": "从起点坐标拖拽到终点坐标", "parameters": {"type": "object", "properties": {"from_x": {"type": "number", "description": "起点归一化 X 坐标 [0, 999]"}, "from_y": {"type": "number", "description": "起点归一化 Y 坐标 [0, 999]"}, "to_x": {"type": "number", "description": "终点归一化 X 坐标 [0, 999]"}, "to_y": {"type": "number", "description": "终点归一化 Y 坐标 [0, 999]"}}, "required": ["from_x", "from_y", "to_x", "to_y"]}}},
        {"type": "function", "function": {"name": "type_text", "description": "输入文本内容", "parameters": {"type": "object", "properties": {"text": {"type": "string", "description": "要输入的文本"}}, "required": ["text"]}}},
        {"type": "function", "function": {"name": "press_key", "description": "按下单个按键", "parameters": {"type": "object", "properties": {"key": {"type": "string", "description": "按键名称，如 enter, tab, escape, backspace, space, up, down, left, right, f1-f12"}}, "required": ["key"]}}},
        {"type": "function", "function": {"name": "hotkey", "description": "执行组合键操作", "parameters": {"type": "object", "properties": {"modifiers": {"type": "array", "items": {"type": "string"}, "description": "修饰键数组，如 [\"command\"], [\"command\", \"shift\"]"}, "key": {"type": "string", "description": "主按键"}}, "required": ["modifiers", "key"]}}},
        {"type": "function", "function": {"name": "open_application", "description": "打开指定应用程序", "parameters": {"type": "object", "properties": {"app_name": {"type": "string", "description": "应用程序名称"}}, "required": ["app_name"]}}},
        {"type": "function", "function": {"name": "close_application", "description": "关闭指定应用程序", "parameters": {"type": "object", "properties": {"app_name": {"type": "string", "description": "应用程序名称"}}, "required": ["app_name"]}}},
        {"type": "function", "function": {"name": "focus_application", "description": "将指定应用程序聚焦到前台", "parameters": {"type": "object", "properties": {"app_name": {"type": "string", "description": "应用程序名称"}}, "required": ["app_name"]}}},
        {"type": "function", "function": {"name": "wait", "description": "等待指定秒数。用于等待页面加载、动画播放、网络请求完成等需要时间的场景。", "parameters": {"type": "object", "properties": {"seconds": {"type": "integer", "description": "等待的秒数，建议 1-30 之间"}}, "required": ["seconds"]}}},
        {"type": "function", "function": {"name": "ask_human", "description": "向人类提问或请求帮助。当你不确定如何操作、需要人类协助完成某个步骤、或需要人类提供额外信息时使用此工具。程序会暂停等待人类输入回复。", "parameters": {"type": "object", "properties": {"question": {"type": "string", "description": "向人类提出的问题或请求描述"}}, "required": ["question"]}}},
        {"type": "function", "function": {"name": "task_complete", "description": "任务完成时调用此工具", "parameters": {"type": "object", "properties": {"summary": {"type": "string", "description": "任务完成摘要"}}, "required": ["summary"]}}},
        {"type": "function", "function": {"name": "read_clipboard", "description": "读取系统剪贴板中的文本内容。用于查看用户或程序复制到剪贴板的文本。", "parameters": {"type": "object", "properties": {}, "required": []}}}
    ])
}

fn send_notification(title: &str, message: &str) {
    use notify_rust::Notification;

    if let Err(e) = Notification::new().summary(title).body(message).show() {
        eprintln!("⚠️ 发送系统通知失败: {}", e);
    }
}

fn format_app_list_section(title: &str, apps: &[String]) -> String {
    if apps.is_empty() {
        return format!("## {}\n- （未获取到应用列表）", title);
    }

    let items = apps
        .iter()
        .map(|app| format!("- {}", app))
        .collect::<Vec<_>>()
        .join("\n");

    format!("## {}\n{}", title, items)
}

fn normalized_to_global(layout: &StitchedLayout, norm_x: f64, norm_y: f64) -> Result<(i32, i32)> {
    layout
        .normalized_to_global(norm_x, norm_y)
        .map_err(|e| anyhow!("坐标转换失败: {}", e))
}

/// 从参数中提取坐标值，兼容数字和数组两种格式。
///
/// 支持以下格式：
/// - 常规：{"x": 200, "y": 50}
/// - 特殊：{"x": [200, 50]}（会将 x=200, y=50）
/// - 特殊：{"y": [200, 50]}（会将 x=200, y=50）
fn extract_coordinates(args: &Value, x_key: &str, y_key: &str, tool_name: &str) -> Result<(f64, f64)> {
    if let Some((x, y)) = extract_coordinate_pair_from_field(args, x_key) {
        println!(
            "⚠️  工具 {} 的字段 {} 返回数组格式，按坐标对解析: x={}, y={}",
            tool_name, x_key, x, y
        );
        return Ok((x, y));
    }

    if let Some((x, y)) = extract_coordinate_pair_from_field(args, y_key) {
        println!(
            "⚠️  工具 {} 的字段 {} 返回数组格式，按坐标对解析: x={}, y={}",
            tool_name, y_key, x, y
        );
        return Ok((x, y));
    }

    let x = get_f64(args, x_key)?;
    let y = get_f64(args, y_key)?;
    Ok((x, y))
}

fn extract_coordinate_pair_from_field(args: &Value, key: &str) -> Option<(f64, f64)> {
    let arr = args.get(key)?.as_array()?;
    if arr.len() < 2 {
        return None;
    }

    let x = arr.first()?.as_f64()?;
    let y = arr.get(1)?.as_f64()?;
    Some((x, y))
}

/// 从 JSON 值中提取 f64 数值，兼容单值和数组格式。
/// 单值: {"x": 184} -> 184.0
/// 数组: {"x": [184, 451]} -> 184.0 (取第一个元素)
fn extract_f64(value: &Value) -> Option<f64> {
    if let Some(n) = value.as_f64() {
        Some(n)
    } else if let Some(arr) = value.as_array() {
        arr.first().and_then(Value::as_f64)
    } else {
        None
    }
}

fn extract_f64_with_warn(value: &Value, field_name: &str) -> Option<f64> {
    if let Some(n) = value.as_f64() {
        Some(n)
    } else if let Some(arr) = value.as_array() {
        println!(
            "⚠️  字段 {} 返回了数组格式 {:?}，取第一个元素",
            field_name, arr
        );
        extract_f64(value)
    } else {
        None
    }
}

fn get_f64(value: &Value, key: &str) -> Result<f64> {
    value
        .get(key)
        .and_then(|v| extract_f64_with_warn(v, key))
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

fn merge_stream_tool_call(pending: &mut Vec<PendingToolCall>, tc: StreamToolCall) {
    if pending.len() <= tc.index {
        pending.resize_with(tc.index + 1, PendingToolCall::default);
    }

    let target = &mut pending[tc.index];

    if let Some(id) = tc.id {
        target.id = Some(id);
    }
    if let Some(call_type) = tc.r#type {
        target.call_type = Some(call_type);
    }
    if let Some(function) = tc.function {
        if let Some(name) = function.name {
            target.name = Some(name);
        }
        if let Some(arguments) = function.arguments {
            target.arguments.push_str(&arguments);
        }
    }
}

fn finalize_stream_tool_calls(pending: Vec<PendingToolCall>) -> Vec<ToolCall> {
    pending
        .into_iter()
        .enumerate()
        .filter_map(|(idx, tc)| {
            let has_any_data = tc.id.is_some()
                || tc.call_type.is_some()
                || tc.name.is_some()
                || !tc.arguments.is_empty();
            if !has_any_data {
                return None;
            }

            Some(ToolCall {
                id: tc.id.unwrap_or_else(|| format!("stream_call_{}", idx)),
                call_type: tc.call_type.unwrap_or_else(|| "function".to_string()),
                function: FunctionCall {
                    name: tc.name.unwrap_or_default(),
                    arguments: tc.arguments,
                },
            })
        })
        .collect()
}

fn build_focused_app_shortcuts_context(app_name: &str) -> Option<String> {
    let shortcuts = crate::shortcut::get_app_menu_shortcuts(app_name).ok()?;

    let mut lines = Vec::new();
    lines.push(format!("当前聚焦应用: {}", app_name));

    if shortcuts.is_empty() {
        lines.push("该应用可用的菜单栏快捷键: （未获取到）".to_string());
    } else {
        lines.push("该应用可用的菜单栏快捷键:".to_string());
        for item in shortcuts.iter() {
            let hotkey = format_shortcut_key(&item.modifiers, &item.key);
            lines.push(format!("- {}: {}", hotkey, item.menu_path));
        }
    }

    lines.push(String::new());
    lines.push("提示：控制应用时，优先使用快捷键而非鼠标点击菜单，这样更快更可靠。".to_string());

    Some(lines.join("\n"))
}

fn get_focused_app_name() -> Option<String> {
    let script = r#"tell application "System Events" to get name of first application process whose frontmost is true"#;
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let app_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if app_name.is_empty() {
        None
    } else {
        Some(app_name)
    }
}

fn format_shortcut_key(modifiers: &str, key: &str) -> String {
    let key = key.trim();
    if modifiers.trim().is_empty() {
        key.to_string()
    } else {
        format!("{}{}", modifiers.trim(), key)
    }
}

fn now_unix_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn build_chat_log_path(timestamp: &str) -> PathBuf {
    PathBuf::from("data").join(format!("chat_{}.json", timestamp))
}

fn save_chat_log(path: &Path, log: &SessionChatLog) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建数据目录失败: {}", parent.display()))?;
    }

    let content = serde_json::to_string_pretty(log).context("聊天记录序列化失败")?;
    std::fs::write(path, content)
        .with_context(|| format!("写入聊天记录文件失败: {}", path.display()))?;

    println!("💾 聊天记录已保存: {}", path.display());
    Ok(())
}

async fn save_chat_log_from_shared(
    path: &Path,
    shared_log: &Arc<Mutex<SessionChatLog>>,
) -> Result<()> {
    let snapshot = {
        let log = shared_log.lock().await;
        log.clone()
    };
    save_chat_log(path, &snapshot)
}
