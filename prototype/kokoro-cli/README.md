# kokoro-cli (prototype)

Temporary prototype. **Will be deleted** when its lessons are absorbed into the main codebase. See `AudiobookGenerator-Wiki/wiki/concepts/prototype-workspace.md`.

## What it validates

End-to-end TTS pipeline with **no Python, no Tauri, no llama-server**:

```
EPUB -> rbook (parse) -> sentencex (segment) -> regex (chunk)
     -> ort + Kokoro ONNX (synthesize) -> hound (write WAV)
     -> ffmpeg shell-out (merge WAV per chapter into MP3)
     -> serde (write failed_chunks.json on error)
```

If this works, every Rust crate the main Tauri core will need has been exercised against a real model on a real book.

## How to run

```bash
# First time: downloads the Kokoro-82M ONNX model to the OS cache
# (typically ~/.cache/huggingface/ on Linux, %LOCALAPPDATA% on Windows)
cargo run --release -- \
    --epub path/to/book.epub \
    --voice af_heart \
    --output ./out/

# Or for a quick test with sample text
cargo run --release -- --demo
```

The CLI will:

1. Print a list of chapters detected in the EPUB
2. Ask for confirmation (or `--yes` to skip)
3. Process each chapter into a WAV per chunk
4. Concatenate the WAVs into one MP3 per chapter
5. Write `failed_chunks.json` in the output directory if anything failed

## Done criteria

This prototype is "done" when:

- [ ] An EPUB of at least one chapter processes end-to-end
- [ ] The output MP3 plays back with intelligible speech
- [ ] `failed_chunks.json` is written when chunks fail (e.g. kill the process mid-run and re-run)
- [ ] Re-running after a failure resumes from the failed chunks only

When all boxes are checked, the lessons are absorbed into the main code and this directory is removed.

## Layout

```
prototype/kokoro-cli/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs      # CLI entry, orchestrates the pipeline
    ├── epub.rs      # rbook-based EPUB parser
    ├── chunker.rs   # sentencex + regex chunking
    ├── kokoro.rs    # ort-based Kokoro ONNX inference
    ├── merger.rs    # ffmpeg shell-out for WAV->MP3
    └── recovery.rs  # serde-based failed_chunks.json
```
