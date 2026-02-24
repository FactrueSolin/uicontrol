use anyhow::Result;
use std::env;
use uicontrol::screen_agent_v2::ScreenAgentV2;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法：cargo run --bin screen_agent_v2 -- \"任务目标描述\"");
        std::process::exit(1);
    }

    let task_goal = &args[1];
    println!("🖥️  屏幕控制 Agent V2 启动（纯 reqwest 实现）");
    println!("📋 任务目标: {}", task_goal);

    let result = ScreenAgentV2::run(task_goal).await?;
    println!("✅ 任务已完成！");
    println!("📝 最终结果: {}", result);
    Ok(())
}

