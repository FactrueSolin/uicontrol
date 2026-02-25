use anyhow::{Context, Result};
use dialoguer::{Input, Password};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL_NAME: &str = "qwen3.5-plus";
const DEFAULT_AGENT_MAX_ROUNDS: usize = 20;

const OPENAI_API_BASE_ENV: &str = "OPENAI_API_BASE";
const OPENAI_API_KEY_ENV: &str = "OPENAI_API_KEY";
const OPENAI_MODEL_NAME_ENV: &str = "OPENAI_MODEL_NAME";
const AGENT_MAX_ROUNDS_ENV: &str = "AGENT_MAX_ROUNDS";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub api_base: String,
    pub api_key: String,
    pub model_name: String,
    pub agent_max_rounds: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PartialAppConfig {
    api_base: Option<String>,
    api_key: Option<String>,
    model_name: Option<String>,
    agent_max_rounds: Option<usize>,
}

impl AppConfig {
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("无法获取用户 Home 目录")?;
        Ok(home.join(".config").join("uicontrol"))
    }

    pub fn config_file_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        // 1. 加载 .env（兼容历史行为）
        dotenv::dotenv().ok();

        // 2. 从 config.toml 读取基础配置
        let file_config = Self::load_partial_from_file()?;

        // 3. 环境变量覆盖 config.toml
        let api_base = std::env::var(OPENAI_API_BASE_ENV)
            .ok()
            .or(file_config.api_base)
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string());

        let api_key = std::env::var(OPENAI_API_KEY_ENV)
            .ok()
            .or(file_config.api_key)
            .unwrap_or_default();

        let model_name = std::env::var(OPENAI_MODEL_NAME_ENV)
            .ok()
            .or(file_config.model_name)
            .unwrap_or_else(|| DEFAULT_MODEL_NAME.to_string());

        let agent_max_rounds = std::env::var(AGENT_MAX_ROUNDS_ENV)
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .or(file_config.agent_max_rounds)
            .unwrap_or(DEFAULT_AGENT_MAX_ROUNDS);

        Ok(Self {
            api_base,
            api_key,
            model_name,
            agent_max_rounds,
        })
    }

    pub fn save(&self) -> Result<()> {
        let file_path = Self::config_file_path()?;
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建配置目录失败: {}", parent.display()))?;
        }

        let content = toml::to_string_pretty(self).context("序列化配置失败")?;
        std::fs::write(&file_path, content)
            .with_context(|| format!("写入配置文件失败: {}", file_path.display()))?;
        Ok(())
    }

    pub fn login_interactive() -> Result<()> {
        let current = Self::load().unwrap_or_else(|_| Self::default());

        println!("🔧 配置 uicontrol\n");

        let api_base: String = Input::new()
            .with_prompt("API Base URL")
            .default(current.api_base)
            .interact_text()
            .context("读取 API Base URL 失败")?;

        let api_key: String = Password::new()
            .with_prompt("API Key（留空则保留当前值）")
            .allow_empty_password(true)
            .interact()
            .context("读取 API Key 失败")?;

        let model_name: String = Input::new()
            .with_prompt("Model Name")
            .default(current.model_name)
            .interact_text()
            .context("读取 Model Name 失败")?;

        let max_rounds: usize = Input::new()
            .with_prompt("Max Rounds")
            .default(current.agent_max_rounds)
            .interact_text()
            .context("读取 Max Rounds 失败")?;

        let final_api_key = if api_key.trim().is_empty() {
            current.api_key
        } else {
            api_key
        };

        let new_config = AppConfig {
            api_base,
            api_key: final_api_key,
            model_name,
            agent_max_rounds: max_rounds,
        };

        new_config.save()?;
        println!("\n✅ 配置已保存到 {}", Self::config_file_path()?.display());
        Ok(())
    }

    pub fn show() -> Result<()> {
        let cfg = Self::load()?;
        println!("当前配置（来源优先级：环境变量 > config.toml > .env > 默认值）");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  API Base:    {}", cfg.api_base);
        println!("  API Key:     {}", mask_api_key(&cfg.api_key));
        println!("  Model Name:  {}", cfg.model_name);
        println!("  Max Rounds:  {}", cfg.agent_max_rounds);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("配置文件: {}", Self::config_file_path()?.display());
        Ok(())
    }

    fn load_partial_from_file() -> Result<PartialAppConfig> {
        let path = Self::config_file_path()?;
        if !path.exists() {
            return Ok(PartialAppConfig::default());
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("读取配置文件失败: {}", path.display()))?;
        let config: PartialAppConfig = toml::from_str(&content)
            .with_context(|| format!("解析 TOML 配置失败: {}", path.display()))?;
        Ok(config)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_base: DEFAULT_API_BASE.to_string(),
            api_key: String::new(),
            model_name: DEFAULT_MODEL_NAME.to_string(),
            agent_max_rounds: DEFAULT_AGENT_MAX_ROUNDS,
        }
    }
}

fn mask_api_key(key: &str) -> String {
    if key.trim().is_empty() {
        return "(未设置)".to_string();
    }

    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "****".to_string();
    }

    let prefix: String = chars.iter().take(4).collect();
    let suffix: String = chars
        .iter()
        .rev()
        .take(4)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    format!("{}...{}", prefix, suffix)
}

