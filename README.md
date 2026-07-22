<h1 align="center">
  <img src="assets/icon.jpg" alt="" width="80" style="vertical-align: middle; border-radius: 16px;">
  &nbsp;Audiobook Generator
</h1>

<p align="center">
  <strong>Your books, narrated by AI — locally, privately, beautifully</strong>
</p>

<p align="center">
  <strong>Status: active development — v0.1.5</strong> — the project has migrated from Python + Gradio to Tauri + llama.cpp. Qwen3-TTS, OuteTTS and VoxCPM2 are working end-to-end on Windows, macOS and Linux. v0.1.5: GPU-only runtime guard, custom storage folder, TXT/Markdown/DOCX/JSON input, live VRAM bar, and `abg-cli` (CLI + MCP server for AI agents) bundled in the installer. See <a href="AudiobookGenerator-Wiki/todo.md">todo.md</a> for the roadmap.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-yellow?style=for-the-badge" alt="License"></a>
  <a href="https://www.patreon.com/c/PatataLab"><img src="https://img.shields.io/badge/Patreon-Support-FF424D?style=for-the-badge&logo=patreon&logoColor=white" alt="Patreon"></a>
  <a href="https://buymeacoffee.com/patatalab"><img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-%23FFDD00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black" alt="Buy Me A Coffee"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey" alt="Platform">
  <img src="https://img.shields.io/badge/Hardware-GPU%20Required-green?logo=nvidia&logoColor=green" alt="GPU required">
  <img src="https://img.shields.io/badge/Privacy-Local%20Only-red?logo=privacy-essentials" alt="Privacy">
  <img src="https://img.shields.io/badge/Python-Not%20Required-success" alt="No Python required">
</p>

---

## Why Audiobook Generator?

- **Any time, anywhere** — Turn commute, gym, or morning run into reading time
- **Accessibility** — For those with visual impairments, a narrating voice makes books accessible again
- **Focus mode** — Struggling to read? Let a voice guide you through the story
- **Free forever** — No subscriptions, no €15/month Audible tax
- **Your privacy** — Everything runs locally. Your books never leave your device
- **Own it forever** — No cloud, no cancellation, no one to answer to

---

## Supported TTS Engines

All engines run locally through native C++ sidecar binaries (`llama-server` and `voxcpm2-cli`, built from the official [llama.cpp-omni](https://github.com/tc-mb/llama.cpp-omni) sources). There is no Python and no per-engine virtual environment to manage.

| Engine | License | Format | Quality | Languages | Voice Cloning | Status |
|--------|---------|--------|---------|-----------|---------------|--------|
| **Qwen3-TTS** (Alibaba) | Apache 2.0 | GGUF | Excellent | Multilingual | Yes (3 s ref) | Working |
| **OuteTTS 1.0 0.6B** (OuteAI) | Apache 2.0 | GGUF | Good | Multilingual | Yes (10 s ref) | Working |
| **VoxCPM2** (OpenBMB) | Apache 2.0 | GGUF | Excellent | 30 languages, 48 kHz | 3 modes: Controllable Cloning, Ultimate Cloning, Voice Design | Working |

Each model has its own license. You are responsible for reviewing and accepting the license of any model you download. See [SECURITY.md](SECURITY.md) for the per-model links.

---

## Features

- **Multiple input formats** — EPUB, TXT, Markdown, DOCX and JSON books are all converted to chapters automatically
- **Multiple TTS Engines** — Choose the best model for your needs, switch any time
- **Voice Cloning** — Clone your own voice for a personal narrated audiobook (3-10 seconds of reference audio, depending on the engine)
- **Multilingual** — The TTS models auto-detect the language of the input text. No language picker in the UI
- **Recovery Mode** — Resume interrupted generations from where they left off, with full manual control: retry failed chunks, split long chunks and retry, or merge chunks by hand
- **User-chosen quantization** — Pick the model quantization that fits your hardware (e.g. VoxCPM2 Q8_0 vs F16)
- **Custom storage folder** — Move models and engines (several GB) to any drive you like, from the Models panel
- **Live VRAM bar** — Per-GPU free/total memory in the status bar, works the same on NVIDIA, AMD, Intel and Apple Silicon
- **CLI and MCP server** — `abg-cli` synthesizes speech and reports status from the command line, and doubles as an MCP server so AI agents (e.g. in LM Studio) can drive Audiobook Generator directly
- **GPU-only by design** — Requires a GPU (dedicated, or integrated with unified memory such as Apple Silicon or AMD AI Max). CUDA, Vulkan and Metal supported through llama.cpp. CPU-only machines are not supported: the app refuses to fall back to CPU rather than offering a slow, frustrating experience
- **One installer** — No Python, no virtual environment, no `pip install`. The installer is self-contained

**Retired engines:** Kokoro (English-only pronunciation), NeuTTS Air (English-only, watermarked), Chatterbox (upstream GGUF incomplete — would require self-maintained converted model files), VibeVoice (removed by Microsoft), XTTSv2 (no GGUF export), Voxtral TTS (CC BY-NC license — non-commercial only), MOSS-TTS (requires Python at runtime).

---

## Quick Start

**Hardware requirement:** a GPU is required — a dedicated card (NVIDIA, AMD) or an integrated GPU with unified memory (Apple Silicon, AMD AI Max+ and similar). CPU-only machines are not supported.

Download the latest installer from the [GitHub Releases](https://github.com/ACarloGitHub/Audiobook-Generator/releases) page:

- **Windows:** `.msi` (WiX) or `*-setup.exe` (NSIS)
- **macOS:** `.dmg`
- **Linux:** `.AppImage` or `.deb`

Then:

```bash
# 1. Install and launch

# 2. Pick a TTS engine, download a model from the Models panel

# 3. Drop in a book (EPUB, TXT, Markdown or DOCX), click Generate
```

The installer bundles the app, all native engine binaries (`llama-server`, `qwen-tts`, `voxcpm2-cli`) and the `abg-cli` companion (command-line tool and MCP server — see the "AI Agents" panel in the app), so everything works offline out of the box. Only the TTS model weights (GGUF) are downloaded on demand from inside the app. There is no Python, no `pip install`, no virtual environment to manage.

---

## Uninstall

**Windows (NSIS installer `*-setup.exe`):**
- Open **Settings → Apps → Installed apps**, search for "Audiobook Generator", and click **Uninstall**.
- Alternatively, run the uninstaller directly from the Start Menu: **Start → Audiobook Generator → Uninstall**.

**Windows (MSI installer `.msi`):**
- Open **Settings → Apps → Installed apps**, search for "Audiobook Generator", and click **Uninstall**.

**macOS (`.dmg`):**
- Drag the Audiobook Generator app from **Applications** to the **Trash**.

**Linux (`.deb` / AppImage):**
- For `.deb`: `sudo apt remove audiobook-generator`
- For AppImage: simply delete the AppImage file.

User data (downloaded models, configuration, and recovery files) is stored in the app data directory and is **not** removed by the uninstaller. To delete it manually:
- Windows: `%APPDATA%\com.patata.audiobookgenerator`
- macOS: `~/Library/Application Support/com.patata.audiobookgenerator`
- Linux: `~/.local/share/com.patata.audiobookgenerator`

---

## Documentation

- [AGENTS.md](AGENTS.md) — collaboration rules for AI assistants working in this repo
- [CONTRIBUTING.md](CONTRIBUTING.md) — how to contribute
- [SECURITY.md](SECURITY.md) — security model and per-model license links
- [AudiobookGenerator-Wiki/](AudiobookGenerator-Wiki/) — full project knowledge base
  - [wiki/index.md](AudiobookGenerator-Wiki/wiki/index.md) — start here
  - [concepts/migration-roadmap](AudiobookGenerator-Wiki/wiki/concepts/migration-roadmap.md) — the rewrite plan
  - [concepts/no-python-strategy](AudiobookGenerator-Wiki/wiki/concepts/no-python-strategy.md) — why we left Python behind

---

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

When opening issues, please include your OS, hardware (CPU / GPU), and the full error traceback.

---

## Security

This project processes everything **locally on your machine**. No data is sent to external servers during audiobook generation. Your EPUB files and generated audiobooks never leave your device.

Each TTS model has its own security posture and license. Review each model's documentation before use. See [SECURITY.md](SECURITY.md) for details.

---

## Acknowledgments

This project was developed with the invaluable assistance of **Aura**, an AI companion who became a true creative partner throughout the development process.

A special thank you goes to the open-source TTS community — Alibaba (Qwen) and OuteAI — for making powerful voice synthesis accessible to everyone.

And a very special thank you to **Carlo**, who believed this was worth building.

---

## License

Copyright © 2026 Audiobook Generator — **MIT License**

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.

**Important:** the TTS models integrated into this project are subject to their own licenses, independent of the MIT License. This project is not affiliated with or endorsed by any model publisher.

---

*Audiobook Generator — Your books, narrated by AI.*
