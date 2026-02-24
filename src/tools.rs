use rig::completion::request::ToolDefinition;
use rig::tool::Tool;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use thiserror::Error;

use crate::display::StitchedLayout;

#[derive(Debug, Error)]
#[error("工具执行失败: {0}")]
pub struct ToolError(pub String);

#[derive(Debug, Deserialize)]
pub struct ClickArgs {
    pub x: f64,
    pub y: f64,
}

pub struct ClickTool {
    pub layout: Arc<Mutex<StitchedLayout>>,
}

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
                    "x": { "type": "number", "description": "归一化 X 坐标，范围 [0,999]" },
                    "y": { "type": "number", "description": "归一化 Y 坐标，范围 [0,999]" }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] click | 输入: norm_x={}, norm_y={}", args.x, args.y);
        let (x, y) = map_norm_to_global(&self.layout, args.x, args.y)?;
        crate::mouse::click(x, y).map_err(ToolError)?;
        let output = format!(
            "已点击归一化坐标 ({:.2}, {:.2}) -> 全局坐标 ({}, {})",
            args.x, args.y, x, y
        );
        println!("[工具结果] click | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct RightClickArgs {
    pub x: f64,
    pub y: f64,
}

pub struct RightClickTool {
    pub layout: Arc<Mutex<StitchedLayout>>,
}

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
                    "x": { "type": "number", "description": "归一化 X 坐标，范围 [0,999]" },
                    "y": { "type": "number", "description": "归一化 Y 坐标，范围 [0,999]" }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] right_click | 输入: norm_x={}, norm_y={}", args.x, args.y);
        let (x, y) = map_norm_to_global(&self.layout, args.x, args.y)?;
        crate::mouse::right_click(x, y).map_err(ToolError)?;
        let output = format!(
            "已右键点击归一化坐标 ({:.2}, {:.2}) -> 全局坐标 ({}, {})",
            args.x, args.y, x, y
        );
        println!("[工具结果] right_click | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct DoubleClickArgs {
    pub x: f64,
    pub y: f64,
}

pub struct DoubleClickTool {
    pub layout: Arc<Mutex<StitchedLayout>>,
}

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
                    "x": { "type": "number", "description": "归一化 X 坐标，范围 [0,999]" },
                    "y": { "type": "number", "description": "归一化 Y 坐标，范围 [0,999]" }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] double_click | 输入: norm_x={}, norm_y={}", args.x, args.y);
        let (x, y) = map_norm_to_global(&self.layout, args.x, args.y)?;
        crate::mouse::double_click(x, y).map_err(ToolError)?;
        let output = format!(
            "已双击归一化坐标 ({:.2}, {:.2}) -> 全局坐标 ({}, {})",
            args.x, args.y, x, y
        );
        println!("[工具结果] double_click | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct HoverArgs {
    pub x: f64,
    pub y: f64,
}

pub struct HoverTool {
    pub layout: Arc<Mutex<StitchedLayout>>,
}

impl Tool for HoverTool {
    const NAME: &'static str = "hover";
    type Error = ToolError;
    type Args = HoverArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "将鼠标移动到屏幕指定坐标，不点击。用于触发悬停菜单、显示工具提示或激活 hover 效果"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "number", "description": "目标位置归一化 X 坐标，范围 [0,999]" },
                    "y": { "type": "number", "description": "目标位置归一化 Y 坐标，范围 [0,999]" }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[工具调用] hover | 输入：norm_x={}, norm_y={}", args.x, args.y);
        let (x, y) = map_norm_to_global(&self.layout, args.x, args.y)?;
        crate::mouse::hover(x, y).map_err(|e| ToolError(e.to_string()))?;
        let output = format!(
            "已将鼠标移动到归一化坐标 ({:.2}, {:.2}) -> 全局坐标 ({}, {})",
            args.x, args.y, x, y
        );
        println!("[工具结果] hover | 输出：{}", output);
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
    pub x: f64,
    pub y: f64,
    pub direction: String,
    pub clicks: i32,
}

pub struct ScrollTool {
    pub layout: Arc<Mutex<StitchedLayout>>,
}

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
                    "x": { "type": "number", "description": "归一化 X 坐标，范围 [0,999]" },
                    "y": { "type": "number", "description": "归一化 Y 坐标，范围 [0,999]" },
                    "direction": { "type": "string", "description": "滚动方向（up/down）" },
                    "clicks": { "type": "integer", "description": "滚动格数" }
                },
                "required": ["x", "y", "direction", "clicks"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let (x, y) = map_norm_to_global(&self.layout, args.x, args.y)?;
        println!(
            "[工具调用] scroll | 输入: norm_x={}, norm_y={}, direction=\"{}\", clicks={}",
            args.x, args.y, args.direction, args.clicks
        );
        crate::mouse::scroll(x, y, &args.direction, args.clicks).map_err(ToolError)?;
        let output = format!(
            "已在归一化坐标 ({:.2}, {:.2}) -> 全局坐标 ({}, {}) 向 {} 滚动 {} 格",
            args.x, args.y, x, y, args.direction, args.clicks
        );
        println!("[工具结果] scroll | 输出: {}", output);
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub struct DragArgs {
    pub from_x: f64,
    pub from_y: f64,
    pub to_x: f64,
    pub to_y: f64,
}

pub struct DragTool {
    pub layout: Arc<Mutex<StitchedLayout>>,
}

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
                    "from_x": { "type": "number", "description": "起点归一化 X 坐标，范围 [0,999]" },
                    "from_y": { "type": "number", "description": "起点归一化 Y 坐标，范围 [0,999]" },
                    "to_x": { "type": "number", "description": "终点归一化 X 坐标，范围 [0,999]" },
                    "to_y": { "type": "number", "description": "终点归一化 Y 坐标，范围 [0,999]" }
                },
                "required": ["from_x", "from_y", "to_x", "to_y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let (from_x, from_y) = map_norm_to_global(&self.layout, args.from_x, args.from_y)?;
        let (to_x, to_y) = map_norm_to_global(&self.layout, args.to_x, args.to_y)?;
        println!(
            "[工具调用] drag | 输入: from_norm=({:.2},{:.2}), to_norm=({:.2},{:.2})",
            args.from_x, args.from_y, args.to_x, args.to_y
        );
        crate::mouse::drag(from_x, from_y, to_x, to_y).map_err(ToolError)?;
        let output = format!(
            "已拖拽: 归一化 ({:.2}, {:.2}) -> ({:.2}, {:.2}), 全局 ({}, {}) -> ({}, {})",
            args.from_x, args.from_y, args.to_x, args.to_y, from_x, from_y, to_x, to_y
        );
        println!("[工具结果] drag | 输出: {}", output);
        Ok(output)
    }
}

fn map_norm_to_global(
    layout: &Arc<Mutex<StitchedLayout>>,
    norm_x: f64,
    norm_y: f64,
) -> Result<(i32, i32), ToolError> {
    let guard = layout
        .lock()
        .map_err(|e| ToolError(format!("布局锁获取失败: {}", e)))?;
    guard
        .normalized_to_global(norm_x, norm_y)
        .map_err(ToolError)
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
