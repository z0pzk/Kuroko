mod agent;
mod config;
mod input;
mod llm;
mod ui;
mod vision;

use eframe::egui;
use ui::KurokoApp;

fn main() -> eframe::Result<()> {
    let cfg = config::AppConfig::load();
    let window_level = if cfg.always_on_top {
        egui::WindowLevel::AlwaysOnTop
    } else {
        egui::WindowLevel::Normal
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([360.0, 540.0])
            .with_min_inner_size([320.0, 440.0])
            .with_window_level(window_level),
        ..Default::default()
    };

    eframe::run_native(
        "KUROKO // 01",
        options,
        Box::new(|cc| Ok(Box::new(KurokoApp::new(cc)))),
    )
}
