# CLI 程序架构与配置管理方案

## 1. CLI 命令结构设计

使用 `clap` derive API 设计命令行结构：

```
uicontrol <SUBCOMMAND>

SUBCOMMANDS:
  run <task>       运行 agent 执行任务（默认行为）
  login            交互式配置 API 凭证并持久化存储
  config show      显示当前生效的配置（脱敏显示 key）
```

当不提供子命令、直接传入任务字符串时，等价于 `run`：

```bash
# 以下两种方式等价
uicontrol "打开浏览器"
uicontrol run "打开浏览器"
```

### clap 结构定义草案

```rust
#[derive(Parser)]
#[command(name = "uicontrol", about = "macOS 屏幕控制 Agent")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 任务目标（省略子命令时直接传入）
    task: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// 运行 agent 执行任务
    Run {
        /// 任务目标描述
        task: String,
    },
    /// 交互式登录，配置 API 凭证
    Login,
    /// 配置管理
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// 显示当前生效的配置
    Show,
}
```

## 2. 配置管理方案

### 2.1 存储位置

```
~/.config/uicontrol/config.toml
```

使用 `dirs` crate 获取跨平台配置目录。macOS 上 `dirs::config_dir()` 返回 `~/Library/Application Support`，但考虑到 CLI 工具惯例，统一使用 `~/.config/uicontrol/`（通过 `dirs::home_dir()` + `.config/uicontrol/` 拼接）。

### 2.2 配置文件格式

```toml
[openai]
api_base = "https://api.openai.com/v1"
api_key = "sk-xxx"
model_name = "qwen3.5-plus"

[agent]
max_rounds = 30
```

### 2.3 配置优先级（从高到低）

```
CLI 参数（未来扩展） > 环境变量 > config.toml > .env 文件 > 默认值
```

具体加载逻辑：

1. 先加载 `.env`（dotenv，保持兼容）
2. 再加载 `~/.config/uicontrol/config.toml`
3. 环境变量覆盖 config.toml 中的值
4. 最终合并为 `AppConfig`

### 2.4 配置结构体

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub openai: OpenAiSection,
    #[serde(default)]
    pub agent: AgentSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAiSection {
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentSection {
    pub max_rounds: Option<usize>,
}
```

### 2.5 login 命令交互流程

```
$ uicontrol login

🔧 配置 uicontrol

API Base URL [https://api.openai.com/v1]: https://api.example.com/v1
API Key: sk-xxxxx
Model Name [qwen3.5-plus]: gpt-4o
Max Rounds [30]: 20

✅ 配置已保存到 ~/.config/uicontrol/config.toml
```

- 方括号内为当前值或默认值，直接回车保留
- API Key 输入时不回显（使用 `dialoguer` 的 Password 输入）
- 如果已有配置文件，读取现有值作为默认值

### 2.6 config show 输出

```
$ uicontrol config show

当前配置（来源优先级：环境变量 > config.toml > .env > 默认值）
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  API Base:    https://api.example.com/v1  [config.toml]
  API Key:     sk-xxx...xxx                [环境变量]
  Model Name:  gpt-4o                      [config.toml]
  Max Rounds:  30                          [默认值]
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
配置文件: ~/.config/uicontrol/config.toml
```

## 3. 文件结构设计

### 3.1 新增文件

| 文件 | 职责 |
|------|------|
| `src/config.rs` | 配置管理：加载、合并、持久化、交互式输入 |

### 3.2 需修改的文件

| 文件 | 修改内容 |
|------|----------|
| `src/bin/screen_agent_v2.rs` | 改造为 clap CLI，添加子命令分发 |
| `src/ai.rs` | `load_openai_config()` 改为调用 `config.rs` 的统一加载逻辑 |
| `src/screen_agent_v2.rs` | `AGENT_MAX_ROUNDS` 从 `AppConfig` 获取而非直接读 env |
| `src/lib.rs` | 添加 `pub mod config;` |
| `Cargo.toml` | 添加 clap, dirs, toml, dialoguer 依赖 |

### 3.3 修改后的调用链

```
bin/screen_agent_v2.rs (CLI 入口)
  ├── login 命令 → config::interactive_login()
  ├── config show → config::show_config()
  └── run 命令
       └── config::load_app_config()  ← 统一加载
            ├── dotenv 加载 .env
            ├── 读取 config.toml
            ├── 读取环境变量覆盖
            └── 返回 AppConfig
                 ├── ai.rs 使用 OpenAiConfig 部分
                 └── screen_agent_v2.rs 使用 max_rounds
```

## 4. 依赖建议

```toml
# Cargo.toml 新增依赖
clap = { version = "4", features = ["derive"] }
dirs = "6"
toml = "0.8"
dialoguer = "0.11"
```

| crate | 用途 |
|-------|------|
| `clap` | CLI 参数解析，derive 宏方式 |
| `dirs` | 获取用户 home 目录 |
| `toml` | config.toml 序列化/反序列化 |
| `dialoguer` | 交互式终端输入（带默认值、密码隐藏） |

## 5. 实施步骤

1. `Cargo.toml` 添加 clap, dirs, toml, dialoguer 依赖
2. 创建 `src/config.rs`，实现配置加载、保存、交互式输入
3. 修改 `src/ai.rs`，`load_openai_config()` 接受 `AppConfig` 参数或从 config 模块获取
4. 修改 `src/screen_agent_v2.rs`，`ScreenAgentV2::run()` 接受 `AppConfig` 参数
5. 改造 `src/bin/screen_agent_v2.rs` 为 clap CLI 入口
6. 修改 `src/lib.rs` 添加 config 模块导出
7. `cargo check` 验证编译通过
