use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ErrorData as McpError, Role, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::Deserialize;

use crate::accessibility;
use crate::app_manager;
// use crate::screenshot;

#[derive(Debug, Clone)]
pub struct AppManagerServer {
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct AppNameRequest {
    app_name: String,
}

// #[derive(Debug, Deserialize, schemars::JsonSchema)]
// struct ScreenshotRequest {
//     display_id: Option<u32>,
// }

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

    #[tool(name = "focus_app", description = "切换焦点至指定应用程序（将应用带到前台）")]
    fn focus_app(
        &self,
        Parameters(AppNameRequest { app_name }): Parameters<AppNameRequest>,
    ) -> Result<CallToolResult, McpError> {
        app_manager::focus_application(&app_name)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "已切换焦点至应用：{}",
            app_name
        ))]))
    }

    // #[tool(
    //     name = "take_screenshot",
    //     description = "截图工具：可指定 display_id 截图单屏，不传则截图所有屏幕"
    // )]
    // fn take_screenshot(
    //     &self,
    //     Parameters(ScreenshotRequest { display_id }): Parameters<ScreenshotRequest>,
    // ) -> Result<CallToolResult, McpError> {
    //     let shots = screenshot::take_screenshot(display_id)
    //         .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    //     let mut contents = Vec::with_capacity(shots.len() * 2);
    //     for shot in shots {
    //         contents.push(Content::text(shot.description.clone()));
    //         contents.push(
    //             Content::image(shot.image_base64, "image/png").with_audience(vec![Role::Assistant]),
    //         );
    //     }

    //     Ok(CallToolResult::success(contents))
    // }
}

#[tool_handler]
impl ServerHandler for AppManagerServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "应用程序管理 MCP server，提供 list_apps/list_running_apps/open_app/close_app/focus_app/get_ui_tree/take_screenshot 工具"
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
