use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, ImageBuffer, ImageFormat, Luma};
use qrcode::{Color, QrCode};
use std::io::Cursor;

use crate::errors::AppError;

pub fn generate_qr_base64(data: &str) -> Result<String, AppError> {
    let code = QrCode::new(data.as_bytes())
        .map_err(|_| AppError::Internal(anyhow::anyhow!("QR generation failed")))?;
    let modules = code.width();
    let scale = 8u32;
    let border = 4u32;
    let image_size = (modules as u32 + border * 2) * scale;
    let colors = code.to_colors();
    let image = ImageBuffer::from_fn(image_size, image_size, |x, y| {
        let module_x = x / scale;
        let module_y = y / scale;
        let value = if module_x < border
            || module_y < border
            || module_x >= modules as u32 + border
            || module_y >= modules as u32 + border
        {
            255
        } else {
            match colors[((module_y - border) as usize) * modules + (module_x - border) as usize] {
                Color::Dark => 0,
                Color::Light => 255,
            }
        };
        Luma([value])
    });
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageLuma8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("QR generation failed")))?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(bytes.into_inner())
    ))
}
