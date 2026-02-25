use std::error::Error;
use std::process::Command;
use std::thread;
use std::time::Duration;

/// 获取已安装的应用程序列表。
pub fn list_applications() -> Result<Vec<String>, Box<dyn Error>> {
    let output = Command::new("ls").arg("/Applications").output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("获取应用列表失败: {}", stderr.trim()).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let apps = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    Ok(apps)
}

/// 获取当前正在运行的应用程序列表。
pub fn list_running_applications() -> Result<Vec<String>, Box<dyn Error>> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(
            "tell application \"System Events\" to get name of every process whose background only is false",
        )
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("获取运行中应用失败: {}", stderr.trim()).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let apps = stdout
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    Ok(apps)
}

/// 打开指定的应用程序。
pub fn open_application(app_name: &str) -> Result<(), Box<dyn Error>> {
    let status = Command::new("open").arg("-a").arg(app_name).status()?;

    if !status.success() {
        return Err(format!("打开应用失败: {}", app_name).into());
    }

    Ok(())
}

/// 关闭指定的应用程序。
pub fn close_application(app_name: &str) -> Result<(), Box<dyn Error>> {
    let clean_name = app_name.trim_end_matches(".app");

    let output = Command::new("pkill")
        .arg("-x")
        .arg(clean_name)
        .output()?;

    if output.status.success() || output.status.code() == Some(1) {
        Ok(())
    } else {
        Err(format!("关闭应用失败: {}", app_name).into())
    }
}

/// 将指定应用切换到前台并聚焦。
pub fn focus_application(app_name: &str) -> Result<(), Box<dyn Error>> {
    let clean_name = app_name.trim().trim_end_matches(".app");
    let script = format!("tell application \"{}\" to activate", clean_name);

    let status = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .status()?;

    if !status.success() {
        return Err(format!("AppleScript activate 执行失败: {}", clean_name).into());
    }

    thread::sleep(Duration::from_millis(500));
    if is_app_focused(clean_name) {
        maximize_frontmost_window();
        return Ok(());
    }

    println!(
        "⚠️  AppleScript activate 未生效，尝试 open -a {}",
        clean_name
    );
    let open_status = Command::new("open").arg("-a").arg(clean_name).status()?;
    if !open_status.success() {
        return Err(format!("open -a 执行失败: {}", clean_name).into());
    }

    thread::sleep(Duration::from_millis(500));
    if is_app_focused(clean_name) {
        maximize_frontmost_window();
        return Ok(());
    }

    Err(format!("无法聚焦应用 '{}'，两种方式均失败", clean_name).into())
}

fn maximize_frontmost_window() {
    let script = r#"
tell application "Finder"
    set desktopBounds to bounds of window of desktop
end tell

set screenWidth to item 3 of desktopBounds
set screenHeight to item 4 of desktopBounds
set menuBarHeight to 25
set usableHeight to screenHeight - menuBarHeight

tell application "System Events"
    set frontApp to first application process whose frontmost is true
    tell frontApp
        if (count of windows) > 0 then
            tell window 1
                set position to {0, menuBarHeight}
                set size to {screenWidth, usableHeight}
            end tell
        end if
    end tell
end tell
"#;

    match Command::new("osascript").arg("-e").arg(script).output() {
        Ok(output) if !output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("⚠️  最大化窗口失败（已忽略）: {}", stderr.trim());
        }
        Err(err) => {
            println!("⚠️  最大化窗口失败（已忽略）: {}", err);
        }
        _ => {}
    }
}

fn is_app_focused(app_name: &str) -> bool {
    let output = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
        .output();

    match output {
        Ok(out) => {
            let current = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
            let target = app_name.trim().to_lowercase();
            current.contains(&target) || target.contains(&current)
        }
        Err(_) => false,
    }
}
