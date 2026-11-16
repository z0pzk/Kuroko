use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{imageops::FilterType, DynamicImage, ImageFormat};
use std::io::Cursor;
use xcap::Window;

#[derive(Debug, Clone)]
pub struct WindowBounds {
    pub title: String,
    pub app_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub bounds: WindowBounds,
    pub jpeg_base64: String,
}

pub fn find_target_window(query: &str) -> Result<WindowBounds> {
    let q = query.trim().to_lowercase();
    let windows = Window::all().map_err(|e| anyhow!("Window enumeration failed: {e}"))?;

    for win in windows {
        let is_minimized = win.is_minimized().unwrap_or(false);
        if is_minimized {
            continue;
        }
        let title = win.title().unwrap_or_default();
        let app_name = win.app_name().unwrap_or_default();
        let width = win.width().unwrap_or(0);
        let height = win.height().unwrap_or(0);

        if width < 200 || height < 200 {
            continue;
        }

        let title_l = title.to_lowercase();
        let app_l = app_name.to_lowercase();
        if title_l.contains(&q) || app_l.contains(&q) {
            return Ok(WindowBounds {
                title,
                app_name,
                x: win.x().unwrap_or(0),
                y: win.y().unwrap_or(0),
                width,
                height,
            });
        }
    }

    Err(anyhow!(
        "Target window matching '{}' not found (ensure Brave is open and not minimized)",
        query
    ))
}

pub fn capture_target_window(query: &str) -> Result<CapturedFrame> {
    let q = query.trim().to_lowercase();
    let windows = Window::all().map_err(|e| anyhow!("Window enumeration failed: {e}"))?;

    for win in windows {
        if win.is_minimized().unwrap_or(false) {
            continue;
        }
        let title = win.title().unwrap_or_default();
        let app_name = win.app_name().unwrap_or_default();
        let width = win.width().unwrap_or(0);
        let height = win.height().unwrap_or(0);
        if width < 200 || height < 200 {
            continue;
        }

        if title.to_lowercase().contains(&q) || app_name.to_lowercase().contains(&q) {
            let bounds = WindowBounds {
                title,
                app_name,
                x: win.x().unwrap_or(0),
                y: win.y().unwrap_or(0),
                width,
                height,
            };

            let rgba = win
                .capture_image()
                .map_err(|e| anyhow!("Screen capture failed: {e}"))?;
            let dyn_img = DynamicImage::ImageRgba8(rgba);

            // Downscale to max 1280px wide to keep token usage & latency low
            let resized = if dyn_img.width() > 1280 {
                let target_h = (dyn_img.height() as f32 * (1280.0 / dyn_img.width() as f32)) as u32;
                dyn_img.resize(1280, target_h, FilterType::Triangle)
            } else {
                dyn_img
            };

            let rgb = resized.to_rgb8();
            let mut buf = Cursor::new(Vec::new());
            DynamicImage::ImageRgb8(rgb)
                .write_to(&mut buf, ImageFormat::Jpeg)
                .map_err(|e| anyhow!("JPEG encode failed: {e}"))?;

            let jpeg_base64 = STANDARD.encode(buf.into_inner());
            return Ok(CapturedFrame {
                bounds,
                jpeg_base64,
            });
        }
    }

    Err(anyhow!("Brave window not found for capture"))
}
