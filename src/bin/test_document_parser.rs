use anyhow::{Context, Result};
use std::env;
use std::path::Path;
use std::time::Instant;
use uicontrol::ai::load_openai_config;
use uicontrol::document_parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "用法: cargo run --bin test_document_parser -- <图片路径>\n示例: cargo run --bin test_document_parser -- /path/to/screenshot.png"
        );
        std::process::exit(1);
    }

    let image_path = &args[1];
    let path = Path::new(image_path);

    if !path.exists() {
        anyhow::bail!("图片文件不存在: {}", path.display());
    }

    if !path.is_file() {
        anyhow::bail!("路径不是文件: {}", path.display());
    }

    let html_path = path.with_extension("html");
    let md_path = path.with_extension("md");
    let total_start = Instant::now();

    let openai_config = load_openai_config().context("加载 OpenAI 配置失败")?;
    println!("当前模型: {}", openai_config.model_name);

    println!("正在将图片转换为 HTML...");
    let html_start = Instant::now();
    let html = document_parser::image_to_html(image_path)
        .await
        .with_context(|| format!("图片转 HTML 失败: {}", path.display()))?;
    std::fs::write(&html_path, &html)
        .with_context(|| format!("写入 HTML 文件失败: {}", html_path.display()))?;
    println!("HTML 转换完成，耗时: {:.2}s", html_start.elapsed().as_secs_f64());
    println!("HTML 已保存到: {}", html_path.display());

    println!("正在将图片转换为 Markdown...");
    let md_start = Instant::now();
    let md = document_parser::image_to_markdown(image_path)
        .await
        .with_context(|| format!("图片转 Markdown 失败: {}", path.display()))?;
    std::fs::write(&md_path, &md)
        .with_context(|| format!("写入 Markdown 文件失败: {}", md_path.display()))?;
    println!(
        "Markdown 转换完成，耗时: {:.2}s",
        md_start.elapsed().as_secs_f64()
    );
    println!("Markdown 已保存到: {}", md_path.display());
    println!("总耗时: {:.2}s", total_start.elapsed().as_secs_f64());

    Ok(())
}
