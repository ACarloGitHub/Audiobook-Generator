"""Bake the multilingual BPE tokenizer into the Chatterbox codec GGUF.

Reads the existing codec GGUF, copies all KV pairs + tensors, adds the
tokenizer metadata from mtl_tokenizer.json, and writes a new GGUF.
"""

import json
import os
import sys

import numpy as np
from gguf import GGUFReader, GGUFWriter, GGUFValueType

CODEC_GGUF_IN  = r"C:\Users\carlo\AppData\Local\Temp\opencode\chatterbox-models\chatterbox-mtl-codec-q4_k_m.gguf"
TOKENIZER_JSON = r"C:\Users\carlo\AppData\Local\Temp\opencode\gguf-work\grapheme_mtl_merged_expanded_v1.json"
CODEC_GGUF_OUT = r"C:\Users\carlo\AppData\Local\Temp\opencode\gguf-work\chatterbox-mtl-codec-q4_k_m-baked.gguf"


def main():
    # ---- Step 1: Read tokenizer JSON ----
    print("Reading tokenizer...")
    with open(TOKENIZER_JSON, encoding="utf-8") as f:
        tj = json.load(f)

    model = tj.get("model", {})
    vocab = model.get("vocab", {})
    n_vocab = len(vocab)

    id_to_tok = [""] * n_vocab
    for tok, tid in vocab.items():
        if 0 <= tid < n_vocab:
            id_to_tok[tid] = tok

    merges_raw = model.get("merges", [])
    merges = []
    for m in merges_raw:
        if isinstance(m, (list, tuple)):
            merges.append(f"{m[0]} {m[1]}")
        else:
            merges.append(str(m))

    added = tj.get("added_tokens", [])
    added_pairs = [(a["content"], int(a["id"])) for a in added]
    unk = model.get("unk_token", "[UNK]")

    print(f"  {n_vocab} tokens, {len(merges)} merges, {len(added_pairs)} added tokens")

    # ---- Step 2: Read existing GGUF ----
    print("Reading existing GGUF...")
    reader = GGUFReader(CODEC_GGUF_IN)

    arch = "codec"
    if "general.architecture" in reader.fields:
        arch = bytes(reader.fields["general.architecture"].parts[-1]).decode("utf-8")

    # ---- Step 3: Create new GGUF ----
    print(f"Creating new GGUF (arch={arch})...")
    writer = GGUFWriter(CODEC_GGUF_OUT, arch)

    # ---- Step 4: Copy all existing KV pairs ----
    print("Copying KV pairs...")
    for name, field in reader.fields.items():
        if name.startswith("GGUF."):
            continue
        # Skip general.architecture — the GGUFWriter constructor already
        # adds it, and duplicating causes a warning + potential corruption.
        if name == "general.architecture":
            continue

        types = field.types
        if not types:
            continue

        t = types[0]

        if t == GGUFValueType.STRING:
            val = bytes(field.parts[-1]).decode("utf-8")
            writer.add_string(name, val)
        elif t == GGUFValueType.UINT32:
            writer.add_uint32(name, int(field.parts[-1][0]))
        elif t == GGUFValueType.INT32:
            writer.add_int32(name, int(field.parts[-1][0]))
        elif t == GGUFValueType.BOOL:
            writer.add_bool(name, bool(field.parts[-1][0]))
        elif t == GGUFValueType.FLOAT32:
            writer.add_float32(name, float(field.parts[-1][0]))
        elif t == GGUFValueType.UINT64:
            writer.add_uint64(name, int(field.parts[-1][0]))
        elif t == GGUFValueType.INT64:
            writer.add_int64(name, int(field.parts[-1][0]))
        elif t == GGUFValueType.UINT16:
            writer.add_uint16(name, int(field.parts[-1][0]))
        elif t == GGUFValueType.INT16:
            writer.add_int16(name, int(field.parts[-1][0]))
        elif t == GGUFValueType.UINT8:
            writer.add_uint8(name, int(field.parts[-1][0]))
        elif t == GGUFValueType.INT8:
            writer.add_int8(name, int(field.parts[-1][0]))
        elif t == GGUFValueType.FLOAT64:
            writer.add_float64(name, float(field.parts[-1][0]))
        elif t == GGUFValueType.ARRAY:
            elem_type = types[1] if len(types) > 1 else None
            # Array values are in the last part for numeric arrays.
            # field.data is unreliable for arrays in this gguf version.
            arr_part = field.parts[-1]
            if isinstance(arr_part, np.ndarray):
                vals = arr_part.flatten().tolist()
            else:
                vals = list(arr_part)
            if elem_type == GGUFValueType.UINT32:
                writer.add_array(name, [int(x) for x in vals])
            elif elem_type == GGUFValueType.INT32:
                writer.add_array(name, [int(x) for x in vals])
            elif elem_type == GGUFValueType.FLOAT32:
                writer.add_array(name, [float(x) for x in vals])
            else:
                print(f"  WARNING: skipping array {name} elem_type={elem_type}")
        else:
            print(f"  WARNING: skipping {name} type={t}")

    # ---- Step 5: Add tokenizer metadata ----
    print("Adding tokenizer metadata...")
    writer.add_string("codec.lm.chatterbox.tokenizer.model", "bpe")
    writer.add_uint32("codec.lm.chatterbox.tokenizer.n_vocab", n_vocab)
    writer.add_string("codec.lm.chatterbox.tokenizer.tokens", "\n".join(id_to_tok))
    writer.add_string("codec.lm.chatterbox.tokenizer.merges", "\n".join(merges))
    writer.add_string(
        "codec.lm.chatterbox.tokenizer.added",
        "\n".join(f"{c}\t{i}" for c, i in added_pairs),
    )
    writer.add_string("codec.lm.chatterbox.tokenizer.unk_token", str(unk))
    print(f"  Added: model=bpe, n_vocab={n_vocab}, merges={len(merges)}, added={len(added_pairs)}")

    # ---- Step 6: Copy all tensors ----
    n_tensors = len(reader.tensors)
    print(f"Copying {n_tensors} tensors...")
    for i, tensor in enumerate(reader.tensors):
        # Pass raw data as-is. The writer stores GGUF dims as reversed(shape),
        # and for quantized types, quant_shape_from_byte_shape converts the
        # uint8 byte shape (e.g. (6561, 288)) back to logical dims (e.g.
        # (6561, 512)), which the writer then reverses to match the original.
        writer.add_tensor(tensor.name, tensor.data, raw_dtype=tensor.tensor_type)

        if (i + 1) % 200 == 0:
            print(f"  {i+1}/{n_tensors}...")

    # ---- Step 7: Write ----
    print("Writing new GGUF...")
    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_tensors_to_file()
    writer.close()

    in_size = os.path.getsize(CODEC_GGUF_IN)
    out_size = os.path.getsize(CODEC_GGUF_OUT)
    print(f"Done!")
    print(f"  Input:  {in_size / 1e6:.1f} MB")
    print(f"  Output: {out_size / 1e6:.1f} MB")
    print(f"  Diff:   {(out_size - in_size) / 1e3:.1f} KB")


if __name__ == "__main__":
    main()
