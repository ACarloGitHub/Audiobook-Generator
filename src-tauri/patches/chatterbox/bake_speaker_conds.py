# -*- coding: utf-8 -*-
"""
Bake speaker conditioning metadata into the codec_lm GGUF.

Adds:
  - codec.speaker.* keys (triggers speaker module init in codec.cpp)
  - codec.lm.chatterbox.has_builtin_conds = true
  - codec.lm.chatterbox.builtin.speaker_emb (256 f32)
  - codec.lm.chatterbox.builtin.cond_prompt_speech_tokens (150 i32)
  - codec.lm.chatterbox.builtin.emotion_adv (0.5)
"""
import sys
import struct
import numpy as np
import torch
import gguf

INPUT  = sys.argv[1] if len(sys.argv) > 1 else r"C:\Users\carlo\AppData\Local\Temp\opencode\gguf-work\chatterbox-mtl-codec-q4_k_m-baked.gguf"
CONDS  = r"C:\Users\carlo\AppData\Local\Temp\opencode\chatterbox-py-models\conds.pt"
OUTPUT = sys.argv[2] if len(sys.argv) > 2 else INPUT.replace(".gguf", "-spk.gguf")

# ── 1. Load conds.pt ──────────────────────────────────────────────
conds = torch.load(CONDS, map_location="cpu", weights_only=True)
t3c = conds["t3"]
genc = conds["gen"]

# T3 conditionals
speaker_emb = t3c["speaker_emb"].flatten().to(torch.float32).tolist()
cond_tokens = t3c["cond_prompt_speech_tokens"].flatten().to(torch.int64).tolist()
emotion = float(t3c["emotion_adv"].flatten().item())  # 0.5

# S3Gen conditionals
gen_prompt_token = genc["prompt_token"].flatten().to(torch.int64).tolist()
gen_prompt_feat = genc["prompt_feat"].squeeze(0).to(torch.float32)  # (314, 80)
gen_embedding = genc["embedding"].flatten().to(torch.float32)       # (192,)

print(f"speaker_emb: len={len(speaker_emb)} first5={speaker_emb[:5]}")
print(f"cond_tokens: len={len(cond_tokens)} first10={cond_tokens[:10]}")
print(f"emotion: {emotion}")
print(f"gen_prompt_token: len={len(gen_prompt_token)}")
print(f"gen_prompt_feat: shape={list(gen_prompt_feat.shape)}")
print(f"gen_embedding: len={len(gen_embedding)}")

# ── 2. Read existing GGUF ─────────────────────────────────────────
reader = gguf.GGUFReader(INPUT)

# Collect existing KV pairs
existing_kv = []
for f in reader.fields.values():
    name = f.name.decode("utf-8") if isinstance(f.name, bytes) else f.name
    existing_kv.append((name, f))

# Collect existing tensors
tensors_info = []
for i in range(len(reader.tensors)):
    t = reader.tensors[i]
    tname = t.name.decode("utf-8") if isinstance(t.name, bytes) else t.name
    tensors_info.append({
        "name": tname,
        "shape": list(t.shape),
        "dtype": t.tensor_type,
        "data": t.data,
        "n_elements": int(np.prod(t.shape)),
    })

print(f"Existing: {len(existing_kv)} KV pairs, {len(tensors_info)} tensors")

# ── 3. Write new GGUF ─────────────────────────────────────────────
writer = gguf.GGUFWriter(OUTPUT, "chatterbox_s3g")

# Skip keys we will override or that are GGUF-internal or handled by writer
SKIP_KEYS = {"GGUF.version", "GGUF.tensor_count", "GGUF.kv_count", "general.architecture"}

def read_scalar_field(field):
    """Read a scalar field value from its parts."""
    t = field.types[0]
    raw = bytes(field.parts[-1])
    if t == gguf.GGUFValueType.STRING:
        return raw.decode("utf-8", errors="replace")
    elif t == gguf.GGUFValueType.UINT32:
        return struct.unpack("<I", raw[:4])[0]
    elif t == gguf.GGUFValueType.INT32:
        return struct.unpack("<i", raw[:4])[0]
    elif t == gguf.GGUFValueType.FLOAT32:
        return struct.unpack("<f", raw[:4])[0]
    elif t == gguf.GGUFValueType.BOOL:
        return bool(raw[0])
    elif t == gguf.GGUFValueType.UINT64:
        return struct.unpack("<Q", raw[:8])[0]
    return None

def read_array_field(field):
    """Read an array field using field.parts (skip metadata parts)."""
    arr_type = field.types[1] if len(field.types) > 1 else None
    # parts layout: [name_len, name_bytes, val_type, elem_type, count, data...]
    data_parts = field.parts[5:]
    if arr_type == gguf.GGUFValueType.STRING:
        return None  # String arrays not handled
    elif arr_type == gguf.GGUFValueType.UINT32:
        return [int(struct.unpack("<I", bytes(p[:4]))[0]) for p in data_parts]
    elif arr_type == gguf.GGUFValueType.INT32:
        return [int(struct.unpack("<i", bytes(p[:4]))[0]) for p in data_parts]
    elif arr_type == gguf.GGUFValueType.FLOAT32:
        return [float(struct.unpack("<f", bytes(p[:4]))[0]) for p in data_parts]
    elif arr_type == gguf.GGUFValueType.UINT64:
        return [int(struct.unpack("<Q", bytes(p[:8]))[0]) for p in data_parts]
    return None

# Copy existing KV pairs
for name, field in existing_kv:
    if name in SKIP_KEYS:
        continue

    types = field.types
    if not types:
        continue
    t = types[0]

    if t == gguf.GGUFValueType.STRING:
        writer.add_string(name, read_scalar_field(field))
    elif t == gguf.GGUFValueType.UINT32:
        writer.add_uint32(name, read_scalar_field(field))
    elif t == gguf.GGUFValueType.INT32:
        writer.add_int32(name, read_scalar_field(field))
    elif t == gguf.GGUFValueType.FLOAT32:
        writer.add_float32(name, read_scalar_field(field))
    elif t == gguf.GGUFValueType.BOOL:
        writer.add_bool(name, read_scalar_field(field))
    elif t == gguf.GGUFValueType.UINT64:
        writer.add_uint64(name, read_scalar_field(field))
    elif t == gguf.GGUFValueType.ARRAY:
        arr = read_array_field(field)
        if arr is not None:
            writer.add_array(name, arr)  # plain Python list
        else:
            print(f"  SKIP array {name}")
    else:
        print(f"  SKIP {name} (type={t})")

# Add speaker encoder metadata
writer.add_bool("codec.speaker.has_encoder", True)
writer.add_string("codec.speaker.encoder_arch", "chatterbox_voice_encoder")
writer.add_uint32("codec.speaker.n_rows", 34)
writer.add_uint32("codec.speaker.hidden_dim", 1024)
writer.add_uint32("codec.speaker.speaker_emb_dim", 256)
writer.add_bool("codec.speaker.needs_ref_pcm", False)
writer.add_bool("codec.speaker.needs_ref_speech_tokens", True)
writer.add_bool("codec.speaker.needs_emotion_scalar", True)
writer.add_uint32("codec.speaker.ref_sample_rate", 0)
writer.add_float32("codec.speaker.emotion_default", 0.5)

# Add builtin conds
writer.add_bool("codec.lm.chatterbox.has_builtin_conds", True)
writer.add_array("codec.lm.chatterbox.builtin.speaker_emb", speaker_emb)  # plain list
writer.add_array("codec.lm.chatterbox.builtin.cond_prompt_speech_tokens", cond_tokens)  # plain list
writer.add_float32("codec.lm.chatterbox.builtin.emotion_adv", emotion)

# Add S3Gen builtin conditioning metadata
writer.add_bool("chatterbox_s3g.has_builtin_conditioning", True)
writer.add_int32("chatterbox_s3g.cond.prompt_token_len", len(gen_prompt_token))
writer.add_int32("chatterbox_s3g.cond.prompt_feat_frames", int(gen_prompt_feat.shape[0]))
writer.add_int32("chatterbox_s3g.cond.prompt_feat_dim", int(gen_prompt_feat.shape[1]))
writer.add_int32("chatterbox_s3g.cond.embedding_dim", len(gen_embedding))
writer.add_array("chatterbox_s3g.cond.prompt_token", gen_prompt_token)

# Copy tensors
for ti in tensors_info:
    ggml_type = ti["dtype"]  # Already ggml type enum
    writer.add_tensor(ti["name"], ti["data"], raw_dtype=gguf.GGMLQuantizationType(ggml_type))

# Add S3Gen conditioning tensors (F32)
# gen_prompt_feat is (314, 80) → ggml ne[0]=80, ne[1]=314
import struct as _struct
pf_flat = gen_prompt_feat.contiguous().view(-1).tolist()
pf_bytes = _struct.pack(f'<{len(pf_flat)}f', *pf_flat)
pf_np = np.frombuffer(pf_bytes, dtype=np.float32).reshape(314, 80)
writer.add_tensor("s3g.cond.prompt_feat", pf_np, raw_dtype=gguf.GGMLQuantizationType.F32)

# gen_embedding is (192,) → ggml ne[0]=192
emb_flat = gen_embedding.contiguous().view(-1).tolist()
emb_bytes = _struct.pack(f'<{len(emb_flat)}f', *emb_flat)
emb_np = np.frombuffer(emb_bytes, dtype=np.float32).reshape(1, 192)
writer.add_tensor("s3g.cond.embedding", emb_np, raw_dtype=gguf.GGMLQuantizationType.F32)

writer.write_header_to_file()
writer.write_kv_data_to_file()
writer.write_tensors_to_file()
writer.close()

print(f"\nWritten: {OUTPUT}")
