# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| New Tauri-based releases (1.x and later) | Yes |
| Legacy Python + Gradio (0.x) | Best effort, no new fixes |

The project is being rewritten. New releases are Tauri-based; the legacy Python releases are still functional but no new security fixes will be backported to them.

## Security Model

Audiobook Generator processes everything **locally on your machine**. No data is sent to external servers during audiobook generation. Your EPUB files and generated audiobooks never leave your device.

The application is composed of:

- A Tauri shell (TypeScript frontend + Rust native core)
- A bundled `llama-server` sidecar that loads GGUF models
- A bundled `ffmpeg` binary for audio processing
- An `ort`-backed ONNX Runtime for the Kokoro engine
- User-downloaded GGUF / ONNX model files in the per-user data directory

None of these components open outbound network connections except for:

- The model downloader, which talks to `huggingface.co` when the user explicitly clicks "Download model"
- The auto-update channel (if enabled in the build), which talks to the project's GitHub releases

Both of these are opt-in. The Tauri shell can be built with the auto-update channel disabled.

## Important Notes

### Third-Party Models

This project integrates several TTS models from different publishers. Each model has its own security posture, license terms, and privacy policy. **You are responsible for reviewing and understanding the security implications of each model you download and use.** Please consult each model's official documentation:

- **Qwen3-TTS (Alibaba Cloud)** — [Qwen License](https://github.com/QwenLM/Qwen3-TTS) (Apache 2.0)
- **OuteTTS 1.0 (OuteAI)** — [Llama 3.2 community license](https://llama.meta.com/llama3_2/license) for the backbone plus CC-BY-NC-SA-4.0 for the OuteTTS additions
- **NeuTTS Air (Neuphonic)** — [Neuphonic terms](https://neuphonic.com) (Apache 2.0)
- **Kokoro (Hexgrad)** — [Kokoro License](https://github.com/hexgrad/kokoro) (Apache 2.0)

The retired engines (XTTSv2, VibeVoice) are no longer shipped or supported:

- ~~XTTSv2 (Coqui)~~ — retired; no GGUF export exists
- ~~VibeVoice (Microsoft)~~ — retired; no GGUF export exists

### Reporting Vulnerabilities

If you discover a security vulnerability in **this project's code** (not in a third-party TTS model), please report it responsibly:

1. **Do NOT** open a public GitHub issue
2. Send a private report by opening a [Security Advisory](https://github.com/ACarloGitHub/Audiobook-Generator/security/advisories/new) instead

Please include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Any suggested fixes (optional)

We aim to respond within 48 hours and will keep you updated on our progress.

## Disclaimer

This software is provided "as-is." The authors are not liable for any damage, data loss, or security issues arising from the use of this software or any third-party TTS models integrated into it. Users are responsible for the security of their own systems and for complying with all applicable model licenses.
