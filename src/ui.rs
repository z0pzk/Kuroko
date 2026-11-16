use crate::agent::{run_agent_loop, AgentEvent};
use crate::config::{AppConfig, ProviderKind};
use crate::vision::{find_target_window, WindowBounds};
use eframe::egui::{
    self, Color32, CornerRadius, FontId, RichText, Stroke, Vec2, ViewportCommand,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

// Industrial Swiss Telemetry Palette (Zero AI-slop, zero gradients)
const BG_CARBON: Color32 = Color32::from_rgb(15, 16, 18);
const SURFACE_SLATE: Color32 = Color32::from_rgb(22, 24, 28);
const BORDER_HAIRLINE: Color32 = Color32::from_rgb(42, 45, 52);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(236, 238, 242);
const TEXT_MUTED: Color32 = Color32::from_rgb(124, 130, 141);
const ACCENT_ORANGE: Color32 = Color32::from_rgb(255, 77, 0);
const LED_GREEN: Color32 = Color32::from_rgb(34, 197, 94);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Run,
    Engine,
    Log,
}

pub struct KurokoApp {
    rt: Runtime,
    config: AppConfig,
    tab: Tab,
    goal_input: String,
    running: bool,
    abort_flag: Arc<AtomicBool>,
    rx: Option<UnboundedReceiver<AgentEvent>>,
    current_step: usize,
    feed: Vec<String>,
    raw_logs: Vec<String>,
    detected_window: Option<WindowBounds>,
    last_probe_time: f64,
}

impl KurokoApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_industrial_theme(&cc.egui_ctx);
        let rt = Runtime::new().expect("tokio runtime");
        let config = AppConfig::load();
        let detected_window = find_target_window(&config.target_window_query).ok();

        Self {
            rt,
            config,
            tab: Tab::Run,
            goal_input: String::new(),
            running: false,
            abort_flag: Arc::new(AtomicBool::new(false)),
            rx: None,
            current_step: 0,
            feed: vec!["READY // ZERO-DEVTOOLS MODE".to_string()],
            raw_logs: Vec::new(),
            detected_window,
            last_probe_time: 0.0,
        }
    }

    fn start_task(&mut self) {
        if self.running || self.goal_input.trim().is_empty() {
            return;
        }
        self.config.save();
        self.running = true;
        self.current_step = 0;
        self.feed.clear();
        self.abort_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = unbounded_channel();
        self.rx = Some(rx);

        let cfg = self.config.clone();
        let goal = self.goal_input.clone();
        let flag = self.abort_flag.clone();

        self.rt.spawn(async move {
            run_agent_loop(cfg, goal, flag, tx).await;
        });
    }

    fn stop_task(&mut self) {
        self.abort_flag.store(true, Ordering::Relaxed);
    }

    fn poll_events(&mut self) {
        let Some(rx) = &mut self.rx else { return };
        while let Ok(ev) = rx.try_recv() {
            match ev {
                AgentEvent::StepStarted { step, .. } => {
                    self.current_step = step;
                }
                AgentEvent::ActionExecuted { step, summary } => {
                    self.feed.push(format!("[{:02}] {}", step, summary));
                }
                AgentEvent::Log(line) => {
                    self.raw_logs.push(line);
                }
                AgentEvent::Finished(msg) => {
                    self.running = false;
                    self.feed.push(format!("END // {msg}"));
                }
                AgentEvent::Error(err) => {
                    self.running = false;
                    self.feed.push(format!("ERR // {err}"));
                    self.raw_logs.push(format!("ERR // {err}"));
                }
            }
        }
    }
}

impl eframe::App for KurokoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();

        // Probe Brave window every 2 seconds when idle
        let now = ctx.input(|i| i.time);
        if !self.running && now - self.last_probe_time > 2.0 {
            self.last_probe_time = now;
            self.detected_window = find_target_window(&self.config.target_window_query).ok();
        }

        // Global ESC kill-switch while running
        if self.running && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.stop_task();
        }

        if self.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG_CARBON).inner_margin(12.0))
            .show(ctx, |ui| {
                // --- TOP TELEMETRY STRIP ---
                ui.horizontal(|ui| {
                    let led_color = if self.running {
                        ACCENT_ORANGE
                    } else if self.detected_window.is_some() {
                        LED_GREEN
                    } else {
                        TEXT_MUTED
                    };
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, CornerRadius::same(1), led_color);

                    ui.label(
                        RichText::new("KUROKO // 01")
                            .font(FontId::monospace(13.0))
                            .strong()
                            .color(TEXT_PRIMARY),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(self.config.provider.label())
                                .font(FontId::monospace(11.0))
                                .color(TEXT_MUTED),
                        );
                    });
                });

                ui.add_space(4.0);
                let target_status = match &self.detected_window {
                    Some(w) => format!("TARGET: {} ({}x{})", w.app_name.to_uppercase(), w.width, w.height),
                    None => format!("TARGET: '{}' NOT FOUND", self.config.target_window_query),
                };
                ui.label(
                    RichText::new(target_status)
                        .font(FontId::monospace(10.5))
                        .color(if self.detected_window.is_some() { TEXT_MUTED } else { ACCENT_ORANGE }),
                );

                ui.add_space(8.0);
                ui.separator();

                // --- TAB STRIP ---
                ui.horizontal(|ui| {
                    for (tab_val, label) in [(Tab::Run, "RUN"), (Tab::Engine, "ENGINE"), (Tab::Log, "LOG")] {
                        let active = self.tab == tab_val;
                        let resp = ui.selectable_label(
                            active,
                            RichText::new(label)
                                .font(FontId::monospace(11.5))
                                .color(if active { ACCENT_ORANGE } else { TEXT_MUTED }),
                        );
                        if resp.clicked() {
                            self.tab = tab_val;
                        }
                    }
                });
                ui.separator();
                ui.add_space(8.0);

                match self.tab {
                    Tab::Run => self.render_run_tab(ui),
                    Tab::Engine => self.render_engine_tab(ui, ctx),
                    Tab::Log => self.render_log_tab(ui),
                }
            });
    }
}

impl KurokoApp {
    fn render_run_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("COMMAND")
                .font(FontId::monospace(10.5))
                .color(TEXT_MUTED),
        );
        ui.add_space(4.0);

        ui.add_enabled(
            !self.running,
            egui::TextEdit::multiline(&mut self.goal_input)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .font(FontId::proportional(13.0))
                .hint_text("e.g. Open YouTube, search for ambient music, and click the first video"),
        );

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if !self.running {
                let btn = egui::Button::new(
                    RichText::new("EXECUTE")
                        .font(FontId::monospace(12.0))
                        .color(Color32::BLACK)
                        .strong(),
                )
                .fill(ACCENT_ORANGE)
                .corner_radius(CornerRadius::same(2))
                .min_size(Vec2::new(110.0, 30.0));

                if ui.add(btn).clicked() {
                    self.start_task();
                }
            } else {
                let btn = egui::Button::new(
                    RichText::new("ABORT [ESC]")
                        .font(FontId::monospace(12.0))
                        .color(TEXT_PRIMARY)
                        .strong(),
                )
                .fill(Color32::from_rgb(185, 28, 28))
                .corner_radius(CornerRadius::same(2))
                .min_size(Vec2::new(120.0, 30.0));

                if ui.add(btn).clicked() {
                    self.stop_task();
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("STEP {:02}/{:02}", self.current_step, self.config.max_steps))
                        .font(FontId::monospace(11.5))
                        .color(TEXT_MUTED),
                );
            });
        });

        ui.add_space(10.0);
        ui.label(
            RichText::new("ACTION FEED")
                .font(FontId::monospace(10.5))
                .color(TEXT_MUTED),
        );
        ui.add_space(4.0);

        egui::Frame::new()
            .fill(SURFACE_SLATE)
            .stroke(Stroke::new(1.0, BORDER_HAIRLINE))
            .corner_radius(CornerRadius::same(2))
            .inner_margin(8.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for line in &self.feed {
                            ui.label(
                                RichText::new(line)
                                    .font(FontId::monospace(11.0))
                                    .color(TEXT_PRIMARY),
                            );
                        }
                    });
            });
    }

    fn render_engine_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(RichText::new("AI PROVIDER").font(FontId::monospace(10.5)).color(TEXT_MUTED));
            ui.horizontal_wrapped(|ui| {
                for kind in [
                    ProviderKind::Gemini,
                    ProviderKind::OpenAiCompatible,
                    ProviderKind::Ollama,
                    ProviderKind::Anthropic,
                ] {
                    if ui
                        .selectable_label(self.config.provider == kind, kind.label())
                        .clicked()
                    {
                        self.config.provider = kind;
                        self.config.model = kind.default_model().to_string();
                        self.config.base_url = kind.default_base_url().to_string();
                    }
                }
            });

            ui.add_space(8.0);
            ui.label(RichText::new("MODEL").font(FontId::monospace(10.5)).color(TEXT_MUTED));
            ui.add(egui::TextEdit::singleline(&mut self.config.model).desired_width(f32::INFINITY));

            ui.add_space(8.0);
            ui.label(RichText::new("BASE URL").font(FontId::monospace(10.5)).color(TEXT_MUTED));
            ui.add(egui::TextEdit::singleline(&mut self.config.base_url).desired_width(f32::INFINITY));

            if self.config.provider != ProviderKind::Ollama {
                ui.add_space(8.0);
                ui.label(RichText::new("API KEY").font(FontId::monospace(10.5)).color(TEXT_MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut self.config.api_key)
                        .password(true)
                        .desired_width(f32::INFINITY),
                );
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            ui.label(RichText::new("TARGET WINDOW MATCH").font(FontId::monospace(10.5)).color(TEXT_MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.config.target_window_query)
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(8.0);
            ui.add(egui::Slider::new(&mut self.config.max_steps, 1..=40).text("MAX STEPS"));
            ui.add(egui::Slider::new(&mut self.config.step_delay_ms, 200..=3000).text("DELAY (MS)"));
            ui.checkbox(&mut self.config.humanize_mouse, "BEZIER HUMANIZED MOUSE");

            if ui.checkbox(&mut self.config.always_on_top, "ALWAYS ON TOP").changed() {
                let level = if self.config.always_on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                };
                ctx.send_viewport_cmd(ViewportCommand::WindowLevel(level));
            }

            ui.add_space(10.0);
            if ui
                .button(RichText::new("SAVE CONFIG").font(FontId::monospace(11.5)).color(ACCENT_ORANGE))
                .clicked()
            {
                self.config.save();
            }
        });
    }

    fn render_log_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("TELEMETRY LOG").font(FontId::monospace(10.5)).color(TEXT_MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("CLEAR").clicked() {
                    self.raw_logs.clear();
                }
            });
        });
        ui.add_space(4.0);
        egui::Frame::new()
            .fill(SURFACE_SLATE)
            .stroke(Stroke::new(1.0, BORDER_HAIRLINE))
            .corner_radius(CornerRadius::same(2))
            .inner_margin(8.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for line in &self.raw_logs {
                            ui.label(
                                RichText::new(line)
                                    .font(FontId::monospace(10.5))
                                    .color(TEXT_MUTED),
                            );
                        }
                    });
            });
    }
}

fn apply_industrial_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_CARBON;
    visuals.window_fill = SURFACE_SLATE;
    visuals.extreme_bg_color = SURFACE_SLATE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_HAIRLINE);
    visuals.widgets.inactive.bg_fill = SURFACE_SLATE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_HAIRLINE);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(2);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(2);
    visuals.widgets.active.corner_radius = CornerRadius::same(2);
    visuals.selection.bg_fill = ACCENT_ORANGE;
    visuals.selection.stroke = Stroke::new(1.0, Color32::BLACK);
    ctx.set_visuals(visuals);
}
