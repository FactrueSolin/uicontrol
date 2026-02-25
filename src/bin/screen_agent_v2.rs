use anyhow::Result;
use clap::{Parser, Subcommand};
use uicontrol::config::AppConfig;
use uicontrol::screen_agent_v2::ScreenAgentV2;

#[derive(Debug, Parser)]
#[command(name = "qcu", about = "qcu 屏幕控制 Agent")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 任务目标（省略子命令时直接传入）
    task: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// 运行 agent 执行任务
    Run {
        /// 任务目标描述
        task: Option<String>,
    },
    /// 交互式登录，配置 API 凭证
    Login,
    /// 配置管理
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    /// 显示当前生效的配置
    Show,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Login) => {
            AppConfig::login_interactive()?;
        }
        Some(Commands::Config {
            action: ConfigAction::Show,
        }) => {
            AppConfig::show()?;
        }
        Some(Commands::Run { task }) => {
            let task_goal = task.or(cli.task).unwrap_or_else(default_task_goal);
            run_agent(&task_goal).await?;
        }
        None => {
            let task_goal = cli.task.unwrap_or_else(default_task_goal);
            run_agent(&task_goal).await?;
        }
    }

    Ok(())
}

async fn run_agent(task_goal: &str) -> Result<()> {
    println!("🖥️  qcu 屏幕控制 Agent 启动（纯 reqwest 实现）");
    println!("📋 任务目标: {}", task_goal);

    let result = ScreenAgentV2::run(task_goal).await?;
    println!("✅ 任务已完成！");
    println!("📝 最终结果: {}", result);
    Ok(())
}

fn default_task_goal() -> String {
    "请描述要执行的任务，例如：qcu run \"打开浏览器并访问 example.com\"".to_string()
}
