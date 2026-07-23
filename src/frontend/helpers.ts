import { state } from "./state";

export function $(sel: string): HTMLElement {
  const el = document.querySelector(sel);
  if (!el) throw new Error(`Missing element: ${sel}`);
  return el as HTMLElement;
}

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function ts(): string {
  const d = new Date();
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function bytesToGB(n: number | null): string {
  if (n === null || n === undefined) return "?";
  return (n / 1024 / 1024 / 1024).toFixed(2);
}

export function pickOutputDir(bookTitle: string): string {
  const safe = bookTitle.replace(/[^a-zA-Z0-9-_ ]/g, "_").trim() || "audiobook";
  return `Generated_Audiobooks/${safe}`;
}

export function hardwareLine(status: import("./types").EngineStatus): string {
  const hw = status.hardware;
  const gpu = hw.gpus[0];
  if (!gpu) return `${hw.os} · ${hw.arch} · no GPU detected`;
  return `${hw.os} · ${gpu.vendor} ${gpu.model} · ${bytesToGB(gpu.vram_bytes)} GB VRAM`;
}

// Log textareas are re-created on every full re-render (e.g. when the
// engine loads mid-generation). Always look the element up fresh: writing
// to a stale reference would silently swallow the log lines.
export function appendLog(id: string, line: string): void {
  const el = document.getElementById(id) as HTMLTextAreaElement | null;
  if (el) {
    el.value += line;
    el.scrollTop = el.scrollHeight;
  }
}

export function setLog(id: string, text: string): void {
  const el = document.getElementById(id) as HTMLTextAreaElement | null;
  if (el) {
    el.value = text;
    el.scrollTop = el.scrollHeight;
  }
}

// Resolve an advanced-parameter value: live DOM input first (Configuration
// panel open), then the user's saved override, then the registry default.
// Without the override layer, edits were lost on every panel switch.
export function paramValue(id: string, registryKey?: string): string | undefined {
  const el = document.getElementById(id) as HTMLInputElement | null;
  if (el && el.value) return el.value;
  const override = state.engineParamOverrides[id];
  if (override) return override;
  if (registryKey) {
    const def = state.engineGeneration[registryKey]?.default;
    if (def !== undefined && def !== null) return String(def);
  }
  return undefined;
}

// Build the `extra` parameter map for the selected engine from the shared
// paramValue resolver. Used by Demo, Test and Generate so all three send
// exactly the same values the user sees in Configuration.
export function collectParamExtras(): Record<string, string> {
  const extra: Record<string, string> = {};
  const put = (id: string, key: string, regKey?: string) => {
    const v = paramValue(id, regKey);
    if (v) extra[key] = v;
  };
  if (state.selectedEngineId.startsWith("Qwen3-TTS")) {
    const instruct = paramValue("qwen-instruct-input") || state.qwenInstruct?.trim();
    if (instruct) extra["instruct"] = instruct;
    const refText = paramValue("qwen-ref-text") || state.referenceTranscript?.trim();
    if (refText) extra["ref_text"] = refText;
    put("qwen-temp", "temp", "temp");
    put("qwen-top-k", "top_k", "top_k");
    put("qwen-top-p", "top_p", "top_p");
    put("qwen-rep-pen", "rep_pen", "rep_pen");
    put("qwen-max-new", "max_new", "max_new");
    put("qwen-seed", "seed");
  } else if (state.selectedEngineId.startsWith("OuteTTS")) {
    put("oute-temperature", "temperature", "temperature");
    put("oute-top-k", "top_k", "top_k");
    put("oute-top-p", "top_p", "top_p");
    put("oute-min-p", "min_p", "min_p");
    put("oute-rep-pen", "repetition_penalty", "repetition_penalty");
    put("oute-max-tokens", "max_tokens", "max_tokens");
    if (state.outeSpeakerJsonPath) {
      extra["speaker_json"] = state.outeSpeakerJsonPath;
    } else if (state.selectedVoiceId) {
      extra["speaker"] = state.selectedVoiceId;
    }
    const ctx = state.engineGeneration["ctx_size"]?.default;
    if (ctx !== undefined && ctx !== null) extra["ctx_size"] = String(ctx);
  } else if (state.selectedEngineId.startsWith("VoxCPM2")) {
    extra["voice_mode"] = state.voxMode;
    if (state.voxMode === "design") {
      const desc = paramValue("vox-voice-description") || state.voxVoiceDescription?.trim();
      if (desc) extra["voice_description"] = desc;
    }
    if (state.voxMode === "ultimate") {
      const refText = paramValue("vox-ref-text") || state.referenceTranscript?.trim();
      if (refText) extra["prompt_text"] = refText;
    }
    put("vox-cfg", "cfg", "cfg");
    put("vox-timesteps", "timesteps", "timesteps");
    put("vox-steps", "steps", "steps");
    put("vox-seed", "seed");
  }
  return extra;
}