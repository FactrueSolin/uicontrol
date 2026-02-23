use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ErrorData as McpError, ServerCapabilities, ServerInfo},
    schemars,
    tool, tool_handler, tool_router,
};
use serde::Deserialize;

use crate::accessibility;
use crate::app_manager;

#[derive(Debug, Clone)]
pub struct AppManagerServer {
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct AppNameRequest {
    app_name: String,
}

impl Default for AppManagerServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl AppManagerServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(name = "list_apps", description = "列出已安装的应用程序")]
    fn list_apps(&self) -> Result<CallToolResult, McpError> {
        let apps = app_manager::list_applications()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let text = serde_json::to_string(&apps)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "list_running_apps", description = "列出当前正在运行的应用程序")]
    fn list_running_apps(&self) -> Result<CallToolResult, McpError> {
        let apps = app_manager::list_running_applications()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let text = serde_json::to_string(&apps)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "open_app", description = "打开指定应用程序")]
    fn open_app(
        &self,
        Parameters(AppNameRequest { app_name }): Parameters<AppNameRequest>,
    ) -> Result<CallToolResult, McpError> {
        app_manager::open_application(&app_name)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "已打开应用: {}",
            app_name
        ))]))
    }

    #[tool(name = "close_app", description = "关闭指定应用程序")]
    fn close_app(
        &self,
        Parameters(AppNameRequest { app_name }): Parameters<AppNameRequest>,
    ) -> Result<CallToolResult, McpError> {
        app_manager::close_application(&app_name)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "已关闭应用: {}",
            app_name
        ))]))
    }

    #[tool(name = "get_ui_tree", description = "获取指定应用的 UI 元素树")]
    fn get_ui_tree(
        &self,
        Parameters(AppNameRequest { app_name }): Parameters<AppNameRequest>,
    ) -> Result<CallToolResult, McpError> {
        let ui_tree = accessibility::get_ui_tree(&app_name)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(ui_tree)]))
    }
}

#[tool_handler]
impl ServerHandler for AppManagerServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "应用程序管理 MCP server，提供 list_apps/list_running_apps/open_app/close_app/get_ui_tree 工具"
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
