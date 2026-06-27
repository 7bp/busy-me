use image::{Rgba, RgbaImage};
use tray_icon::Icon;

pub fn create_icon(color: [u8; 3]) -> Icon {
    let size = 48u32;
    let mut img = RgbaImage::new(size, size);
    let center = (size as f32 / 2.0, size as f32 / 2.0);
    let outer_r = size as f32 / 2.0 - 1.0;
    let inner_r = outer_r * 0.55;

    for x in 0..size {
        for y in 0..size {
            let dx = x as f32 - center.0;
            let dy = y as f32 - center.1;
            let dist = (dx * dx + dy * dy).sqrt();

            let px = if dist <= outer_r && dist >= inner_r {
                let edge = 1.0;
                let alpha = if dist < inner_r + edge {
                    ((dist - inner_r).max(0.0).min(1.0) * 255.0) as u8
                } else if dist > outer_r - edge {
                    ((outer_r - dist).max(0.0).min(1.0) * 255.0) as u8
                } else {
                    255
                };
                Rgba([color[0], color[1], color[2], alpha])
            } else {
                Rgba([0, 0, 0, 0])
            };
            img.put_pixel(x, y, px);
        }
    }

    let rgba = img.into_raw();
    Icon::from_rgba(rgba, size, size).expect("failed to create tray icon")
}
