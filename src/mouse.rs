use core_graphics::display::CGDisplay;
use core_graphics::geometry::CGPoint;
use enigo::{Axis, Button, Direction, Enigo, Mouse, Settings};
use std::thread;
use std::time::Duration;

fn create_enigo() -> Result<Enigo, String> {
    Enigo::new(&Settings::default()).map_err(|e| e.to_string())
}

// 使用 macOS 原生 API 移动鼠标到全局绝对坐标
fn move_to_absolute(x: i32, y: i32) -> Result<(), String> {
    let point = CGPoint::new(x as f64, y as f64);
    CGDisplay::warp_mouse_cursor_position(point).map_err(|e| format!("移动鼠标失败: {:?}", e))
}

fn move_to(_enigo: &mut Enigo, x: i32, y: i32) -> Result<(), String> {
    move_to_absolute(x, y)?;
    // 给系统一点时间处理鼠标移动
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

pub fn right_click(x: i32, y: i32) -> Result<(), String> {
    let mut enigo = create_enigo()?;
    move_to(&mut enigo, x, y)?;
    enigo
        .button(Button::Right, Direction::Click)
        .map_err(|e| e.to_string())
}

pub fn click(x: i32, y: i32) -> Result<(), String> {
    let mut enigo = create_enigo()?;
    move_to(&mut enigo, x, y)?;
    enigo
        .button(Button::Left, Direction::Click)
        .map_err(|e| e.to_string())
}

pub fn double_click(x: i32, y: i32) -> Result<(), String> {
    let mut enigo = create_enigo()?;
    move_to(&mut enigo, x, y)?;
    enigo
        .button(Button::Left, Direction::Click)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(80));
    enigo
        .button(Button::Left, Direction::Click)
        .map_err(|e| e.to_string())
}

pub fn drag(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Result<(), String> {
    let mut enigo = create_enigo()?;
    move_to(&mut enigo, from_x, from_y)?;
    enigo
        .button(Button::Left, Direction::Press)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(50));
    move_to(&mut enigo, to_x, to_y)?;
    thread::sleep(Duration::from_millis(50));
    enigo
        .button(Button::Left, Direction::Release)
        .map_err(|e| e.to_string())
}

pub fn scroll(x: i32, y: i32, direction: &str, clicks: i32) -> Result<(), String> {
    let mut enigo = create_enigo()?;
    move_to(&mut enigo, x, y)?;

    let length = match direction {
        "up" => clicks.abs(),
        "down" => -clicks.abs(),
        _ => return Err(format!("不支持的滚动方向: {}，仅支持 up/down", direction)),
    };

    enigo
        .scroll(length, Axis::Vertical)
        .map_err(|e| e.to_string())
}

pub fn hover(x: i32, y: i32) -> Result<(), String> {
    let mut enigo = create_enigo()?;
    move_to(&mut enigo, x, y)
}

pub fn long_press(x: i32, y: i32, duration_ms: u64) -> Result<(), String> {
    let mut enigo = create_enigo()?;
    move_to(&mut enigo, x, y)?;
    enigo
        .button(Button::Left, Direction::Press)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(duration_ms));
    enigo
        .button(Button::Left, Direction::Release)
        .map_err(|e| e.to_string())
}

pub fn select_area(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Result<(), String> {
    drag(from_x, from_y, to_x, to_y)
}
