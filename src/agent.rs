use crate::config::AppConfig;
use crate::input::InputController;
use crate::llm::{decide_next_action, AgentAction};
use crate::vision::capture_target_window;
use reqwest::Client;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone)]
pub enum AgentEvent {
    StepStarted { step: usize, max_steps: usize },
    ActionExecuted { step: usize, summary: String },
    Log(String),
    Finished(String),
    Error(String),
}

pub async fn run_agent_loop(
    cfg: AppConfig,
    goal: String,
    abort_flag: Arc<AtomicBool>,
    tx: UnboundedSender<AgentEvent>,
) {
    let client = Client::new();
    let mut history: Vec<String> = Vec::new();

    for step in 1..=cfg.max_steps {
        if abort_flag.load(Ordering::Relaxed) {
            let _ = tx.send(AgentEvent::Finished("ABORTED BY USER".to_string()));
            return;
        }

        let _ = tx.send(AgentEvent::StepStarted {
            step,
            max_steps: cfg.max_steps,
        });

        let frame = match capture_target_window(&cfg.target_window_query) {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(format!("Capture error: {e}")));
                return;
            }
        };

        let _ = tx.send(AgentEvent::Log(format!(
            "STEP {:02}: captured '{}' ({}x{})",
            step, frame.bounds.title, frame.bounds.width, frame.bounds.height
        )));

        let action = match decide_next_action(
            &client,
            &cfg,
            &goal,
            &history,
            &frame.jpeg_base64,
        )
        .await
        {
            Ok(act) => act,
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(format!("LLM error: {e}")));
                return;
            }
        };

        if abort_flag.load(Ordering::Relaxed) {
            let _ = tx.send(AgentEvent::Finished("ABORTED BY USER".to_string()));
            return;
        }

        let mut input = match InputController::new(cfg.humanize_mouse) {
            Ok(ctrl) => ctrl,
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(format!("Input init error: {e}")));
                return;
            }
        };

        let step_desc = match action {
            AgentAction::Click { x, y, double, reason } => {
                match input.click_at(&frame.bounds, x, y, double) {
                    Ok((px, py)) => format!(
                        "CLICK ({:.0},{:.0} -> {}px,{}px) {}",
                        x, y, px, py, reason
                    ),
                    Err(e) => {
                        let _ = tx.send(AgentEvent::Error(format!("Click failed: {e}")));
                        return;
                    }
                }
            }
            AgentAction::Type {
                text,
                press_enter,
                reason,
            } => {
                if let Err(e) = input.type_text(&text) {
                    let _ = tx.send(AgentEvent::Error(format!("Type failed: {e}")));
                    return;
                }
                if press_enter {
                    let _ = input.press_key("enter");
                }
                format!("TYPE \"{}\" {}", text, reason)
            }
            AgentAction::Key { key, reason } => {
                if let Err(e) = input.press_key(&key) {
                    let _ = tx.send(AgentEvent::Error(format!("Key failed: {e}")));
                    return;
                }
                format!("KEY [{}] {}", key.to_uppercase(), reason)
            }
            AgentAction::Scroll { delta, reason } => {
                if let Err(e) = input.scroll(delta) {
                    let _ = tx.send(AgentEvent::Error(format!("Scroll failed: {e}")));
                    return;
                }
                format!("SCROLL ({}) {}", delta, reason)
            }
            AgentAction::Wait { ms, reason } => {
                sleep(Duration::from_millis(ms.min(5000))).await;
                format!("WAIT {}ms {}", ms, reason)
            }
            AgentAction::Done { summary } => {
                let _ = tx.send(AgentEvent::ActionExecuted {
                    step,
                    summary: format!("DONE: {summary}"),
                });
                let _ = tx.send(AgentEvent::Finished(summary));
                return;
            }
        };

        history.push(format!("Step {step}: {step_desc}"));
        let _ = tx.send(AgentEvent::ActionExecuted {
            step,
            summary: step_desc,
        });

        sleep(Duration::from_millis(cfg.step_delay_ms)).await;
    }

    let _ = tx.send(AgentEvent::Finished(format!(
        "Reached max steps ({})",
        cfg.max_steps
    )));
}
