use anyhow::{Context, Result};
use rig::providers::openai;

use crate::config::AppConfig;

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
    let app_config = AppConfig::load().context("加载应用配置失败")?;

    let api_base = if app_config.api_base.trim().is_empty() {
        anyhow::bail!("缺少 OPENAI_API_BASE，请通过 `uicontrol login` 或环境变量配置")
    } else {
        app_config.api_base
    };

    let api_key = if app_config.api_key.trim().is_empty() {
        anyhow::bail!("缺少 OPENAI_API_KEY，请通过 `uicontrol login` 或环境变量配置")
    } else {
        app_config.api_key
    };

    let model_name = if app_config.model_name.trim().is_empty() {
        anyhow::bail!("缺少 OPENAI_MODEL_NAME，请通过 `uicontrol login` 或环境变量配置")
    } else {
        app_config.model_name
    };

    Ok(OpenAiConfig {
        api_base,
        api_key,
        model_name,
    })
}

pub fn get_openai_client() -> Result<openai::CompletionsClient> {
    let config = load_openai_config()?;

    let client = openai::Client::builder()
        .api_key(&config.api_key)
        .base_url(&config.api_base)
        .build()
        .context("初始化 rig OpenAI 客户端失败")?
        .completions_api();

    Ok(client)
}
