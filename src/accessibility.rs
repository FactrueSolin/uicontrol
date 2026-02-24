use accessibility::{AXAttribute, AXUIElement, AXUIElementAttributes};
use accessibility_sys::{
    AXIsProcessTrustedWithOptions, AXValueGetTypeID, AXValueGetValue, AXValueRef,
    kAXTrustedCheckOptionPrompt, kAXValueTypeCGPoint,
};
use core_foundation::{
    base::{CFType, TCFType},
    boolean::CFBoolean,
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use serde::Serialize;
use serde_json::Value;
use std::{error::Error, ffi::OsStr, path::Path, process::Command};

const MAX_DEPTH: usize = 64;

#[derive(Debug, Serialize)]
struct UiPosition {
    x: f64,
    y: f64,
}

#[derive(Debug, Serialize)]
struct UiNode {
    role: Option<String>,
    name: Option<String>,
    value: Option<Value>,
    position: Option<UiPosition>,
    children: Vec<UiNode>,
}

#[repr(C)]
#[derive(Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

fn ensure_accessibility_permission() -> Result<(), Box<dyn Error>> {
    let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
    let trusted = unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };

    if trusted {
        Ok(())
    } else {
        Err("未获得 macOS 辅助功能权限，请在 系统设置 -> 隐私与安全性 -> 辅助功能 中授权".into())
    }
}

fn find_process_pid(app_name: &str) -> Result<i32, Box<dyn Error>> {
    let normalized = app_name.trim().trim_end_matches(".app").to_lowercase();
    let normalized_no_space = normalized.replace(' ', "");

    if normalized.is_empty() {
        return Err("应用名称不能为空".into());
    }

    let output = Command::new("ps").arg("-axo").arg("pid=,comm=").output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("读取进程列表失败: {}", stderr.trim()).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut no_space_match: Option<i32> = None;
    let mut contains_match: Option<i32> = None;

    for line in stdout.lines() {
        let mut parts = line.trim().splitn(2, char::is_whitespace);
        let pid_str = match parts.next() {
            Some(v) => v,
            None => continue,
        };
        let command_path = match parts.next() {
            Some(v) => v,
            None => continue,
        };

        let pid = match pid_str.parse::<i32>() {
            Ok(v) => v,
            Err(_) => continue,
        };

        let executable = Path::new(command_path)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or(command_path)
            .to_lowercase();
        let executable_no_space = executable.replace(' ', "");

        // 1) 精确匹配（最高优先级）
        if executable == normalized {
            return Ok(pid);
        }

        // 2) 无空格匹配
        if executable_no_space == normalized_no_space && no_space_match.is_none() {
            no_space_match = Some(pid);
        }

        // 3) 包含匹配（最低优先级）
        if executable.contains(&normalized) && contains_match.is_none() {
            contains_match = Some(pid);
        }
    }

    if let Some(pid) = no_space_match.or(contains_match) {
        return Ok(pid);
    }

    if let Some(pid) = find_pid_via_applescript(app_name) {
        return Ok(pid);
    }

    Err(format!("未找到应用进程: {}", app_name).into())
}

fn find_pid_via_applescript(app_name: &str) -> Option<i32> {
    let target = app_name.trim().trim_end_matches(".app").trim();
    if target.is_empty() {
        return None;
    }

    // AppleScript 字符串转义
    let escaped = target.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"tell application "System Events" to get unix id of process "{}""#,
        escaped
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn cf_type_to_json(value: CFType) -> Option<Value> {
    if let Some(v) = value.downcast::<CFString>() {
        return Some(Value::String(v.to_string()));
    }

    if let Some(v) = value.downcast::<CFBoolean>() {
        return Some(Value::Bool(bool::from(v)));
    }

    if let Some(v) = value.downcast::<CFNumber>() {
        if let Some(i) = v.to_i64() {
            return Some(Value::Number(i.into()));
        }

        if let Some(f) = v.to_f64() {
            if let Some(n) = serde_json::Number::from_f64(f) {
                return Some(Value::Number(n));
            }
        }
    }

    None
}

fn get_position(element: &AXUIElement) -> Option<UiPosition> {
    let position_attr = AXAttribute::<CFType>::new(&CFString::from_static_string("AXPosition"));
    let value = element.attribute(&position_attr).ok()?;

    let ax_value_type_id = unsafe { AXValueGetTypeID() };
    if value.type_of() != ax_value_type_id {
        return None;
    }

    let mut point = CGPoint::default();
    let ok = unsafe {
        AXValueGetValue(
            value.as_CFTypeRef() as AXValueRef,
            kAXValueTypeCGPoint,
            (&mut point as *mut CGPoint).cast(),
        )
    };

    if ok {
        Some(UiPosition {
            x: point.x,
            y: point.y,
        })
    } else {
        None
    }
}

fn build_ui_node(element: &AXUIElement, depth: usize) -> UiNode {
    let role = element.role().ok().map(|s| s.to_string());
    let name = element
        .title()
        .ok()
        .map(|s| s.to_string())
        .or_else(|| element.description().ok().map(|s| s.to_string()));
    let value = element.value().ok().and_then(cf_type_to_json);
    let position = get_position(element);

    eprintln!(
        "[get_ui_tree] depth={} role={}",
        depth,
        role.as_deref().unwrap_or("<unknown>")
    );

    let ax_children: Vec<AXUIElement> = if depth >= MAX_DEPTH {
        Vec::new()
    } else {
        element
            .children()
            .ok()
            .map(|arr| arr.into_iter().map(|item| (*item).clone()).collect())
            .unwrap_or_default()
    };

    eprintln!(
        "[get_ui_tree] depth={} role={} AXChildren={}",
        depth,
        role.as_deref().unwrap_or("<unknown>"),
        ax_children.len()
    );

    let children = ax_children
        .into_iter()
        .map(|child| build_ui_node(&child, depth + 1))
        .collect::<Vec<_>>();

    UiNode {
        role,
        name,
        value,
        position,
        children,
    }
}

fn collect_app_top_level_elements(app_element: &AXUIElement) -> Vec<AXUIElement> {
    let mut children: Vec<AXUIElement> = app_element
        .children()
        .ok()
        .map(|arr| arr.into_iter().map(|item| (*item).clone()).collect())
        .unwrap_or_default();
    let windows: Vec<AXUIElement> = app_element
        .windows()
        .ok()
        .map(|arr| arr.into_iter().map(|item| (*item).clone()).collect())
        .unwrap_or_default();

    eprintln!(
        "[get_ui_tree] app AXChildren={}, AXWindows={}",
        children.len(),
        windows.len()
    );

    let has_window_in_children = children.iter().any(|child| {
        child
            .role()
            .ok()
            .map(|s| s.to_string())
            .as_deref()
            == Some("AXWindow")
    });

    if !has_window_in_children && !windows.is_empty() {
        eprintln!(
            "[get_ui_tree] AXChildren 中未发现 AXWindow，追加 AXWindows 进行遍历"
        );
        children.extend(windows);
    }

    if children.is_empty() {
        eprintln!("[get_ui_tree] 未获取到任何顶层元素");
    }

    children
}

/// 获取指定应用的 UI 元素树
pub fn get_ui_tree(app_name: &str) -> Result<String, Box<dyn Error>> {
    ensure_accessibility_permission()?;

    let pid = find_process_pid(app_name)?;
    let app_element = AXUIElement::application(pid);

    let top_level = collect_app_top_level_elements(&app_element);
    if top_level.is_empty() {
        return Err("无法读取应用 UI 树（可能是权限不足或应用不支持 AX）".into());
    }

    let nodes: Vec<UiNode> = top_level
        .into_iter()
        .map(|elem| build_ui_node(&elem, 0))
        .collect();

    Ok(serde_json::to_string(&nodes)?)
}
