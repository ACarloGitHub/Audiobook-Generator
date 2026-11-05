# Contributing to Audiobook Generator

Thank you for your interest in contributing!

Audiobook Generator turns books (EPUB, TXT, Markdown, DOCX, JSON) into audiobooks
using text-to-speech models that run entirely on the user's machine. Everything is
local: no cloud, no account, no telemetry, and no Python at runtime.

## Project Status

The project is **published and actively developed**. The current release is
**v0.5.0** and ships installers for Windows, macOS and Linux with four working
engines: Qwen3-TTS, OuteTTS, VoxCPM2 and Audio8-TTS (the Audio8 runtime is
Windows-only for now).

The stack is **Tauri 2 + TypeScript (vanilla + Vite) + Rust**, with each TTS
engine running as a native C++ binary compiled from unmodified upstream sources
(`qwen-tts`, `voxcpm2-cli`, `audiocpp_cli`, `llama-server`). There are no Python
virtual environments, no `pip install`, and no per-engine toolchains.

Two constraints are part of the product, not preferences:

- **A GPU is required.** There is no CPU-only build and no silent CPU fallback.
  CUDA, Vulkan and Metal are supported through llama.cpp.
- **Only unmodified upstream artefacts are used.** We do not ship re-converted or
  patched model files, and we do not maintain patched forks of third-party
  libraries. If an engine only works after we modify an upstream file, it is not
  adopted.

## Documentation

- [README.md](README.md) — what the app does, supported engines, install and
  uninstall instructions
- [SECURITY.md](SECURITY.md) — threat model and the license of every model
- [LICENSE](LICENSE) — MIT

## Reporting Bugs

Please open an issue and include:

- **Operating system** and version (Windows 10/11, macOS, Linux distribution)
- **Hardware**: CPU and GPU model; for NVIDIA, the driver version
- **Engine and model**: for example "Qwen3-TTS 1.7B Base, Q8_0" or
  "VoxCPM2 F16"; include the voice mode if it matters
- **Steps to reproduce** — the book format, the chapter, and whether it happens
  on the first chunk or later
- **The log file**, which is the single most useful piece of information:
  - Windows: `%APPDATA%\com.patata.audiobookgenerator\audiobook-generator.log`
  - macOS: `~/Library/Application Support/com.patata.audiobookgenerator/audiobook-generator.log`
  - Linux: `~/.local/share/com.patata.audiobookgenerator/audiobook-generator.log`
- **`failed_chunks.json`** if the problem involves Error Recovery: it lives in
  the output folder of the affected book and contains the exact engine error for
  every failed chunk

If the problem is a wrong or truncated voice, say so explicitly: the app has an
automatic duration check, and a chunk that fails it is moved to Error Recovery
with a reason.

## Suggesting Features

Open an issue with the `enhancement` label. Describe the problem first, then the
solution you have in mind, then any reference material (a model card, a paper, a
comparable tool). "Would be nice" is not enough: explain what it unlocks for a
user who is converting a book.

Because the engine roster is curated, proposals to add a TTS engine should
include:

- license of the weights (commercial use must be allowed)
- an inference runtime that works without Python at runtime
- a GGUF (or ONNX) build published by the upstream project or by the runtime
  author, usable as-is
- evidence that Italian works, not just that it is listed

## Pull Requests

Keep changes focused: one logical change per pull request. Open an issue first
for anything larger than a bug fix, so we can agree on the approach before you
write the code.

### Ground rules

1. **Never commit model files.** No `.gguf`, `.onnx`, `.safetensors`, no
   `models/` folder and nothing from a Hugging Face cache. Models are downloaded
   by the app at runtime; `.gitignore` already covers these paths.
2. **No new runtime that needs Python or system packages.** Anything that must be
   installed with `pip`, `apt` or a manual setup step will be rejected. Native
   dependencies belong inside the installer as Tauri sidecar binaries.
3. **No patched upstream artefacts.** Engines must work with the files and the
   binaries exactly as their authors publish them. Do not add "fixed" model
   files or carry patches on top of a third-party library.
4. **Do not add a CPU fallback.** The app refuses to synthesise without a GPU by
   design. A pull request that silently falls back to CPU goes against the
   product decision and will not be merged.
5. **Frontend: keep the toolchain small.** Tauri + vanilla TypeScript + Vite.
   Do not introduce React, Vue, Svelte or a CSS framework.
6. **Model data belongs in JSON.** Names, download URLs, licenses, languages,
   quantisations, parameters and limits live in
   `src-tauri/engine_registry.json`; the list of model ids lives in
   `src-tauri/plugins/plugin_registry.json`. Do not hardcode model names,
   limits or URLs in Rust or TypeScript.
7. **Documentation follows the change.** A user-visible change updates
   `README.md`; adding or removing an engine updates `SECURITY.md` too.

### Adding a TTS engine

1. Add the engine to `src-tauri/engine_registry.json` (license, languages,
   variants, parameters with min/max/default, chunking limits, download URLs,
   runtime and its pinned version).
2. Add one entry per model/variant to `src-tauri/plugins/plugin_registry.json`.
3. Implement the `BaseTTSPlugin` trait in `src-tauri/src/plugins/<engine>/`:
   `name`, `plugin_type`, `is_installed`, `load_model`, `synthesize`, `unload`.
   Book-level generation lives in a `synthesize_book` function in the same
   module, because chapter/segment/recovery flow is engine-specific.
4. Register it in `plugin_manager.rs` and in the generation dispatcher in
   `commands.rs`.
5. Wire downloads in `model_manager.rs` and the panels in
   `src/frontend/{configuration,models}.ts`.
6. Add the sidecar build step for every platform in
   `.github/workflows/release.yml`.
7. Cover the new parsing and defaults with tests.

Engine processes must be attached to the Windows Job Object (so they die with the
app), must go through the GPU check, and must report the last lines of the
engine's own error output rather than a generic message.

### Development setup

Requirements:

- **Node.js 20 or newer**
- **Rust 1.77 or newer** (stable)
- **Tauri 2 prerequisites**: WebView2 on Windows, Xcode Command Line Tools on
  macOS, `webkit2gtk-4.1` and `libayatana-appindicator3` on Linux
- **A GPU** with CUDA, Vulkan or Metal

Run the app:

```bash
npm install
npm run tauri:dev
```

Engine binaries are not built by `tauri dev`. For development, stage them in
`src-tauri/sidecars/bundled/<engine>/` (`qwentts`, `voxcpm2`, `audio8`,
`llama.cpp`, `cuda-shared`); model weights are downloaded from the Models panel
of the running app.

### Before submitting

All of these must pass:

```bash
npm run typecheck                  # TypeScript, no emit
npm run build                      # frontend bundle
cd src-tauri && cargo test         # Rust unit and integration tests
cd src-tauri && cargo check --bins # includes the CLI/MCP binary
```

If your change touches generation, recovery or an engine, do a real run on a
short book as well: the tests do not exercise the actual model binaries.

### Commits

- English, present tense: `Add OuteTTS retry on server restart`, not `Added ...`
- One logical change per commit; do not mix a refactor with a behaviour change
- No secrets, tokens, logs or personal paths in the diff
- If the change is visible to users, say so in the commit body and update the
  README in the same pull request

## Code Style

- **Indentation:** 4 spaces, no tabs
- **Encoding:** UTF-8
- **Rust:** follow the surrounding code; keep public functions documented when
  the behaviour is not obvious
- **TypeScript:** no `any` unless unavoidable; keep one panel per file under
  `src/frontend/`
- **Comments:** English, only where the code does not explain itself
- **Naming:** `snake_case` for variables and functions, `PascalCase` for types,
  `UPPER_SNAKE_CASE` for constants
- **Tests:** new logic ships with a test; prefer unit tests next to the code

## License

By contributing, you agree that your contributions are licensed under the MIT
License of this project.

The TTS models are **not** covered by the MIT License: each one keeps its own
terms, listed in [SECURITY.md](SECURITY.md). Do not commit model files, and do
not add a model whose license forbids commercial use.
