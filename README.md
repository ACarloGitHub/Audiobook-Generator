<h1 align="center">
  <img src="assets/icon.jpg" alt="" width="80" style="vertical-align: middle; border-radius: 16px;">
  &nbsp;Audiobook Generator
</h1>

<p align="center">
  <strong>Your books, narrated by AI — locally, privately, beautifully</strong>
</p>

<p align="center">
  <strong>Status: rewriting the stack</strong> — the project is migrating from Python + Gradio to Tauri + llama.cpp. See <a href="AudiobookGenerator-Wiki/todo.md">todo.md</a> and the <a href="AudiobookGenerator-Wiki/wiki/concepts/migration-roadmap.md">migration roadmap</a> for details. New releases will ship as native desktop installers. Until then, the legacy Python implementation still works but is no longer the recommended path.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-yellow?style=for-the-badge" alt="License"></a>
  <a href="https://www.patreon.com/c/PatataLab"><img src="https://img.shields.io/badge/Patreon-Support-FF424D?style=for-the-badge&logo=patreon&logoColor=white" alt="Patreon"></a>
  <a href="https://buymeacoffee.com/patatalab"><img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-%23FFDD00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black" alt="Buy Me A Coffee"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey" alt="Platform">
  <img src="https://img.shields.io/badge/Hardware-GPU%20%7C%20CPU-green?logo=nvidia&logoColor=green" alt="GPU/CPU">
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

All engines run locally through `llama-server` (a Tauri sidecar binary) or, in the case of Kokoro, through ONNX Runtime loaded in-process. There is no Python and no per-engine virtual environment to manage.

| Engine | Format | Quality | Speed | Voice Cloning | Hardware |
|--------|--------|---------|-------|---------------|----------|
| **Qwen3-TTS** (Alibaba) | GGUF | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ✅ (3 s ref) | CPU / GPU / Vulkan |
| **OuteTTS 1.0** (OuteAI) | GGUF | ⭐⭐⭐⭐ | ⭐⭐⭐ | ✅ (10 s ref) | CPU / GPU |
| **NeuTTS Air** (Neuphonic) | GGUF | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ✅ (3 s ref) | CPU |
| **Kokoro** (Hexgrad) | ONNX | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ❌ (preset voices) | CPU |

Each model has its own license. You are responsible for reviewing and accepting the license of any model you download. See [SECURITY.md](SECURITY.md) for the per-model links.

---

## Features

- **EPUB Processing** — Reads and parses EPUB files automatically
- **Multiple TTS Engines** — Choose the best model for your needs, switch any time
- **Voice Cloning** — Clone your own voice for a personal narrated audiobook (3-10 seconds of reference audio, depending on the engine)
- **Multilingual** — The TTS models auto-detect the language of the input text. No language picker in the UI
- **Recovery Mode** — Resume interrupted generations from where they left off
- **GPU Acceleration** — CUDA, Vulkan, Metal, DirectML supported through llama-server
- **One installer** — No Python, no virtual environment, no `pip install`. The installer is self-contained

---

## Quick Start

The first public release will ship as a native installer for Windows, macOS, and Linux. Until then, the project is in active development — see the [migration roadmap](AudiobookGenerator-Wiki/wiki/concepts/migration-roadmap.md).

When the first installer is available:

```bash
# 1. Download the installer for your platform
#    Windows: AudiobookGenerator-x.y.z.msi
#    macOS:   AudiobookGenerator-x.y.z.dmg
#    Linux:   AudiobookGenerator-x.y.z.AppImage

# 2. Install and launch

# 3. Pick a TTS engine, download a model, drop in an EPUB, click Generate
```

The installer bundles `llama-server`, `ffmpeg`, and the Tauri shell. Models are downloaded on demand from inside the app. There is no Python, no `pip install`, no virtual environment to manage.

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

A special thank you goes to the open-source TTS community — Alibaba (Qwen), OuteAI, Neuphonic, and Hexgrad — for making powerful voice synthesis accessible to everyone.

And a very special thank you to **Carlo**, who believed this was worth building.

---

## License

Copyright © 2026 Audiobook Generator — **MIT License**

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.

**Important:** the TTS models integrated into this project are subject to their own licenses, independent of the MIT License. This project is not affiliated with or endorsed by any model publisher.

---

*Audiobook Generator — Your books, narrated by AI.*
