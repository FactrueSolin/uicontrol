use enigo::{Enigo, Keyboard, Settings};
use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

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
