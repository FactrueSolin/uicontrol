use std::env;

use uicontrol::image_diff::compare_images;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("用法: cargo run --bin test_image_diff -- <image1_path> <image2_path>");
        std::process::exit(1);
    }

    let diff_percentage = compare_images(&args[1], &args[2])?;
    println!("差异百分比: {:.4}%", diff_percentage);

    Ok(())
}
