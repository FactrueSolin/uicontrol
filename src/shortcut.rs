use core_foundation::{
    base::{CFType, TCFType},
    boolean::CFBoolean,
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use core_foundation_sys::{
    array::{CFArrayGetCount, CFArrayGetTypeID, CFArrayGetValueAtIndex, CFArrayRef},
    base::{Boolean, CFGetTypeID, CFRelease, CFRetain, CFTypeID, CFTypeRef},
    dictionary::CFDictionaryRef,
    string::CFStringRef,
};
use core_graphics::event::CGEventFlags;
use serde::Serialize;
use std::{error::Error, ffi::OsStr, os::raw::c_void, path::Path, process::Command};

type AXUIElementRef = *const c_void;
type AXError = i32;
type PidT = i32;

const AX_ERROR_SUCCESS: AXError = 0;
const AX_ERROR_NO_VALUE: AXError = -25212;
const MAX_DEPTH: usize = 64;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: PidT) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementGetTypeID() -> CFTypeID;

    fn AXIsProcessTrustedWithOptions(the_dict: CFDictionaryRef) -> Boolean;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[derive(Debug, Clone, Serialize)]
pub struct ShortcutItem {
    pub menu_path: String,
    pub modifiers: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalShortcutItem {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub modifiers: String,
    pub key_code: i64,
}

struct OwnedAXElement {
    raw: AXUIElementRef,
}

impl OwnedAXElement {
    fn from_create_rule(raw: AXUIElementRef) -> Option<Self> {
        if raw.is_null() {
            None
        } else {
            Some(Self { raw })
        }
    }

    fn from_get_rule(raw: AXUIElementRef) -> Option<Self> {
        if raw.is_null() {
            return None;
        }

        unsafe {
            CFRetain(raw.cast());
        }

        Some(Self { raw })
    }

    fn as_raw(&self) -> AXUIElementRef {
        self.raw
    }
}

impl Drop for OwnedAXElement {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.raw.cast());
        }
    }
}

fn ensure_accessibility_permission() -> Result<(), Box<dyn Error>> {
    let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
    let trusted = unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };

    if trusted != 0 {
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

        if executable == normalized {
            return Ok(pid);
        }

        if executable_no_space == normalized_no_space && no_space_match.is_none() {
            no_space_match = Some(pid);
        }

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

fn copy_attribute_value(element: AXUIElementRef, attribute: &str) -> Option<CFType> {
    let attr = CFString::new(attribute);
    let mut value: CFTypeRef = std::ptr::null();
    let error = unsafe {
        AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value as *mut _)
    };

    if error == AX_ERROR_NO_VALUE || value.is_null() {
        return None;
    }

    if error != AX_ERROR_SUCCESS {
        return None;
    }

    Some(unsafe { CFType::wrap_under_create_rule(value) })
}

fn get_title(element: AXUIElementRef) -> Option<String> {
    let value = copy_attribute_value(element, "AXTitle")?;
    value.downcast::<CFString>().map(|s| s.to_string())
}

fn get_cmd_char(element: AXUIElementRef) -> Option<String> {
    let value = copy_attribute_value(element, "AXMenuItemCmdChar")?;
    value.downcast::<CFString>().map(|s| s.to_string())
}

fn get_cmd_modifiers(element: AXUIElementRef) -> Option<i64> {
    let value = copy_attribute_value(element, "AXMenuItemCmdModifiers")?;
    value
        .downcast::<CFNumber>()
        .and_then(|n| n.to_i64().or_else(|| n.to_f64().map(|f| f as i64)))
}

fn get_children(element: AXUIElementRef) -> Vec<OwnedAXElement> {
    let value = match copy_attribute_value(element, "AXChildren") {
        Some(v) => v,
        None => return Vec::new(),
    };

    let value_ref = value.as_CFTypeRef();
    let array_type_id = unsafe { CFArrayGetTypeID() };
    if unsafe { CFGetTypeID(value_ref) } != array_type_id {
        return Vec::new();
    }

    let array_ref = value_ref as CFArrayRef;
    let count = unsafe { CFArrayGetCount(array_ref) };

    let ax_type_id = unsafe { AXUIElementGetTypeID() };
    (0..count)
        .filter_map(|idx| {
            let item_ref = unsafe { CFArrayGetValueAtIndex(array_ref, idx) } as AXUIElementRef;
            if item_ref.is_null() {
                return None;
            }

            if unsafe { CFGetTypeID(item_ref.cast()) } != ax_type_id {
                return None;
            }

            OwnedAXElement::from_get_rule(item_ref)
        })
        .collect()
}

fn modifiers_to_symbols(raw: i64) -> String {
    let mut flags = CGEventFlags::CGEventFlagCommand;
    if raw & 1 != 0 {
        flags |= CGEventFlags::CGEventFlagShift;
    }
    if raw & 2 != 0 {
        flags |= CGEventFlags::CGEventFlagAlternate;
    }
    if raw & 4 != 0 {
        flags |= CGEventFlags::CGEventFlagControl;
    }

    let mut symbols = String::new();
    if flags.contains(CGEventFlags::CGEventFlagCommand) {
        symbols.push('⌘');
    }
    if flags.contains(CGEventFlags::CGEventFlagShift) {
        symbols.push('⇧');
    }
    if flags.contains(CGEventFlags::CGEventFlagAlternate) {
        symbols.push('⌥');
    }
    if flags.contains(CGEventFlags::CGEventFlagControl) {
        symbols.push('⌃');
    }
    symbols
}

fn global_modifiers_to_symbols(mask: i64) -> String {
    let mut symbols = String::new();
    if mask & 1_048_576 != 0 {
        symbols.push('⌘');
    }
    if mask & 131_072 != 0 {
        symbols.push('⇧');
    }
    if mask & 524_288 != 0 {
        symbols.push('⌥');
    }
    if mask & 262_144 != 0 {
        symbols.push('⌃');
    }
    if mask & 8_388_608 != 0 {
        symbols.push('🌐');
    }
    if mask & 65_536 != 0 {
        symbols.push('⇪');
    }
    symbols
}

fn symbolic_hotkey_name(id: i64) -> String {
    match id {
        7 => "切换输入法（上一个）".to_string(),
        8 => "切换输入法（下一个）".to_string(),
        27 => "截取屏幕并保存文件".to_string(),
        28 => "截取选定区域并保存文件".to_string(),
        29 => "截取屏幕到剪贴板".to_string(),
        30 => "截取选定区域到剪贴板".to_string(),
        31 => "截取窗口到剪贴板".to_string(),
        32 => "Mission Control".to_string(),
        33 => "应用程序窗口".to_string(),
        34 => "显示桌面".to_string(),
        35 => "启动屏幕保护程序".to_string(),
        64 => "Spotlight".to_string(),
        65 => "Finder 搜索窗口".to_string(),
        79 => "向左移动一个空间".to_string(),
        80 => "向右移动一个空间".to_string(),
        118 => "切换到桌面 1".to_string(),
        119 => "切换到桌面 2".to_string(),
        120 => "切换到桌面 3".to_string(),
        121 => "切换到桌面 4".to_string(),
        160 => "显示 Launchpad".to_string(),
        161 => "显示通知中心".to_string(),
        _ => format!("系统快捷键 #{}", id),
    }
}

pub fn get_global_shortcuts() -> Result<Vec<GlobalShortcutItem>, Box<dyn Error>> {
    let output = Command::new("defaults")
        .arg("export")
        .arg("com.apple.symbolichotkeys")
        .arg("-")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("读取 com.apple.symbolichotkeys 失败: {}", stderr.trim()).into());
    }

    let mut convert = Command::new("plutil")
        .arg("-convert")
        .arg("json")
        .arg("-o")
        .arg("-")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    if let Some(stdin) = convert.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(&output.stdout)?;
    }

    let json_output = convert.wait_with_output()?;
    if !json_output.status.success() {
        let stderr = String::from_utf8_lossy(&json_output.stderr);
        return Err(format!("plutil 转换 JSON 失败: {}", stderr.trim()).into());
    }

    let value: serde_json::Value = serde_json::from_slice(&json_output.stdout)?;
    let hotkeys = value
        .get("AppleSymbolicHotKeys")
        .and_then(|v| v.as_object())
        .ok_or("未找到 AppleSymbolicHotKeys")?;

    let mut result = Vec::new();
    for (id_str, item) in hotkeys {
        let id = match id_str.parse::<i64>() {
            Ok(v) => v,
            Err(_) => continue,
        };

        let enabled = item
            .get("enabled")
            .and_then(|v| v.as_i64())
            .map(|v| v != 0)
            .unwrap_or(false);

        let params = item
            .get("value")
            .and_then(|v| v.get("parameters"))
            .and_then(|v| v.as_array());

        let key_code = params
            .and_then(|arr| arr.get(1))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);

        let modifier_mask = params
            .and_then(|arr| arr.get(2))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        result.push(GlobalShortcutItem {
            id,
            name: symbolic_hotkey_name(id),
            enabled,
            modifiers: global_modifiers_to_symbols(modifier_mask),
            key_code,
        });
    }

    result.sort_by_key(|x| x.id);
    Ok(result)
}

fn maybe_collect_shortcut(element: AXUIElementRef, menu_path: &[String], output: &mut Vec<ShortcutItem>) {
    let key = get_cmd_char(element)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if key.is_empty() {
        return;
    }

    let raw_mod = get_cmd_modifiers(element).unwrap_or(0);
    let modifiers = modifiers_to_symbols(raw_mod);
    let menu_path = menu_path.join(" > ");

    output.push(ShortcutItem {
        menu_path,
        modifiers,
        key,
    });
}

fn traverse_menu(element: AXUIElementRef, path: &mut Vec<String>, depth: usize, output: &mut Vec<ShortcutItem>) {
    if depth > MAX_DEPTH {
        return;
    }

    let title = get_title(element)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let pushed = title.is_some();

    if let Some(t) = title {
        path.push(t);
        maybe_collect_shortcut(element, path, output);
    }

    for child in get_children(element) {
        traverse_menu(child.as_raw(), path, depth + 1, output);
    }

    if pushed {
        let _ = path.pop();
    }
}

pub fn get_app_menu_shortcuts(app_name: &str) -> Result<Vec<ShortcutItem>, Box<dyn Error>> {
    ensure_accessibility_permission()?;

    let pid = find_process_pid(app_name)?;
    let app = OwnedAXElement::from_create_rule(unsafe { AXUIElementCreateApplication(pid) })
        .ok_or("创建 AXUIElement 失败")?;

    let menu_bar = copy_attribute_value(app.as_raw(), "AXMenuBar")
        .and_then(|v| {
            let ax_type_id = unsafe { AXUIElementGetTypeID() };
            if v.type_of() != ax_type_id {
                return None;
            }
            OwnedAXElement::from_get_rule(v.as_CFTypeRef().cast())
        })
        .ok_or("无法读取应用菜单栏（可能应用无菜单栏、未前台激活或无辅助功能权限）")?;

    let mut result = Vec::new();
    let mut path = Vec::new();
    for child in get_children(menu_bar.as_raw()) {
        traverse_menu(child.as_raw(), &mut path, 0, &mut result);
    }

    Ok(result)
}
