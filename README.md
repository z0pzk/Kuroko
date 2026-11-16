[README.md](https://github.com/user-attachments/files/34926509/README.md)
# KUROKO // 01

Zero-DevTools vision-driven browser operator for Brave / Chromium, written in Rust.

## Concept
Named after **Kuroko (黒子)** — the stagehands in traditional Japanese Kabuki theater who dress in black and move props invisibly while the actors perform.

Instead of launching Brave with `--remote-debugging-port`, injecting JavaScript, or triggering `Runtime.enable` / `navigator.webdriver = true`, Kuroko operates **purely from the outside**:
1. **Observe**: Captures only the active Brave window via OS window capture (`xcap`), downscaling to JPEG.
2. **Decide**: Sends the screenshot to any Vision-capable LLM (Gemini, OpenAI-compatible, Ollama, or Claude) and receives a single normalized `[0..1000]` JSON action.
3. **Act**: Translates normalized coordinates to screen pixels and executes humanized quadratic-Bezier mouse movement, clicks, typing, and scrolling via OS input (`enigo`).

## Supported AI Backends
- **Gemini**: `gemini-2.5-flash` / `gemini-2.5-pro` via `generativelanguage.googleapis.com`
- **OpenAI-Compatible**: OpenAI, OpenRouter, Groq, LM Studio (`http://localhost:1234/v1`), llama.cpp (`http://localhost:8080/v1`)
- **Ollama (Local)**: `qwen2.5vl:7b`, `llama3.2-vision`, `minicpm-v` via `http://localhost:11434`
- **Anthropic**: Claude vision models via `api.anthropic.com`

## Build & Run
```bash
cargo build --release
./target/release/kuroko
```

Press **ESC** at any time while running to abort immediately.
