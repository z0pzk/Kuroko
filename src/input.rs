use crate::vision::WindowBounds;
use anyhow::{anyhow, Result};
use enigo::{
    Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings,
};
use rand::Rng;
use std::thread;
use std::time::Duration;

pub struct InputController {
    enigo: Enigo,
    humanize: bool,
}

impl InputController {
    pub fn new(humanize: bool) -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow!("Failed to initialize OS input controller: {e}"))?;
        Ok(Self { enigo, humanize })
    }

    pub fn map_normalized(bounds: &WindowBounds, nx: f32, ny: f32) -> (i32, i32) {
        let cx = nx.clamp(0.0, 1000.0) / 1000.0;
        let cy = ny.clamp(0.0, 1000.0) / 1000.0;
        let px = bounds.x + (cx * bounds.width as f32).round() as i32;
        let py = bounds.y + (cy * bounds.height as f32).round() as i32;
        (px, py)
    }

    pub fn move_to(&mut self, target_x: i32, target_y: i32) -> Result<()> {
        if !self.humanize {
            self.enigo
                .move_mouse(target_x, target_y, Coordinate::Abs)
                .map_err(|e| anyhow!("Mouse move failed: {e}"))?;
            return Ok(());
        }

        let (start_x, start_y) = self.enigo.location().unwrap_or((target_x, target_y));
        let mut rng = rand::rng();

        let mid_x = (start_x + target_x) as f32 / 2.0 + rng.random_range(-35.0..35.0);
        let mid_y = (start_y + target_y) as f32 / 2.0 + rng.random_range(-35.0..35.0);

        let steps = rng.random_range(14..24);
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let inv = 1.0 - t;
            let x = inv * inv * start_x as f32
                + 2.0 * inv * t * mid_x
                + t * t * target_x as f32;
            let y = inv * inv * start_y as f32
                + 2.0 * inv * t * mid_y
                + t * t * target_y as f32;

            let _ = self
                .enigo
                .move_mouse(x.round() as i32, y.round() as i32, Coordinate::Abs);
            thread::sleep(Duration::from_millis(rng.random_range(6..14)));
        }

        Ok(())
    }

    pub fn click_at(&mut self, bounds: &WindowBounds, nx: f32, ny: f32, double: bool) -> Result<(i32, i32)> {
        let (px, py) = Self::map_normalized(bounds, nx, ny);
        self.move_to(px, py)?;
        thread::sleep(Duration::from_millis(45));

        self.enigo
            .button(Button::Left, Direction::Click)
            .map_err(|e| anyhow!("Click failed: {e}"))?;

        if double {
            thread::sleep(Duration::from_millis(75));
            self.enigo
                .button(Button::Left, Direction::Click)
                .map_err(|e| anyhow!("Double click failed: {e}"))?;
        }
        Ok((px, py))
    }

    pub fn type_text(&mut self, text: &str) -> Result<()> {
        if !self.humanize {
            self.enigo
                .text(text)
                .map_err(|e| anyhow!("Text input failed: {e}"))?;
            return Ok(());
        }

        let mut rng = rand::rng();
        let mut buf = [0u8; 4];
        for ch in text.chars() {
            let s = ch.encode_utf8(&mut buf);
            self.enigo
                .text(s)
                .map_err(|e| anyhow!("Char input failed: {e}"))?;
            thread::sleep(Duration::from_millis(rng.random_range(18..55)));
        }
        Ok(())
    }

    pub fn press_key(&mut self, key_name: &str) -> Result<()> {
        let key = match key_name.to_lowercase().as_str() {
            "enter" | "return" => Key::Return,
            "tab" => Key::Tab,
            "escape" | "esc" => Key::Escape,
            "backspace" => Key::Backspace,
            "space" => Key::Space,
            "up" => Key::UpArrow,
            "down" => Key::DownArrow,
            "left" => Key::LeftArrow,
            "right" => Key::RightArrow,
            "pagedown" => Key::PageDown,
            "pageup" => Key::PageUp,
            other => {
                if let Some(ch) = other.chars().next() {
                    Key::Unicode(ch)
                } else {
                    return Err(anyhow!("Unsupported key: {key_name}"));
                }
            }
        };

        self.enigo
            .key(key, Direction::Click)
            .map_err(|e| anyhow!("Key press failed: {e}"))?;
        Ok(())
    }

    pub fn scroll(&mut self, delta_y: i32) -> Result<()> {
        self.enigo
            .scroll(delta_y, Axis::Vertical)
            .map_err(|e| anyhow!("Scroll failed: {e}"))?;
        Ok(())
    }
}
