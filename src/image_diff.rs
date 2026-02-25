use std::error::Error;

use base64::Engine;
use image::{DynamicImage, GenericImageView};

const MAX_RGB_DISTANCE: f64 = 441.6729559300637;

pub fn compare_images(image1_path: &str, image2_path: &str) -> Result<f64, Box<dyn Error>> {
    let image1 = image::open(image1_path)?;
    let image2 = image::open(image2_path)?;

    compare_dynamic_images(&image1, &image2)
}

pub fn compare_images_base64(base64_1: &str, base64_2: &str) -> Result<f64, Box<dyn Error>> {
    let bytes_1 = base64::engine::general_purpose::STANDARD.decode(base64_1)?;
    let bytes_2 = base64::engine::general_purpose::STANDARD.decode(base64_2)?;

    let image1 = image::load_from_memory(&bytes_1)?;
    let image2 = image::load_from_memory(&bytes_2)?;

    compare_dynamic_images(&image1, &image2)
}

fn compare_dynamic_images(image1: &DynamicImage, image2: &DynamicImage) -> Result<f64, Box<dyn Error>> {
    let (width, height) = image1.dimensions();

    if width == 0 || height == 0 {
        return Err("图片尺寸不能为 0".into());
    }

    let image1_rgb = image1.to_rgb8();
    let image2_resized = if image2.dimensions() != (width, height) {
        image2.resize_exact(width, height, image::imageops::FilterType::Triangle)
    } else {
        image2.clone()
    };
    let image2_rgb = image2_resized.to_rgb8();

    let mut total_normalized_diff = 0.0_f64;

    for (pixel1, pixel2) in image1_rgb.pixels().zip(image2_rgb.pixels()) {
        let dr = pixel1[0] as f64 - pixel2[0] as f64;
        let dg = pixel1[1] as f64 - pixel2[1] as f64;
        let db = pixel1[2] as f64 - pixel2[2] as f64;

        let distance = (dr * dr + dg * dg + db * db).sqrt();
        total_normalized_diff += distance / MAX_RGB_DISTANCE;
    }

    let total_pixels = (width as f64) * (height as f64);
    let average_diff_percentage = (total_normalized_diff / total_pixels) * 100.0;

    Ok(average_diff_percentage)
}
