# Chatterbox sidecar — codec.cpp patches and bake scripts

Everything needed to rebuild the Chatterbox TTS sidecar (`tts-cli.exe` +
`ttsbackbone.dll`) and its baked-speaker GGUF from scratch, on any machine.

## Contents

- `codec.cpp-local-changes.patch` — working-tree diff of our fork of
  https://github.com/mybigday/codec.cpp, base commit
  `c5ef02b12bf4129b10aad0a463637c7372f9572f` (2026-07-17). Key changes:
  - `src/lm/speaker_chatterbox.cpp`: voice-encoder (VE) tensors optional
    (`has_ve` flag); the `_from_emb` path uses only the `lm.chatterbox.cond.*`
    tensors already present in the codec GGUF.
  - `src/lm/chatterbox_t3.cpp`: when `--ref-audio` is passed but the GGUF has
    no VE, warn and fall back to the builtin speaker conditioning instead of
    failing (`build_prompt`).
  - `src/lm/lm.cpp`, `src/lm/parallel_heads_delay.cpp`: small fixes.
  - `CMakeLists.txt`, `cmake/SetupTtsBackbone.cmake`: backbone build wiring.
  - `examples/tts-cli.cpp`: CLI additions.
- `ggml-im2col-griddim-y.patch` — patch for the `ggml` submodule
  (https://github.com/ggml-org/ggml, base commit
  `68fee723b1f0c2432258b77710f3ca973b3bc5cc`, v0.9.6). The CUDA `im2col`
  kernels launched with `gridDim.y = OW`, which exceeds the CUDA hard limit
  of 65535 for long sequences (S3Gen decode of ~350+ speech frames, i.e.
  chunks longer than ~12-15 seconds) and aborted with
  `IM2COL failed: invalid configuration argument`. The patch loops over the
  Y grid dimension exactly like the existing Z-dimension loop.
- `gen_backbone_def.ps1` — helper script from the fork (DEF file generation
  for `ttsbackbone.dll`).
- `bake_speaker_conds.py` — bakes the default speaker conditioning
  (T3 `speaker_emb` / `cond_prompt_speech_tokens` / `emotion_adv`, S3Gen
  `prompt_token` / `prompt_feat` / `embedding`, extracted from ResembleAI
  `conds.pt`) into the codec GGUF, producing
  `chatterbox-mtl-codec-q4_k_m-baked-spk.gguf`. Tensor shapes matter:
  `s3g.cond.prompt_feat` ggml `[80,314,1]` = numpy `(314,80)`,
  `s3g.cond.embedding` ggml `[192,1]` = numpy `(1,192)`; pass tensors via
  `np.frombuffer(struct.pack(...))` or the GGUF corrupts.
- `bake_tokenizer.py` — tokenizer baking helper.

## Rebuild recipe (Windows, CUDA)

1. Clone `mybigday/codec.cpp` at the base commit above, apply
   `codec.cpp-local-changes.patch`; enter the `ggml` submodule at its base
   commit and apply `ggml-im2col-griddim-y.patch`.
2. `cmake -B build -S . -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=86`
   (RTX 3090; adjust arch for other GPUs). CUDA must be built from the same
   ggml sources — mixing an external `ggml-cuda.dll` with a freshly built
   `ggml.dll` segfaults on model load.
3. `cmake --build build --config Release --parallel`
4. Deploy to `%APPDATA%\com.patata.audiobookgenerator\resources\codec.cpp\`:
   `tts-cli.exe`, `ttsbackbone.dll`, `codec.dll`, and the runtime DLLs
   (`ggml*.dll` from `build/bin/Release`, `llama.dll`, `cudart64_12.dll`,
   `cublas64_12.dll`, `cublasLt64_12.dll`).
5. Replace the codec GGUF in `models/chatterbox/` with the baked-speaker one
   (same file name: `chatterbox-mtl-codec-Q4_K_M.gguf`).

## Distribution notes (open)

- The Chatterbox reference-audio picker in the UI is currently a no-op: the
  shipped codec GGUF has no voice encoder, so arbitrary voice cloning is
  unavailable; `--ref-audio` is ignored with a warning (see patch).
- macOS/Linux builds (Metal/Vulkan) are still TODO — the ggml patch is
  CUDA-only, other backends are unaffected.
- `engine_registry.json` still points to the upstream codec GGUF (no baked
  speaker). The baked GGUF must be hosted (e.g. HuggingFace) before a fresh
  install can download a working model.
