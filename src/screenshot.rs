use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Clone)]
pub struct ScreenshotImage {
    pub description: String,
    pub uri: String,
    pub image_base64: String,
}

/// 获取当前系统可用的显示器编号（与 screencapture -D 参数一致）。
pub fn list_displays() -> Result<Vec<u32>, Box<dyn Error>> {
    let mut display_ids = Vec::new();

    // screencapture 的显示器编号通常从 1 开始。
    // 这里通过探测方式获取有效编号，避免引入额外图形框架依赖。
    for display_id in 1..=16 {
        if can_capture_display(display_id)? {
            display_ids.push(display_id);
        }
    }

    Ok(display_ids)
}

/// 对指定屏幕或所有屏幕进行截图，并返回 base64 编码数据。
pub fn take_screenshot(display_id: Option<u32>) -> Result<Vec<ScreenshotImage>, Box<dyn Error>> {
    let target_displays = if let Some(id) = display_id {
        vec![id]
    } else {
        list_displays()?
    };

    if target_displays.is_empty() {
        return Err("未发现可用显示器，无法截图".into());
    }

    let mut results = Vec::with_capacity(target_displays.len());

    for id in target_displays {
        let image_base64 = capture_single_display(id)?;
        results.push(ScreenshotImage {
            description: format!("显示器 {} 的截图", id),
            uri: format!("screenshot://display/{}", id),
            image_base64,
        });
    }

    Ok(results)
}

fn can_capture_display(display_id: u32) -> Result<bool, Box<dyn Error>> {
    let tmp_file = build_temp_file_path(display_id, "probe");
    let output = Command::new("screencapture")
        .arg("-x")
        .arg(format!("-D{}", display_id))
        .arg(&tmp_file)
        .output()?;

    let ok = output.status.success() && tmp_file.exists();
    let _ = fs::remove_file(&tmp_file);
    Ok(ok)
}

fn capture_single_display(display_id: u32) -> Result<String, Box<dyn Error>> {
    let tmp_file = build_temp_file_path(display_id, "capture");

    let output = Command::new("screencapture")
        .arg("-x")
        .arg(format!("-D{}", display_id))
        .arg(&tmp_file)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "对显示器 {} 截图失败: {}",
            display_id,
            stderr.trim()
        )
        .into());
    }

    let image_bytes = fs::read(&tmp_file)?;
    fs::remove_file(&tmp_file)?;

    let encoded = general_purpose::STANDARD.encode(image_bytes);
    Ok(encoded)
}

fn build_temp_file_path(display_id: u32, tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "uicontrol_screenshot_{}_{}_{}_{}.png",
        display_id,
        tag,
        std::process::id(),
        nanos
    ))
}
