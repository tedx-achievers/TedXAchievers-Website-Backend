use base64::{engine::general_purpose::STANDARD, Engine};
use image::{ImageBuffer, ImageFormat, Rgba};
use qrcode::{Color as QrColor, QrCode};
use std::io::Cursor;

use crate::errors::AppError;

pub fn generate_qr_base64(data: &str) -> Result<String, AppError> {
    let code =
        QrCode::new(data.as_bytes()).map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    let width = code.width();
    let scale = 10usize;
    let quiet = 4usize;
    let image_size = (width + quiet * 2) * scale;
    let mut image = ImageBuffer::from_pixel(
        image_size as u32,
        image_size as u32,
        Rgba([0xff_u8, 0xff_u8, 0xff_u8, 0xff_u8]),
    );
    for (index, bit) in code.to_colors().iter().enumerate() {
        if *bit != QrColor::Dark {
            continue;
        }
        let row = index / width;
        let column = index % width;
        for dy in 0..scale {
            for dx in 0..scale {
                image.put_pixel(
                    ((column + quiet) * scale + dx) as u32,
                    ((row + quiet) * scale + dy) as u32,
                    Rgba([0xe6_u8, 0x2b_u8, 0x1e_u8, 0xff_u8]),
                );
            }
        }
    }
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(bytes.into_inner())
    ))
}
