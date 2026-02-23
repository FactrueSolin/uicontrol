use std::error::Error;
use std::process::Command;

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
