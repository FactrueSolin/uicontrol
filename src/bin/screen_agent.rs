use anyhow::{Context, Result};
use std::env;

use uicontrol::screen_agent::ScreenAgent;

#[tokio::main]
async fn main() -> Result<()> {
    // 从命令行参数获取任务目标
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("用法: cargo run --bin screen_agent -- \"任务目标描述\"");
        eprintln!("示例: cargo run --bin screen_agent -- \"打开Safari浏览器并访问baidu.com\"");
        std::process::exit(1);
    }

    let task_goal = &args[1];

    println!("🖥️  屏幕控制 Agent 启动");
    println!("📋 任务目标: {}", task_goal);
    println!();

    let result = ScreenAgent::run(task_goal)
        .await
        .context("Agent 执行失败")?;

    println!();
    println!("📝 最终结果: {}", result);

    Ok(())
}
