use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose};
use image::imageops::{FilterType, overlay};
use image::{DynamicImage, ImageFormat, RgbaImage};

use crate::display::StitchedLayout;

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

/// 按给定布局对所有屏幕截图进行拼接，返回 (PNG base64, 布局副本)
pub fn take_stitched_screenshot(
    layout: &StitchedLayout,
) -> Result<(String, StitchedLayout), Box<dyn Error>> {
    if layout.displays.is_empty() {
        return Err("布局中没有显示器信息".into());
    }
    if layout.stitched_width == 0 || layout.stitched_height == 0 {
        return Err("布局拼接尺寸无效".into());
    }

    let mut canvas = RgbaImage::new(layout.stitched_width, layout.stitched_height);

    for (index, display) in layout.displays.iter().enumerate() {
        // 注意：screencapture -D 接收的是从 1 开始的顺序编号，
        // 而不是 CoreGraphics 的 display_id。
        let capture_id = (index as u32) + 1;
        let tmp_file = build_temp_file_path(capture_id, "stitched");
        capture_single_display_to_file(capture_id, &tmp_file)?;

        let mut image = image::open(&tmp_file)?;
        let _ = fs::remove_file(&tmp_file);

        if image.width() != display.logical_width || image.height() != display.logical_height {
            image = image.resize_exact(
                display.logical_width,
                display.logical_height,
                FilterType::Lanczos3,
            );
        }

        let (ox, oy) = layout
            .offsets
            .get(index)
            .copied()
            .ok_or("布局 offsets 与 displays 长度不一致")?;

        overlay(&mut canvas, &image.to_rgba8(), ox as i64, oy as i64);
    }

    let mut png_bytes = Vec::new();
    DynamicImage::ImageRgba8(canvas).write_to(
        &mut std::io::Cursor::new(&mut png_bytes),
        ImageFormat::Png,
    )?;
    let encoded = general_purpose::STANDARD.encode(png_bytes);

    Ok((encoded, layout.clone()))
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

    capture_single_display_to_file(display_id, &tmp_file)?;

    let image_bytes = fs::read(&tmp_file)?;
    fs::remove_file(&tmp_file)?;

    let encoded = general_purpose::STANDARD.encode(image_bytes);
    Ok(encoded)
}

fn capture_single_display_to_file(display_id: u32, tmp_file: &PathBuf) -> Result<(), Box<dyn Error>> {
    
    let output = Command::new("screencapture")
        .arg("-x")
        .arg(format!("-D{}", display_id))
        .arg(tmp_file)
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

    Ok(())
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
