# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| Tauri-based releases (0.1 and later, current line 0.5.x) | Yes |
| Legacy Python + Gradio (pre-0.1) | Best effort, no new fixes |

The project was rewritten from Python + Gradio to Tauri + Rust. Releases from 0.1 on are Tauri-based; the legacy Python releases are still functional but no new security fixes are backported to them.

## Security Model

Audiobook Generator processes everything **locally on your machine**. No data is sent to external servers during audiobook generation. Your EPUB files and generated audiobooks never leave your device.

The application is composed of:

- A Tauri shell (TypeScript frontend + Rust native core)
- Native sidecar binaries shipped inside the installer: `llama-server`, `qwen-tts`, `voxcpm2-cli` and `audiocpp_cli` (load GGUF models). `ffmpeg` is optional: the app uses the system one when present, otherwise chapters are merged into WAV by the in-app fallback merger
- An `ort`-backed ONNX Runtime for audio codec decoding (DAC speech codec for OuteTTS)
- User-downloaded GGUF / ONNX model files in the per-user data directory

None of these components open outbound network connections except for:

- The model downloader, which talks to `huggingface.co` when the user explicitly clicks "Download model"
- The auto-update channel (if enabled in the build), which talks to the project's GitHub releases

The first-run wizard downloads nothing: the engine binaries ship inside the installer. The AuraWrite integration does not use the network either — it only reads files that AuraWrite writes on the same machine. All network access is opt-in, and the Tauri shell can be built with the auto-update channel disabled.

## Important Notes

### Third-Party Models

This project integrates several TTS models from different publishers. Each model has its own security posture, license terms, and privacy policy. **You are responsible for reviewing and understanding the security implications of each model you download and use.** Please consult each model's official documentation:

- **Qwen3-TTS (Alibaba Cloud)** — [Apache 2.0](https://huggingface.co/Qwen/Qwen3-TTS-12Hz-1.7B-Base)
- **OuteTTS 1.0 0.6B (OuteAI)** — [Apache 2.0](https://huggingface.co/OuteAI/OuteTTS-1.0-0.6B) (Qwen3 0.6B backbone + DAC speech codec)
- **VoxCPM2 (OpenBMB)** — [Apache 2.0](https://huggingface.co/openbmb/VoxCPM2) (2B tokenizer-free diffusion-AR TTS, 30 languages, 48 kHz; GGUF build from the official [llama.cpp-omni](https://github.com/tc-mb/llama.cpp-omni) fork, compiled unmodified)
- **Audio8-TTS-Realistic 0.6B (Edge0-AI)** — [Apache 2.0](https://huggingface.co/Edge0/Audio8-TTS-Realistic-0.6b) (0.6B DualAR TTS with a 44.1 kHz neural codec, 23 languages; GGUF published by the [audio.cpp](https://github.com/0xShug0/audio.cpp) runtime project, loaded by `audiocpp_cli` compiled unmodified)

The retired engines are no longer shipped or supported:

- ~~Kokoro (Hexgrad)~~ — retired; English-only pronunciation
- ~~NeuTTS Air (Neuphonic)~~ — excluded; English-only with audio watermark
- ~~Chatterbox (Resemble AI)~~ — retired 2026-07-18; upstream GGUF incomplete, would require self-maintained converted model files
- ~~XTTSv2 (Coqui)~~ — retired; no GGUF export exists
- ~~VibeVoice (Microsoft)~~ — retired; code removed by publisher
- ~~Voxtral TTS (Mistral AI)~~ — excluded; CC BY-NC license (non-commercial only)
- ~~MOSS-TTS~~ — excluded; requires Python at runtime
- ~~Fish Audio S2 Pro~~ — excluded; no C++/GGUF runtime (Python only) and a research-only licence

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
