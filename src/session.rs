use std::error::Error;
use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SESSIONS_DIR: &str = "data/sessions";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<SessionMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<SessionToolCall>>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
}

impl Session {
    pub fn new() -> Self {
        let now = Utc::now().to_rfc3339();

        Self {
            id: Uuid::new_v4().to_string(),
            title: "新会话".to_string(),
            created_at: now.clone(),
            updated_at: now,
            messages: Vec::new(),
        }
    }

    pub fn add_message(&mut self, msg: SessionMessage) {
        if self.title == "新会话" && msg.role == "user" {
            let trimmed = msg.content.trim();
            if !trimmed.is_empty() {
                self.title = trimmed.chars().take(50).collect();
            }
        }

        self.messages.push(msg);
        self.updated_at = Utc::now().to_rfc3339();
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let dir = PathBuf::from(SESSIONS_DIR);
        fs::create_dir_all(&dir)?;

        let file_path = dir.join(format!("{}.json", self.id));
        let json = serde_json::to_string_pretty(self)?;
        fs::write(file_path, json)?;

        Ok(())
    }

    pub fn load(id: &str) -> Result<Self, Box<dyn Error>> {
        let file_path = PathBuf::from(SESSIONS_DIR).join(format!("{}.json", id));
        let content = fs::read_to_string(file_path)?;
        let session: Session = serde_json::from_str(&content)?;
        Ok(session)
    }

    pub fn list_all() -> Result<Vec<SessionSummary>, Box<dyn Error>> {
        let dir = PathBuf::from(SESSIONS_DIR);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            let session: Session = serde_json::from_str(&content)?;

            summaries.push(SessionSummary {
                id: session.id,
                title: session.title,
                created_at: session.created_at,
                updated_at: session.updated_at,
                message_count: session.messages.len(),
            });
        }

        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(summaries)
    }

    pub fn delete(id: &str) -> Result<(), Box<dyn Error>> {
        let file_path = PathBuf::from(SESSIONS_DIR).join(format!("{}.json", id));

        match fs::remove_file(file_path) {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(Box::new(err)),
        }
    }
}

/// 从 API 消息格式（serde_json::Value）转换为 SessionMessage
/// 过滤掉图片 base64，只保留文本
pub fn api_message_to_session_message(api_msg: &serde_json::Value) -> SessionMessage {
    let role = api_msg
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let content = extract_text_content(api_msg.get("content"));

    let tool_calls = api_msg
        .get("tool_calls")
        .and_then(|v| v.as_array())
        .map(|calls| {
            calls
                .iter()
                .map(|call| {
                    let id = call
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let function = call.get("function");
                    let name = function
                        .and_then(|f| f.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let arguments = function
                        .and_then(|f| f.get("arguments"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    SessionToolCall {
                        id,
                        name,
                        arguments,
                    }
                })
                .collect::<Vec<_>>()
        });

    let tool_call_id = api_msg
        .get("tool_call_id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);

    SessionMessage {
        role,
        content,
        tool_calls,
        tool_call_id,
    }
}

fn extract_text_content(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(items)) => {
            let mut parts = Vec::new();

            for item in items {
                let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");

                match item_type {
                    "text" => {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            if !text.trim().is_empty() {
                                parts.push(text.to_string());
                            }
                        }
                    }
                    "image_url" | "input_image" => {
                        parts.push("[screenshot]".to_string());
                    }
                    _ => {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            if !text.trim().is_empty() {
                                parts.push(text.to_string());
                            }
                        }
                    }
                }
            }

            parts.join("\n")
        }
        _ => String::new(),
    }
}
