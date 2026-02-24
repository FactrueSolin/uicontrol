use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rig::completion::message::{ImageDetail, ImageMediaType, UserContent};
use rig::completion::{Message, Prompt};
use rig::{OneOrMany, client::CompletionClient};

use crate::ai::{get_openai_client, load_openai_config};
use crate::screenshot;
use crate::tools::{
    CloseApplicationTool,
    FocusApplicationTool,
    ListApplicationsTool,
    ListRunningApplicationsTool,
    OpenApplicationTool,
    TaskCompleteTool,
};

const MAX_ROUNDS: usize = 20;

const PREAMBLE: &str = r#"你是一个 macOS 应用管理助手。你可以看到用户的屏幕截图，并通过工具来管理应用程序完成用户的任务。

## 可用工具
- open_application(app_name) - 打开指定应用程序
- close_application(app_name) - 关闭指定应用程序
- focus_application(app_name) - 将指定应用切换到前台并聚焦
- list_applications() - 列出系统中已安装的应用程序
- list_running_applications() - 列出当前正在运行的应用程序
- task_complete(summary) - 任务完成时调用，传入完成摘要

## 工作方式
1. 每轮你会收到当前屏幕截图和任务指令
2. 分析任务需求，选择合适的工具执行
3. 任务完成后必须调用 task_complete
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

        // 构建 agent，注册应用管理相关工具
        let agent = client
            .agent(config.model_name.clone())
            .preamble(PREAMBLE)
            .default_max_turns(5)
            .tool(OpenApplicationTool)
            .tool(CloseApplicationTool)
            .tool(FocusApplicationTool)
            .tool(ListApplicationsTool)
            .tool(ListRunningApplicationsTool)
            .tool(TaskCompleteTool {
                completed: Arc::clone(&task_completed),
            })
            .build();

        let mut last_response = String::new();

        // 外层循环：每轮截图 + 发送
        for round in 1..=MAX_ROUNDS {
            println!("--- 第 {} 轮 ---", round);

            // 1. 截取当前屏幕
            let screenshots = screenshot::take_screenshot(Some(1))
                .map_err(|e| anyhow!("截图失败: {}", e))?;

            let screenshot_base64 = screenshots
                .first()
                .ok_or_else(|| anyhow!("未获取到任何截图"))?
                .image_base64
                .as_str();

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

            // 3. 打印发送给模型的上下文摘要（方便调试）
            println!(
                "[上下文] 指令: {}...",
                &instruction.chars().take(80).collect::<String>()
            );
            println!("[上下文] 截图: {} bytes base64", screenshot_base64.len());

            // 4. 调用 agent（rig 内部处理 tool call）
            let response = agent
                .prompt(message)
                .await
                .with_context(|| format!("第 {} 轮 agent 调用失败", round))?;

            println!("[模型回复] {}", response);
            last_response = response;

            // 5. 检查任务是否完成
            if task_completed.load(Ordering::SeqCst) {
                println!("✅ 任务已完成！");
                return Ok(last_response);
            }

            // 6. 等待一小段时间让 UI 更新
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        Ok(format!(
            "已达到最大轮次 {}，最后回复: {}",
            MAX_ROUNDS, last_response
        ))
    }
}
