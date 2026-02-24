use rig::completion::request::ToolDefinition;
use rig::tool::Tool;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("工具执行失败: {0}")]
pub struct ToolError(pub String);

#[derive(Debug, Deserialize)]
pub struct ClickArgs {
    pub x: i32,
    pub y: i32,
}

pub struct ClickTool;

impl Tool for ClickTool {
    const NAME: &'static str = "click";
    type Error = ToolError;
    type Args = ClickArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "在屏幕指定坐标执行鼠标左键单击".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer", "description": "屏幕 X 坐标" },
                    "y": { "type": "integer", "description": "屏幕 Y 坐标" }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] click | 输入: x={}, y={}", args.x, args.y);
        crate::mouse::click(args.x, args.y).map_err(ToolError)?;
        let output = format!("已点击坐标 ({}, {})", args.x, args.y);
        println!("[工具结果] click | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct RightClickArgs {
    pub x: i32,
    pub y: i32,
}

pub struct RightClickTool;

impl Tool for RightClickTool {
    const NAME: &'static str = "right_click";
    type Error = ToolError;
    type Args = RightClickArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "在屏幕指定坐标执行鼠标右键点击".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer", "description": "屏幕 X 坐标" },
                    "y": { "type": "integer", "description": "屏幕 Y 坐标" }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] right_click | 输入: x={}, y={}", args.x, args.y);
        crate::mouse::right_click(args.x, args.y).map_err(ToolError)?;
        let output = format!("已右键点击坐标 ({}, {})", args.x, args.y);
        println!("[工具结果] right_click | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct DoubleClickArgs {
    pub x: i32,
    pub y: i32,
}

pub struct DoubleClickTool;

impl Tool for DoubleClickTool {
    const NAME: &'static str = "double_click";
    type Error = ToolError;
    type Args = DoubleClickArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "在屏幕指定坐标执行鼠标双击".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer", "description": "屏幕 X 坐标" },
                    "y": { "type": "integer", "description": "屏幕 Y 坐标" }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] double_click | 输入: x={}, y={}", args.x, args.y);
        crate::mouse::double_click(args.x, args.y).map_err(ToolError)?;
        let output = format!("已双击坐标 ({}, {})", args.x, args.y);
        println!("[工具结果] double_click | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct TypeTextArgs {
    pub text: String,
}

pub struct TypeTextTool;

impl Tool for TypeTextTool {
    const NAME: &'static str = "type_text";
    type Error = ToolError;
    type Args = TypeTextArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "模拟键盘输入指定文本".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "要输入的文本" }
                },
                "required": ["text"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] type_text | 输入: text=\"{}\"", args.text);
        crate::keyboard::type_text(&args.text).map_err(ToolError)?;
        let output = format!("已输入文本: {}", args.text);
        println!("[工具结果] type_text | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct PressKeyArgs {
    pub key: String,
}

pub struct PressKeyTool;

impl Tool for PressKeyTool {
    const NAME: &'static str = "press_key";
    type Error = ToolError;
    type Args = PressKeyArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "模拟按下并释放单个按键。支持: enter, tab, escape, space, backspace, delete, up, down, left, right, home, end, pageup, pagedown, f1-f12，以及单个字符".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "按键名称" }
                },
                "required": ["key"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] press_key | 输入: key=\"{}\"", args.key);
        crate::keyboard::press_key(&args.key).map_err(ToolError)?;
        let output = format!("已按下按键: {}", args.key);
        println!("[工具结果] press_key | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct HotkeyArgs {
    pub modifiers: Vec<String>,
    pub key: String,
}

pub struct HotkeyTool;

impl Tool for HotkeyTool {
    const NAME: &'static str = "hotkey";
    type Error = ToolError;
    type Args = HotkeyArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "模拟组合键操作。modifiers 支持: cmd/command, ctrl/control, shift, alt/option。例如复制是 modifiers=[\"cmd\"], key=\"c\"".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "modifiers": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "修饰键列表"
                    },
                    "key": { "type": "string", "description": "主按键" }
                },
                "required": ["modifiers", "key"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "[工具调用] hotkey | 输入: modifiers={:?}, key=\"{}\"",
            args.modifiers, args.key
        );
        let mod_refs: Vec<&str> = args.modifiers.iter().map(|s| s.as_str()).collect();
        crate::keyboard::hotkey(&mod_refs, &args.key).map_err(ToolError)?;
        let output = format!(
            "已执行组合键: {} + {}",
            args.modifiers.join("+"),
            args.key
        );
        println!("[工具结果] hotkey | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct ScrollArgs {
    pub x: i32,
    pub y: i32,
    pub direction: String,
    pub clicks: i32,
}

pub struct ScrollTool;

impl Tool for ScrollTool {
    const NAME: &'static str = "scroll";
    type Error = ToolError;
    type Args = ScrollArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "在屏幕指定坐标执行滚轮操作。direction 支持 up/down，clicks 为滚动格数".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer", "description": "屏幕 X 坐标" },
                    "y": { "type": "integer", "description": "屏幕 Y 坐标" },
                    "direction": { "type": "string", "description": "滚动方向（up/down）" },
                    "clicks": { "type": "integer", "description": "滚动格数" }
                },
                "required": ["x", "y", "direction", "clicks"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "[工具调用] scroll | 输入: x={}, y={}, direction=\"{}\", clicks={}",
            args.x, args.y, args.direction, args.clicks
        );
        crate::mouse::scroll(args.x, args.y, &args.direction, args.clicks).map_err(ToolError)?;
        let output = format!(
            "已在 ({}, {}) 向 {} 滚动 {} 格",
            args.x, args.y, args.direction, args.clicks
        );
        println!("[工具结果] scroll | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct DragArgs {
    pub from_x: i32,
    pub from_y: i32,
    pub to_x: i32,
    pub to_y: i32,
}

pub struct DragTool;

impl Tool for DragTool {
    const NAME: &'static str = "drag";
    type Error = ToolError;
    type Args = DragArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "从起点坐标拖拽到终点坐标".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "from_x": { "type": "integer", "description": "起点 X 坐标" },
                    "from_y": { "type": "integer", "description": "起点 Y 坐标" },
                    "to_x": { "type": "integer", "description": "终点 X 坐标" },
                    "to_y": { "type": "integer", "description": "终点 Y 坐标" }
                },
                "required": ["from_x", "from_y", "to_x", "to_y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "[工具调用] drag | 输入: from=({},{}), to=({},{})",
            args.from_x, args.from_y, args.to_x, args.to_y
        );
        crate::mouse::drag(args.from_x, args.from_y, args.to_x, args.to_y).map_err(ToolError)?;
        let output = format!(
            "已拖拽: ({}, {}) -> ({}, {})",
            args.from_x, args.from_y, args.to_x, args.to_y
        );
        println!("[工具结果] drag | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct OpenApplicationArgs {
    pub app_name: String,
}

pub struct OpenApplicationTool;

impl Tool for OpenApplicationTool {
    const NAME: &'static str = "open_application";
    type Error = ToolError;
    type Args = OpenApplicationArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "打开指定的 macOS 应用程序".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "app_name": { "type": "string", "description": "应用程序名称" }
                },
                "required": ["app_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "[工具调用] open_application | 输入: app_name=\"{}\"",
            args.app_name
        );
        crate::app_manager::open_application(&args.app_name).map_err(|e| ToolError(e.to_string()))?;
        let output = format!("已打开应用: {}", args.app_name);
        println!("[工具结果] open_application | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct CloseApplicationArgs {
    pub app_name: String,
}

pub struct CloseApplicationTool;

impl Tool for CloseApplicationTool {
    const NAME: &'static str = "close_application";
    type Error = ToolError;
    type Args = CloseApplicationArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "关闭指定的应用程序".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "app_name": { "type": "string", "description": "要关闭的应用程序名称" }
                },
                "required": ["app_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "[工具调用] close_application | 输入: app_name=\"{}\"",
            args.app_name
        );
        crate::app_manager::close_application(&args.app_name)
            .map_err(|e| ToolError(e.to_string()))?;
        let output = format!("已关闭应用: {}", args.app_name);
        println!("[工具结果] close_application | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct FocusApplicationArgs {
    pub app_name: String,
}

pub struct FocusApplicationTool;

impl Tool for FocusApplicationTool {
    const NAME: &'static str = "focus_application";
    type Error = ToolError;
    type Args = FocusApplicationArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "将指定应用程序切换到前台并聚焦".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "app_name": { "type": "string", "description": "应用程序名称" }
                },
                "required": ["app_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "[工具调用] focus_application | 输入: app_name=\"{}\"",
            args.app_name
        );
        crate::app_manager::focus_application(&args.app_name)
            .map_err(|e| ToolError(e.to_string()))?;
        let output = format!("已聚焦应用: {}", args.app_name);
        println!("[工具结果] focus_application | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct ListApplicationsArgs {}

pub struct ListApplicationsTool;

impl Tool for ListApplicationsTool {
    const NAME: &'static str = "list_applications";
    type Error = ToolError;
    type Args = ListApplicationsArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "列出系统中已安装的应用程序".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] list_applications | 输入: (无参数)");
        let apps = crate::app_manager::list_applications().map_err(|e| ToolError(e.to_string()))?;
        let output = serde_json::to_string(&apps).map_err(|e| ToolError(e.to_string()))?;
        println!("[工具结果] list_applications | 输出: {} 个应用", apps.len());
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct ListRunningApplicationsArgs {}

pub struct ListRunningApplicationsTool;

impl Tool for ListRunningApplicationsTool {
    const NAME: &'static str = "list_running_applications";
    type Error = ToolError;
    type Args = ListRunningApplicationsArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "列出当前正在运行的应用程序".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] list_running_applications | 输入: (无参数)");
        let apps = crate::app_manager::list_running_applications()
            .map_err(|e| ToolError(e.to_string()))?;
        let output = serde_json::to_string(&apps).map_err(|e| ToolError(e.to_string()))?;
        println!(
            "[工具结果] list_running_applications | 输出: {} 个应用",
            apps.len()
        );
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct TaskCompleteArgs {
    pub summary: String,
}

pub struct TaskCompleteTool {
    pub completed: Arc<AtomicBool>,
}

impl Tool for TaskCompleteTool {
    const NAME: &'static str = "task_complete";
    type Error = ToolError;
    type Args = TaskCompleteArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "当任务目标已完成时调用此工具，传入完成摘要".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "任务完成摘要" }
                },
                "required": ["summary"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] task_complete | 输入: summary=\"{}\"", args.summary);
        self.completed.store(true, Ordering::SeqCst);
        let output = format!("任务已完成: {}", args.summary);
        println!("[工具结果] task_complete | 输出: {}", output);
        Ok(output)
    }
}
