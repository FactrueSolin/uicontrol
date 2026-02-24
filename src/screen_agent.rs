use anyhow::{anyhow, Result};
use rig::completion::message::{ImageDetail, ImageMediaType, UserContent};
use rig::completion::request::PromptError;
use rig::completion::{Message, Prompt};
use rig::{client::CompletionClient, OneOrMany};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use crate::ai::{get_openai_client, load_openai_config};
use crate::display::{StitchedLayout, get_stitched_layout};
use crate::screenshot;
use crate::tools::{
    ClickTool, CloseApplicationTool, DoubleClickTool, DragTool, FocusApplicationTool, HotkeyTool,
    HoverTool, ListApplicationsTool, ListRunningApplicationsTool, OpenApplicationTool,
    PressKeyTool, RightClickTool, ScrollTool, TaskCompleteTool, TypeTextTool,
};

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
- hotkey(modifiers, key) - 组合键操作（modifiers 为数组，如 ["command"], ["command", "shift"]）

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

pub struct ScreenAgent;

impl ScreenAgent {
    /// 运行屏幕控制 Agent
    /// task_goal: 用户的任务目标描述
    pub async fn run(task_goal: &str) -> Result<String> {
        let config = load_openai_config()?;
        let client = get_openai_client()?;

        // 共享的任务完成标志
        let task_completed = Arc::new(AtomicBool::new(false));
        let layout_state = Arc::new(Mutex::new(StitchedLayout {
            displays: Vec::new(),
            stitched_width: 1,
            stitched_height: 1,
            offsets: Vec::new(),
        }));

        // 构建 agent，注册全部 15 个工具
        let agent = client
            .agent(config.model_name.clone())
            .preamble(PREAMBLE)
            .default_max_turns(3)
            .additional_params(json!({"enable_thinking": false}))
            // 鼠标工具
            .tool(ClickTool {
                layout: Arc::clone(&layout_state),
            })
            .tool(RightClickTool {
                layout: Arc::clone(&layout_state),
            })
            .tool(DoubleClickTool {
                layout: Arc::clone(&layout_state),
            })
            .tool(HoverTool {
                layout: Arc::clone(&layout_state),
            })
            .tool(ScrollTool {
                layout: Arc::clone(&layout_state),
            })
            .tool(DragTool {
                layout: Arc::clone(&layout_state),
            })
            // 键盘工具
            .tool(TypeTextTool)
            .tool(PressKeyTool)
            .tool(HotkeyTool)
            // App 管理工具
            .tool(OpenApplicationTool)
            .tool(CloseApplicationTool)
            .tool(FocusApplicationTool)
            .tool(ListApplicationsTool)
            .tool(ListRunningApplicationsTool)
            // 控制工具
            .tool(TaskCompleteTool {
                completed: Arc::clone(&task_completed),
            })
            .build();

        let mut last_response = String::new();
        let mut printed_layout_diagnostics = false;

        // 外层循环：每轮截图 + 发送
        for round in 1..=MAX_ROUNDS {
            println!("--- 第 {} 轮 ---", round);

            // 1. 获取布局并进行多屏拼接截图
            let layout = get_stitched_layout().map_err(|e| anyhow!("获取屏幕布局失败: {}", e))?;

            if !printed_layout_diagnostics {
                println!("📺 屏幕布局信息：");
                for (i, display) in layout.displays.iter().enumerate() {
                    println!(
                        "  显示器 {} (id={}): {}x{} @ origin({}, {}), scale={}",
                        i + 1,
                        display.display_id,
                        display.logical_width,
                        display.logical_height,
                        display.origin_x,
                        display.origin_y,
                        display.scale_factor
                    );
                }
                println!(
                    "  拼接图尺寸: {}x{}",
                    layout.stitched_width, layout.stitched_height
                );
                for (i, offset) in layout.offsets.iter().enumerate() {
                    println!("  显示器 {} 在拼接图中的偏移: ({}, {})", i + 1, offset.0, offset.1);
                }
                printed_layout_diagnostics = true;
            }

            let (screenshot_base64_owned, stitched_layout) = screenshot::take_stitched_screenshot(&layout)
                .map_err(|e| anyhow!("多屏拼接截图失败: {}", e))?;

            {
                let mut guard = layout_state
                    .lock()
                    .map_err(|e| anyhow!("更新屏幕布局锁失败: {}", e))?;
                *guard = stitched_layout;
            }

            let screenshot_base64 = screenshot_base64_owned.as_str();

            println!("📸 拼接截图大小: {} KB", screenshot_base64.len() / 1024);

            // 2. 构建多模态消息
            let instruction = if round == 1 {
                format!(
                    "任务目标：{}\n\n请分析当前屏幕截图，执行下一步操作。",
                    task_goal
                )
            } else {
                format!(
                    "任务目标：{}\n上一轮操作结果：{}\n\n请分析当前屏幕截图，继续执行下一步操作。如果任务已完成，请调用 task_complete。",
                    task_goal, last_response
                )
            };

            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📤 [第 {} 轮] 发送给 AI 的输入：", round);
            println!("{}", instruction);
            println!("[截图已附带]");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

            let message = Message::User {
                content: OneOrMany::many(vec![
                    UserContent::text(&instruction),
                    UserContent::image_base64(
                        screenshot_base64,
                        Some(ImageMediaType::PNG),
                        Some(ImageDetail::Auto),
                    ),
                ])
                .expect("多模态消息内容不能为空"),
            };

            // 3. 调用 agent（rig 内部处理 tool call）
            let response = agent.prompt(message).await;

            match response {
                Ok(text) => {
                    println!("📥 [第 {} 轮] AI 的输出：", round);
                    println!("{}", text);
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    last_response = text;
                }
                Err(PromptError::MaxTurnsError { .. }) => {
                    // 工具已成功执行，只是 rig 内部的多轮对话超限
                    // 继续外层循环的下一轮截图即可
                    println!("⚠️  [第 {} 轮] 工具已执行，多轮对话达到上限，继续下一轮截图", round);
                    last_response = "工具已执行，等待下一轮截图确认结果".to_string();
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("MaxTurnError") || err_str.contains("max turn") {
                        println!(
                            "⚠️  [第 {} 轮] 工具已执行，多轮对话达到上限，继续下一轮截图",
                            round
                        );
                        last_response = "工具已执行，等待下一轮截图确认结果".to_string();
                    } else if err_str.contains("ApiResponse") || err_str.contains("JsonError") {
                        println!("⚠️  [第 {} 轮] API 响应解析失败: {}", round, err_str);
                        println!("⚠️  继续下一轮截图");
                        last_response = "工具已执行，等待下一轮截图确认结果".to_string();
                    } else {
                        println!("❌ [第 {} 轮] agent 调用失败: {}", round, err_str);
                        return Err(anyhow!("第 {} 轮 agent 调用失败: {}", round, err_str));
                    }
                }
            }

            // 4. 检查任务是否完成
            if task_completed.load(Ordering::SeqCst) {
                println!("✅ 任务已完成！");
                return Ok(last_response);
            }

            // 5. 等待一小段时间让 UI 更新
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        Ok(format!(
            "已达到最大轮次 {}，最后回复: {}",
            MAX_ROUNDS, last_response
        ))
    }
}
