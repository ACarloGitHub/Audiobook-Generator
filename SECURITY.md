# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.x     | ✅ Currently supported |

## Security Model

Audiobook Generator processes everything **locally on your machine**. No data is sent to external servers during audiobook generation. Your EPUB files and generated audiobooks never leave your device.

## Important Notes

### Third-Party Models

This project integrates several TTS models from different publishers. Each model has its own security posture, license terms, and privacy policy. **You are responsible for reviewing and understanding the security implications of each model you download and use.** Please consult each model's official documentation:

- **XTTSv2 (Coqui)** — [Coqui Terms of Use](https://coqui.ai/terms)
- **Kokoro (Hexgrad)** — [Kokoro License](https://github.com/hexgrad/kokoro)
- **VibeVoice (Microsoft)** — [Microsoft Open Source Code of Conduct](https://github.com/microsoft/VibeVoice)
- **Qwen 3 TTS (Alibaba Cloud)** — [Qwen License](https://github.com/QwenLM/Qwen3-TTS)

### Reporting Vulnerabilities

If you discover a security vulnerability in **this project's code** (not in the TTS models themselves), please report it responsibly:

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
