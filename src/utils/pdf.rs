use base64::{engine::general_purpose::STANDARD, Engine};
use image::io::Reader as ImageReader;
use printpdf::{Color, Image, ImageTransform, Line, Mm, PdfDocument, Point, Polygon, Rgb};
use std::io::{BufWriter, Cursor};

use crate::errors::AppError;

pub fn generate_ticket_pdf(
    attendee_name: &str,
    attendee_email: &str,
    ticket_code: &str,
    qr_base64: &str,
    tier: &str,
    amount: &str,
    event_name: &str,
    event_theme: &str,
    event_date: &str,
    event_time: &str,
    event_venue: &str,
    purchase_date: &str,
) -> Result<Vec<u8>, AppError> {
    let (document, page, layer) = PdfDocument::new(event_name, Mm(210.0), Mm(297.0), "Ticket");
    let layer = document.get_page(page).get_layer(layer);
    let font = document
        .add_builtin_font(printpdf::BuiltinFont::Helvetica)
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    let white = Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None));
    let muted = Color::Rgb(Rgb::new(0.55, 0.55, 0.55, None));
    let footer_muted = Color::Rgb(Rgb::new(0.65, 0.65, 0.65, None));
    let red = Color::Rgb(Rgb::new(0.9, 0.08, 0.04, None));
    layer.set_fill_color(Color::Rgb(Rgb::new(0.02, 0.02, 0.02, None)));
    layer.add_polygon(rectangle(Mm(0.0), Mm(0.0), Mm(210.0), Mm(297.0)));
    layer.set_fill_color(red.clone());
    layer.add_polygon(rectangle(Mm(0.0), Mm(292.0), Mm(210.0), Mm(5.0)));
    layer.add_polygon(rectangle(Mm(0.0), Mm(0.0), Mm(210.0), Mm(5.0)));
    layer.set_fill_color(Color::Rgb(Rgb::new(0.06, 0.06, 0.06, None)));
    layer.add_polygon(rectangle(Mm(0.0), Mm(252.0), Mm(210.0), Mm(40.0)));
    layer.set_fill_color(red.clone());
    layer.add_polygon(rectangle(Mm(20.0), Mm(249.5), Mm(170.0), Mm(1.5)));

    let logo_bytes = include_bytes!("../../logo-white.png");
    let logo_image = ImageReader::new(Cursor::new(&logo_bytes[..]))
        .with_guessed_format()
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
        .decode()
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    Image::from_dynamic_image(&logo_image).add_to_layer(
        layer.clone(),
        ImageTransform {
            translate_x: Some(Mm(24.0)),
            translate_y: Some(Mm(258.0)),
            dpi: Some(72.0),
            scale_x: Some(0.19),
            scale_y: Some(0.19),
            ..Default::default()
        },
    );
    layer.set_fill_color(muted.clone());
    layer.use_text(event_theme, 8.0, Mm(24.0), Mm(254.0), &font);
    layer.set_fill_color(white.clone());
    layer.use_text(attendee_name, 22.0, Mm(20.0), Mm(241.0), &font);
    layer.set_fill_color(muted.clone());
    layer.use_text(attendee_email, 9.0, Mm(20.0), Mm(233.0), &font);

    layer.set_fill_color(Color::Rgb(Rgb::new(0.08, 0.08, 0.08, None)));
    layer.add_polygon(rectangle(Mm(20.0), Mm(170.0), Mm(170.0), Mm(62.0)));
    draw_line(
        &layer,
        Mm(20.0),
        Mm(232.0),
        Mm(190.0),
        Mm(232.0),
        muted.clone(),
    );
    draw_line(
        &layer,
        Mm(20.0),
        Mm(212.0),
        Mm(190.0),
        Mm(212.0),
        muted.clone(),
    );
    draw_line(
        &layer,
        Mm(20.0),
        Mm(194.0),
        Mm(190.0),
        Mm(194.0),
        muted.clone(),
    );
    draw_line(
        &layer,
        Mm(100.0),
        Mm(232.0),
        Mm(100.0),
        Mm(212.0),
        muted.clone(),
    );
    draw_line(
        &layer,
        Mm(80.0),
        Mm(212.0),
        Mm(80.0),
        Mm(194.0),
        muted.clone(),
    );
    draw_line(
        &layer,
        Mm(130.0),
        Mm(212.0),
        Mm(130.0),
        Mm(194.0),
        muted.clone(),
    );

    layer.set_fill_color(muted.clone());
    layer.use_text("TICKET CODE", 7.0, Mm(25.0), Mm(224.0), &font);
    layer.set_fill_color(white.clone());
    let ted_part = if ticket_code.len() >= 4 {
        &ticket_code[..4]
    } else {
        ticket_code
    };
    let rest_part = if ticket_code.len() >= 4 {
        &ticket_code[4..]
    } else {
        ""
    };
    layer.set_fill_color(red.clone());
    layer.use_text(ted_part, 12.0, Mm(25.0), Mm(217.0), &font);
    layer.set_fill_color(white.clone());
    layer.use_text(rest_part, 12.0, Mm(37.0), Mm(217.0), &font);
    layer.set_fill_color(muted.clone());
    layer.use_text("TICKET TIER", 7.0, Mm(105.0), Mm(224.0), &font);
    layer.set_fill_color(white.clone());
    layer.use_text(tier, 12.0, Mm(105.0), Mm(217.0), &font);
    layer.set_fill_color(muted.clone());
    layer.use_text("AMOUNT PAID", 7.0, Mm(25.0), Mm(208.0), &font);
    layer.use_text("DATE", 7.0, Mm(85.0), Mm(208.0), &font);
    layer.use_text("TIME", 7.0, Mm(135.0), Mm(208.0), &font);
    layer.set_fill_color(white.clone());
    layer.use_text(amount, 9.0, Mm(25.0), Mm(201.0), &font);
    layer.use_text(event_date, 8.0, Mm(85.0), Mm(201.0), &font);
    layer.use_text(event_time, 8.0, Mm(135.0), Mm(201.0), &font);
    layer.set_fill_color(muted.clone());
    layer.use_text("VENUE", 7.0, Mm(25.0), Mm(191.0), &font);
    layer.set_fill_color(white.clone());
    layer.use_text(event_venue, 8.0, Mm(25.0), Mm(184.0), &font);
    let encoded = match qr_base64.strip_prefix("data:image/png;base64,") {
        Some(value) => value,
        None => qr_base64,
    };
    let qr_bytes = STANDARD
        .decode(encoded)
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    let qr_image = ImageReader::new(Cursor::new(qr_bytes))
        .with_guessed_format()
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
        .decode()
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    layer.set_fill_color(white.clone());
    layer.add_polygon(rectangle(Mm(67.0), Mm(95.0), Mm(76.0), Mm(72.0)));
    let qr_scale: f32 = 0.62;
    let qr_size_mm = (f64::from(qr_image.width()) / 72.0) * 25.4 * f64::from(qr_scale);
    let qr_x = (67.0 + (76.0 - qr_size_mm) / 2.0) as f32;
    let qr_y = (95.0 + (72.0 - qr_size_mm) / 2.0) as f32;
    Image::from_dynamic_image(&qr_image).add_to_layer(
        layer.clone(),
        ImageTransform {
            translate_x: Some(Mm(qr_x)),
            translate_y: Some(Mm(qr_y)),
            dpi: Some(72.0),
            scale_x: Some(qr_scale),
            scale_y: Some(qr_scale),
            ..Default::default()
        },
    );
    layer.set_fill_color(muted.clone());
    layer.use_text(
        "SCAN THIS QR CODE AT THE ENTRANCE",
        8.0,
        Mm(74.0),
        Mm(90.0),
        &font,
    );
    layer.set_fill_color(white.clone());
    layer.set_fill_color(red.clone());
    let ted_part = if ticket_code.len() >= 4 {
        &ticket_code[..4]
    } else {
        ticket_code
    };
    let rest_part = if ticket_code.len() >= 4 {
        &ticket_code[4..]
    } else {
        ""
    };
    layer.set_fill_color(red.clone());
    layer.use_text(ted_part, 14.0, Mm(85.0), Mm(82.0), &font);
    layer.set_fill_color(white.clone());
    layer.use_text(rest_part, 14.0, Mm(98.0), Mm(82.0), &font);
    layer.set_fill_color(footer_muted.clone());
    layer.set_fill_color(footer_muted);
    layer.use_text(
        &format!("Purchase date: {purchase_date}"),
        9.0,
        Mm(20.0),
        Mm(18.0),
        &font,
    );
    layer.use_text(
        "tedxachieversuniversity.com.ng",
        9.0,
        Mm(137.0),
        Mm(18.0),
        &font,
    );
    let mut output = BufWriter::new(Vec::new());
    document
        .save(&mut output)
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    output
        .into_inner()
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}

fn draw_line(layer: &printpdf::PdfLayerReference, x1: Mm, y1: Mm, x2: Mm, y2: Mm, color: Color) {
    layer.set_outline_color(color);
    layer.add_line(Line {
        points: vec![(Point::new(x1, y1), false), (Point::new(x2, y2), false)],
        is_closed: false,
    });
}

fn rectangle(x: Mm, y: Mm, width: Mm, height: Mm) -> Polygon {
    Polygon {
        rings: vec![vec![
            (Point::new(x, y), false),
            (Point::new(Mm(x.0 + width.0), y), false),
            (Point::new(Mm(x.0 + width.0), Mm(y.0 + height.0)), false),
            (Point::new(x, Mm(y.0 + height.0)), false),
        ]],
        ..Default::default()
    }
}
