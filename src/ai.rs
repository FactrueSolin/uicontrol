use anyhow::{Context, Result};
use dotenv::dotenv;
use rig::providers::openai;

pub const OPENAI_API_BASE_ENV: &str = "OPENAI_API_BASE";
pub const OPENAI_API_KEY_ENV: &str = "OPENAI_API_KEY";
pub const OPENAI_MODEL_NAME_ENV: &str = "OPENAI_MODEL_NAME";

#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    pub api_base: String,
    pub api_key: String,
    pub model_name: String,
}

pub fn load_openai_config() -> Result<OpenAiConfig> {
    let _ = dotenv();

    let api_base = std::env::var(OPENAI_API_BASE_ENV)
        .context("缺少 OPENAI_API_BASE，请在 .env 中配置 OpenAI 兼容接口地址")?;
    let api_key = std::env::var(OPENAI_API_KEY_ENV)
        .context("缺少 OPENAI_API_KEY，请在 .env 中配置 API Key")?;
    let model_name = std::env::var(OPENAI_MODEL_NAME_ENV)
        .context("缺少 OPENAI_MODEL_NAME，请在 .env 中配置模型名")?;

    Ok(OpenAiConfig {
        api_base,
        api_key,
        model_name,
    })
}

pub fn get_openai_client() -> Result<openai::Client> {
    let config = load_openai_config()?;

    let client = openai::Client::builder()
        .api_key(&config.api_key)
        .base_url(&config.api_base)
        .build()
        .context("初始化 rig OpenAI 客户端失败")?;

    Ok(client)
}

