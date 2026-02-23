mod keyboard;

use keyboard::type_text;

fn main() {
    println!("UIControl - 系统控制工具");
    
    // 示例：模拟键盘输入
    if let Err(e) = type_text("Hello, World!") {
        eprintln!("错误：{}", e);
    }
}
