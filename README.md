# NEXUS — Local AI Workbench

A local-first, desktop AI workbench for Windows 11. Combines a personal AI second brain, a research copilot, document intelligence, and a controlled local agent into one native-feeling application.

The application runs **entirely on the user's PC** and is configured against:

* Qwen3.5 9B Q4_K_M (general assistant) — referenced at `D:\LmStudio\lmstudio-community\Qwen3.5-9B-GGUF\Qwen3.5-9B-Q4_K_M.gguf`.
* Qwen2.5 Coder 14B Q4_K_M (coding specialist) — referenced at `D:\LmStudio\Qwen\Qwen2.5-Coder-14B-Instruct-GGUF\qwen2.5-coder-14b-instruct-q4_k_m.gguf`.
* LM Studio's OpenAI-compatible local API at `http://127.0.0.1:1234/v1` as the default backend.

It does not embed or copy any model files. It does not require an internet connection for normal use. It does not upload documents, prompts, or model outputs.

## Status

* **Phase 0** complete: environment audit, architecture (`ARCHITECTURE.md`), data model (`DATA_MODEL.md`), security model (`SECURITY.md`), ADRs, implementation plan.
* **Phase 1** complete: Tauri 2 + Rust + React/TS desktop shell, sidebar + Command Center + System Monitor + Model Manager + Settings, SQLite + migrations, dark theme, command palette. See `docs/PHASE1_SUMMARY.md`.
* **Phase 2** (in progress): real LM Studio / llama.cpp / Ollama provider abstraction, streaming chat, test prompt, model profile management, capability detection, GPU offload notes.

## Building from source

Requirements:
* Windows 11 (or 10)
* Rust toolchain via rustup with the `x86_64-pc-windows-gnu` host
* MSYS2 UCRT64 toolchain (provides `gcc.exe`)
* Node.js 20+ and npm
* Edge WebView2 runtime (preinstalled on Windows 11)

Steps:
```
# install rustup with the GNU host
rustup-init.exe -y --default-toolchain stable --default-host x86_64-pc-windows-gnu --profile minimal

# install web deps
cd app/web && npm install && cd ../..

# build the Rust binary (will also build the web bundle via tauri-build)
cd app/src-tauri && cargo build
```

The debug binary lands at `app/src-tauri/target/debug/nexus.exe`. The release binary at `app/src-tauri/target/release/nexus.exe` is produced by `cargo build --release`. The Tauri installer is produced by `cargo tauri build` (Phase 11).

## First launch

On the first launch:
1. NEXUS creates `%APPDATA%\ai.nexus.workbench\` and its subfolders.
2. SQLite migrations apply.
3. The Settings page lets the user pick the LM Studio base URL and other defaults.
4. The Model Manager page shows the live reachability of each backend.

## Layout

```
ARCHITECTURE.md          # system architecture
DATA_MODEL.md            # SQL schema (versioned migrations)
SECURITY.md              # threat model, permission levels
ADR.md                   # 10 ADRs justifying significant decisions
IMPLEMENTATION_PLAN.md   # per-phase checklist
docs/PHASE1_SUMMARY.md   # what landed in Phase 1
app/                     # source code
  src-tauri/             # Rust core + Tauri binary
  web/                   # React/TS frontend
```

## License

MIT. The bundled `qwen` GGUF model files are governed by their original Qwen licenses; NEXUS only references them by path.
