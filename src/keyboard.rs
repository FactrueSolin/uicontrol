use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// 模拟键盘输入文本
///
/// # 参数
/// * `text` - 要输入的文本字符串
///
/// # 返回
/// * `Ok(())` - 输入成功
/// * `Err(String)` - 输入失败，返回错误信息
pub fn type_text(text: &str) -> Result<(), String> {
    let settings = Settings::default();
    let mut enigo = Enigo::new(&settings).map_err(|e| format!("创建 Enigo 实例失败：{}", e))?;
    enigo.text(text)
        .map_err(|e| format!("键盘输入失败：{}", e))
}

/// 将文本写入系统剪贴板并模拟 Cmd+V 粘贴（macOS）
///
/// # 参数
/// * `text` - 需要复制并粘贴的文本
///
/// # 返回
/// * `Ok(())` - 操作成功
/// * `Err(_)` - 写入剪贴板或模拟按键失败
pub fn copy_and_paste(text: &str) -> Result<(), Box<dyn Error>> {
    // 1) 写入系统剪贴板：pbcopy 从 stdin 读取文本
    let mut pbcopy = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(stdin) = pbcopy.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    } else {
        return Err("无法获取 pbcopy stdin".into());
    }

    let pbcopy_status = pbcopy.wait()?;
    if !pbcopy_status.success() {
        return Err("pbcopy 执行失败".into());
    }

    // 2) 模拟 Cmd+V：通过 osascript 调用 System Events
    let paste_status = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke \"v\" using command down")
        .status()?;

    if !paste_status.success() {
        return Err("osascript 模拟 Cmd+V 失败".into());
    }

    Ok(())
}

fn parse_key(key: &str) -> Result<Key, String> {
    let normalized = key.trim().to_lowercase();

    let parsed = match normalized.as_str() {
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "page_up" | "page-up" => Key::PageUp,
        "pagedown" | "page_down" | "page-down" => Key::PageDown,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        _ => {
            let mut chars = normalized.chars();
            if let (Some(ch), None) = (chars.next(), chars.next()) {
                Key::Unicode(ch)
            } else {
                return Err(format!("不支持的按键：{}", key));
            }
        }
    };

    Ok(parsed)
}

fn parse_modifier(modifier: &str) -> Result<Key, String> {
    match modifier.trim().to_lowercase().as_str() {
        "cmd" | "command" => Ok(Key::Meta),
        "ctrl" | "control" => Ok(Key::Control),
        "shift" => Ok(Key::Shift),
        "alt" | "option" => Ok(Key::Alt),
        _ => Err(format!("不支持的修饰键：{}", modifier)),
    }
}

pub fn press_key(key: &str) -> Result<(), String> {
    let parsed_key = parse_key(key)?;
    let settings = Settings::default();
    let mut enigo = Enigo::new(&settings).map_err(|e| e.to_string())?;
    enigo
        .key(parsed_key, Direction::Click)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn hotkey(modifiers: &[&str], key: &str) -> Result<(), String> {
    let parsed_modifiers: Vec<Key> = modifiers
        .iter()
        .map(|m| parse_modifier(m))
        .collect::<Result<Vec<_>, _>>()?;
    let main_key = parse_key(key)?;

    let settings = Settings::default();
    let mut enigo = Enigo::new(&settings).map_err(|e| e.to_string())?;

    for modifier in &parsed_modifiers {
        enigo
            .key(*modifier, Direction::Press)
            .map_err(|e| e.to_string())?;
    }

    enigo
        .key(main_key, Direction::Click)
        .map_err(|e| e.to_string())?;

    for modifier in parsed_modifiers.iter().rev() {
        enigo
            .key(*modifier, Direction::Release)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn hold_key(key: &str, duration_ms: u64) -> Result<(), String> {
    let parsed_key = parse_key(key)?;
    let settings = Settings::default();
    let mut enigo = Enigo::new(&settings).map_err(|e| e.to_string())?;

    enigo
        .key(parsed_key, Direction::Press)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(duration_ms));
    enigo
        .key(parsed_key, Direction::Release)
        .map_err(|e| e.to_string())?;

    Ok(())
}
