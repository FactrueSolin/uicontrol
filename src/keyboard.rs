use enigo::{Enigo, Keyboard, Settings};

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
