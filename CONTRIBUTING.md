# Contributing to Audiobook Generator

Thank you for your interest in contributing! 🎧

## How to Contribute

### Reporting Bugs

When reporting bugs, please include:
- **Operating System** (Windows, macOS, Linux)
- **Python version** (`python --version`)
- **GPU** (if applicable, and the driver version)
- **Steps to reproduce** the issue
- **Error messages** (full tracebacks if possible)

### Suggesting Features

Open an issue with the label `enhancement`. Describe:
- The problem you're trying to solve
- How you envision the solution
- Any relevant examples or references

### Pull Requests

1. **TTS Models:** Do NOT commit model files, weights, or anything in `tts_models/`. Only code changes belong in PRs.
2. **Testing:** Test your changes with the Gradio interface before submitting.
3. **New TTS Engines:** If adding a new model, include:
   - Clear setup instructions in the PR description
   - The model's license (with a link to the original license)
   - A note that users must accept the model's license separately

### Code Style

- Use meaningful variable and function names
- Comment complex logic
- Keep functions focused and small

### License

By contributing, you agree that your contributions will be licensed under the MIT License of this project.
