# Contributing to Audiobook Generator

Thank you for your interest in contributing!

## Project Status

Audiobook Generator is being rewritten from Python + Gradio to Tauri + llama.cpp + native Rust. If you are about to send a Pull Request, please read:

- [AudiobookGenerator-Wiki/todo.md](AudiobookGenerator-Wiki/todo.md) — the open work
- [AudiobookGenerator-Wiki/wiki/concepts/migration-roadmap.md](AudiobookGenerator-Wiki/wiki/concepts/migration-roadmap.md) — the migration plan
- [AudiobookGenerator-Wiki/wiki/concepts/no-python-strategy.md](AudiobookGenerator-Wiki/wiki/concepts/no-python-strategy.md) — the direction of the rewrite

PRs against the legacy Python stack will be reviewed on a case-by-case basis. Most of the active work is on the Tauri shell and the Rust engine plugins.

## How to Contribute

### Reporting Bugs

When reporting bugs, please include:

- Operating System (Windows, macOS, Linux) and version
- Hardware (CPU / GPU; for NVIDIA, the driver version)
- The exact TTS engine and model you were using
- Steps to reproduce the issue
- Error messages (full traceback or the contents of the log file)

### Suggesting Features

Open an issue with the label `enhancement`. Describe:

- The problem you are trying to solve
- How you envision the solution
- Any relevant examples or references

### Pull Requests

1. **TTS Models.** Do **NOT** commit model files, weights, or anything under `models/`, `~/.cache/`, or any path that contains a `.gguf` or `.onnx` file. The `gitignore` already excludes these, but please double-check before pushing.
2. **Native Dependencies.** Do not introduce a new runtime dependency unless it can be shipped as a Tauri sidecar or vendored into the binary. PRs that add a `pip install` or a system-level package requirement will be rejected.
3. **Frontend.** Use the existing Tauri + vanilla TypeScript + Vite setup. Do not introduce React, Svelte, or Vue — the project mirrors the AuraWrite conventions and we want to keep the toolchain small.
4. **Engine Plugins.** New TTS engines must:
   - Be a Rust struct that implements the `TtsEngine` trait (see [wiki/concepts/plugin-architecture.md](AudiobookGenerator-Wiki/wiki/concepts/plugin-architecture.md))
   - Ship as a GGUF or ONNX file (or a small set of them). No PyTorch checkpoints, no Python wrappers.
   - Surface the model's license in the UI and require user acceptance before the first generation
5. **Testing.** Test your changes with the Tauri development shell (`npm run tauri:dev`) before submitting.
6. **Wiki.** When you add a new concept, engine, or pattern, create or update the corresponding wiki page. See [AudiobookGenerator-Wiki/agents.md](AudiobookGenerator-Wiki/agents.md) for the schema.

### Code Style

- TypeScript: ESLint + Prettier, defaults in the repo
- Rust: `cargo fmt` and `cargo clippy` with the default lints
- Meaningful variable and function names
- Comment complex logic
- Keep functions focused and small
- Commit messages in English, present tense ("Add OuteTTS plugin", not "Added")

### License

By contributing, you agree that your contributions will be licensed under the MIT License of this project.
