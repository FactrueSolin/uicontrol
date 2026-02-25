use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::io::{self, Write};
use uicontrol::config::AppConfig;
use uicontrol::screen_agent_v2::{ScreenAgentV2, TaskResult};
use uicontrol::session::{Session, SessionToolCall};

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
    /// 运行 agent（支持会话恢复 + 交互式 REPL）
    Run {
        /// 任务目标描述
        #[arg(long)]
        task: Option<String>,

        /// 恢复指定会话 ID
        #[arg(long)]
        resume: Option<String>,

        /// 覆盖最大轮次（等价设置 AGENT_MAX_ROUNDS）
        #[arg(long)]
        max_rounds: Option<usize>,
    },

    /// 会话管理
    Sessions {
        #[command(subcommand)]
        action: Option<SessionsAction>,
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

#[derive(Debug, Subcommand)]
enum SessionsAction {
    /// 删除指定会话
    Delete { session_id: String },
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
        Some(Commands::Run {
            task,
            resume,
            max_rounds,
        }) => {
            run_interactive(task.or(cli.task), resume, max_rounds).await?;
        }
        Some(Commands::Sessions { action }) => {
            handle_sessions_command(action)?;
        }
        None => {
            run_interactive(cli.task, None, None).await?;
        }
    }

    Ok(())
}

async fn run_interactive(
    initial_task: Option<String>,
    resume_id: Option<String>,
    max_rounds: Option<usize>,
) -> Result<()> {
    if let Some(n) = max_rounds {
        std::env::set_var("AGENT_MAX_ROUNDS", n.to_string());
        println!("⚙️ 已设置最大轮次 AGENT_MAX_ROUNDS={}", n);
    }

    println!("🖥️  qcu 屏幕控制 Agent 启动（会话 + REPL）");

    let agent = ScreenAgentV2;

    let (mut session, mut messages) = if let Some(id) = resume_id {
        let loaded = Session::load(&id).map_err(|e| anyhow!(e.to_string()))?;
        let msg_count = loaded.messages.len();
        let api_messages = session_to_api_messages(&loaded);
        println!(
            "📂 已恢复会话 {}（标题：{}，历史消息：{}）",
            short_session_id(&loaded.id),
            loaded.title,
            msg_count
        );
        (loaded, api_messages)
    } else {
        let s = Session::new();
        println!("🆕 新会话已创建：{}", short_session_id(&s.id));
        (s, Vec::new())
    };

    if let Some(task) = initial_task {
        run_one_task(&agent, &task, &mut messages, &mut session).await?;
    }

    repl_loop(&agent, &mut session, &mut messages).await
}

async fn repl_loop(
    agent: &ScreenAgentV2,
    session: &mut Session,
    messages: &mut Vec<Value>,
) -> Result<()> {
    print_repl_help();

    loop {
        print!(
            "\n📋 会话 {} | 输入新任务（输入 /quit 退出，/help 查看帮助）:\n> ",
            short_session_id(&session.id)
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        match input {
            "/quit" | "/exit" => {
                session.save().map_err(|e| anyhow!(e.to_string()))?;
                println!("👋 已退出");
                break;
            }
            "/help" => {
                print_repl_help();
            }
            "/new" => {
                session.save().map_err(|e| anyhow!(e.to_string()))?;
                *session = Session::new();
                messages.clear();
                println!("🆕 已切换到新会话：{}", short_session_id(&session.id));
            }
            "/sessions" => {
                print_sessions_list()?;
            }
            _ if input.starts_with("/resume ") => {
                let id = input.trim_start_matches("/resume").trim();
                if id.is_empty() {
                    println!("⚠️ 用法：/resume <session_id>");
                    continue;
                }

                session.save().map_err(|e| anyhow!(e.to_string()))?;
                let loaded = Session::load(id).map_err(|e| anyhow!(e.to_string()))?;
                *messages = session_to_api_messages(&loaded);
                *session = loaded;

                println!(
                    "📂 已切换到会话 {}（标题：{}，历史消息：{}）",
                    short_session_id(&session.id),
                    session.title,
                    session.messages.len()
                );
            }
            task => {
                run_one_task(agent, task, messages, session).await?;
            }
        }
    }

    Ok(())
}

async fn run_one_task(
    agent: &ScreenAgentV2,
    task_goal: &str,
    messages: &mut Vec<Value>,
    session: &mut Session,
) -> Result<()> {
    println!("📋 任务目标: {}", task_goal);
    let result = agent
        .run_task(task_goal, messages, session)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;

    match result {
        TaskResult::Completed(summary) => {
            println!("✅ 任务已完成！");
            println!("📝 最终结果: {}", summary);
        }
        TaskResult::MaxRoundsReached(max_rounds) => {
            println!("⏸️ 达到最大轮次 {}，任务暂停", max_rounds);
        }
        TaskResult::UserInterrupt => {
            println!("⚠️ 用户中断任务");
        }
    }

    Ok(())
}

fn handle_sessions_command(action: Option<SessionsAction>) -> Result<()> {
    match action {
        None => {
            print_sessions_list()?;
        }
        Some(SessionsAction::Delete { session_id }) => {
            Session::delete(&session_id).map_err(|e| anyhow!(e.to_string()))?;
            println!("🗑️ 已删除会话: {}", session_id);
        }
    }
    Ok(())
}

fn print_sessions_list() -> Result<()> {
    let sessions = Session::list_all().map_err(|e| anyhow!(e.to_string()))?;

    if sessions.is_empty() {
        println!("📭 暂无历史会话");
        return Ok(());
    }

    println!("📚 历史会话列表（按更新时间倒序）");
    println!(
        "{:<36} | {:<20} | {:<24} | {:<24} | {:>6}",
        "ID", "标题", "创建时间", "更新时间", "消息数"
    );
    println!("{}", "-".repeat(122));

    for s in sessions {
        println!(
            "{:<36} | {:<20} | {:<24} | {:<24} | {:>6}",
            s.id,
            truncate_text(&s.title, 20),
            s.created_at,
            s.updated_at,
            s.message_count
        );
    }

    Ok(())
}

fn print_repl_help() {
    println!("\n📖 REPL 命令：");
    println!("  /help                 显示帮助");
    println!("  /quit | /exit         退出程序");
    println!("  /new                  开始新会话（清空上下文）");
    println!("  /sessions             列出历史会话");
    println!("  /resume <session_id>  切换到历史会话");
    println!("  其他任意输入           作为新任务执行");
}

fn short_session_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn truncate_text(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }

    let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn session_to_api_messages(session: &Session) -> Vec<Value> {
    session
        .messages
        .iter()
        .map(|m| {
            let content_value = if m.content.is_empty() {
                Value::Null
            } else {
                Value::String(m.content.clone())
            };

            match m.role.as_str() {
                "assistant" => {
                    if let Some(tool_calls) = &m.tool_calls {
                        json!({
                            "role": "assistant",
                            "content": content_value,
                            "tool_calls": tool_calls_to_api(tool_calls)
                        })
                    } else {
                        json!({
                            "role": "assistant",
                            "content": content_value
                        })
                    }
                }
                "tool" => json!({
                    "role": "tool",
                    "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
                    "content": content_value
                }),
                _ => json!({
                    "role": m.role,
                    "content": content_value
                }),
            }
        })
        .collect()
}

fn tool_calls_to_api(calls: &[SessionToolCall]) -> Vec<Value> {
    calls
        .iter()
        .map(|tc| {
            json!({
                "id": tc.id,
                "type": "function",
                "function": {
                    "name": tc.name,
                    "arguments": tc.arguments
                }
            })
        })
        .collect()
}
