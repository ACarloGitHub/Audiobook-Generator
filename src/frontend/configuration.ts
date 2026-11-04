import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { escapeHtml } from "./helpers";
import { renderEngineStrip } from "./engine-strip";
import { state } from "./state";
import type { EngineStatus, EngineDefaults } from "./types";

type QwenMode = "Voice Clone" | "Custom Voice" | "Voice Design";

function qwenModeFromEngineId(id: string): QwenMode {
    if (id.includes("VoiceDesign")) return "Voice Design";
    if (id.includes("CustomVoice")) return "Custom Voice";
    return "Voice Clone";
}

export function renderConfiguration(status: EngineStatus): string {
    const engineOptions = status.engines
        .map((e) => {
            const label = e.installed
                ? `${e.display_name} · ${e.format} · ${e.license} · ${e.size_mb} MB`
                : `${e.display_name} — not downloaded`;
            const attr = e.installed ? "" : "disabled";
            const sel = e.id === state.selectedEngineId && e.installed ? "selected" : "";
            return `<option value="${escapeHtml(e.id)}" ${attr} ${sel}>${escapeHtml(label)}</option>`;
        })
        .join("");

    const isQwen = state.selectedEngineId.startsWith("Qwen3-TTS");
    const isOute = state.selectedEngineId.startsWith("OuteTTS");
    const isVox = state.selectedEngineId.startsWith("VoxCPM2");
    const isAudio8 = state.selectedEngineId.startsWith("Audio8");
    const qwenControls = isQwen ? renderQwenControls() : "";
    const outeControls = isOute ? renderOuteControls() : "";
    const voxControls = isVox ? renderVoxControls() : "";
    const audio8Controls = isAudio8 ? renderAudio8Controls() : "";

    return `
    ${renderEngineStrip(status)}
    <div class="card">
      <h2>TTS Engine and Voice</h2>
      <label class="field-label">TTS Model</label>
      <select id="engine-select" class="select">${engineOptions}</select>

      <p class="field-help">
        Engine is loaded automatically when you click Generate Audiobook. Use the engine strip header to release VRAM between books.
      </p>

      ${qwenControls}
      ${outeControls}
      ${voxControls}
      ${audio8Controls}
    </div>
  `;
}

// Default instruct for Qwen CustomVoice, pre-filled when empty: calm,
// even narration instead of the preset's theatrical default delivery.
const DEFAULT_NARRATION_INSTRUCT =
    "Read calmly and evenly, like a professional audiobook narrator";

function genDefault(key: string): string {
    const p = state.engineGeneration[key];
    if (!p || p.default === null || p.default === undefined) return "";
    return String(p.default);
}

// Slider + number field kept in sync, with min/max and a plain-language
// description from engine_registry.json (backup configuration_tab.py style).
// The number input keeps the canonical id so collectParamExtras and the
// override persistence keep working unchanged.
function paramSlider(id: string, label: string, regKey: string): string {
    const p = state.engineGeneration[regKey];
    const min = p?.min ?? 0;
    const max = p?.max ?? 100;
    const step = p?.step ?? 1;
    const value = state.engineParamOverrides[id] ?? genDefault(regKey);
    const desc = p?.description
        ? `<p class="param-desc">${escapeHtml(p.description)}</p>`
        : "";
    return `
      <div class="field-row param-row">
        <div class="param-head">
          <label class="field-label">${label}</label>
          <span class="param-range">min ${min} · max ${max}</span>
        </div>
        <div class="param-controls">
          <input type="range" class="param-slider" id="${id}-range" min="${min}" max="${max}" step="${step}" value="${value}" />
          <input type="number" class="num-input param-num" id="${id}" min="${min}" max="${max}" step="${step}" value="${value}" />
        </div>
        ${desc}
      </div>
    `;
}

function paramCheckbox(id: string, label: string, regKey: string): string {
    const def = state.engineGeneration[regKey]?.default;
    const override = state.engineParamOverrides[id];
    const checked = override !== undefined ? override === "true" : def === true;
    const desc = state.engineGeneration[regKey]?.description
        ? `<p class="param-desc">${escapeHtml(state.engineGeneration[regKey]!.description!)}</p>`
        : "";
    return `
      <div class="field-row param-row">
        <label class="checkbox-row">
          <input type="checkbox" id="${id}" ${checked ? "checked" : ""} />
          <span>${label}</span>
        </label>
        ${desc}
      </div>
    `;
}

/** Languages available for the selected engine, with "Auto" first. */
function languageOptions(): string[] {
    const base = state.engineSupportedLanguages.length > 0
        ? [...state.engineSupportedLanguages]
        : ["Auto", "Chinese", "English", "German", "Italian", "Portuguese", "Spanish", "Japanese", "Korean", "French", "Russian"];
    if (!base.includes("Auto")) base.unshift("Auto");
    return base;
}

function renderLanguageSelect(id: string, label: string, help?: string): string {
    const langs = languageOptions();
    // If the book language is not supported by this engine, show "Auto"
    // instead of an arbitrary first entry.
    const current = langs.includes(state.selectedLanguage) ? state.selectedLanguage : "Auto";
    const options = langs
        .map((l) => `<option value="${escapeHtml(l)}" ${current === l ? "selected" : ""}>${escapeHtml(l)}</option>`)
        .join("");
    return `
      <div class="field-row">
        <label class="field-label">${label}</label>
        <select class="select" id="${id}">${options}</select>
        ${help ? `<p class="field-help">${help}</p>` : ""}
      </div>
    `;
}

/** Reference recording summary + warnings (duration, pauses, transcript). */
function renderReferenceInfo(transcriptId?: string, maxSeconds = 30): string {
    if (!state.referenceWavPath) return "";
    const info = state.referenceInfo;
    if (!info) {
        return `<p class="field-help">Reference selected, but it is not a readable PCM WAV.</p>`;
    }
    const warnings: string[] = [];
    if (info.duration_seconds < 3) {
        warnings.push(`The reference is only ${info.duration_seconds.toFixed(1)} s: use at least 3 s of continuous speech.`);
    }
    if (info.duration_seconds > maxSeconds) {
        warnings.push(`The reference is ${info.duration_seconds.toFixed(1)} s: keep it within 3-${maxSeconds} s, it shares the engine context with the text.`);
    }
    if (info.longest_silence_seconds > 1.5) {
        warnings.push(`It contains a ${info.longest_silence_seconds.toFixed(1)} s pause: the model imitates it, which is the usual cause of long pauses after a few words. Prefer a continuous reading.`);
    } else if (info.silence_ratio > 0.35) {
        warnings.push(`${(info.silence_ratio * 100).toFixed(0)}% of the sample is silence: a continuous reading gives a steadier narration.`);
    }
    if (info.peak > 0.98) {
        warnings.push("The recording is close to clipping: lower its level before cloning, clipped samples degrade the voice.");
    }
    if (info.peak < 0.05) {
        warnings.push(`The recording is very quiet (peak ${(info.peak * 100).toFixed(0)}%): normalize it before cloning.`);
    }
    if (transcriptId) {
        const words = state.referenceTranscript.trim().split(/\s+/).filter(Boolean).length;
        if (words > 30) {
            warnings.push(`The transcript has ${words} words: keep the sample around 20-30 words.`);
        }
    }
    const summary = `${info.duration_seconds.toFixed(1)} s · ${info.sample_rate} Hz · ${info.channels} ch · longest pause ${info.longest_silence_seconds.toFixed(1)} s`;
    const warningHtml = warnings
        .map((w) => `<p class="param-desc" style="color:#b45309">⚠ ${escapeHtml(w)}</p>`)
        .join("");
    return `<p class="field-help">Reference: ${escapeHtml(summary)}</p>${warningHtml}`;
}

function transcriptWordCount(): number {
    return state.referenceTranscript.trim().split(/\s+/).filter(Boolean).length;
}

function renderQwenControls(): string {
    const mode = qwenModeFromEngineId(state.selectedEngineId);
    const modeBadge = `<p class="field-help">Mode: <strong>${mode}</strong></p>`;

    let modeControls = "";

    if (mode === "Custom Voice") {
        // Voices (with official native language) come from the registry via
        // engine_defaults; fall back to bare names if none arrived yet.
        const voices = state.engineVoices.length > 0
            ? state.engineVoices.map((v) => ({ id: v.id, label: `${v.display_name} — native: ${v.language}` }))
            : ["Vivian", "Serena", "Uncle_Fu", "Dylan", "Eric", "Ryan", "Aiden", "Ono_Anna", "Sohee"]
                .map((v) => ({ id: v, label: v }));
        const voiceOptions = voices
            .map((v) => `<option value="${v.id}" ${state.selectedVoiceId === v.id ? "selected" : ""}>${escapeHtml(v.label)}</option>`)
            .join("");
        modeControls = `
          <div class="field-row">
            <label class="field-label">Preset Voice</label>
            <select class="select" id="qwen-speaker-select">${voiceOptions}</select>
            <p class="param-desc">Official notice: each preset voice gives its best quality in its native language. All voices can speak Italian, but accent and quality may degrade.</p>
          </div>
          ${renderLanguageSelect("qwen-language-select", "Language")}
          <div class="field-row">
            <label class="field-label">Additional Instructions (pre-filled — edit or delete as you like)</label>
            <input type="text" class="text-input" id="qwen-instruct-input" value="${escapeHtml(state.qwenInstruct || "Read calmly and evenly, like a professional audiobook narrator")}" />
          </div>
        `;
    } else if (mode === "Voice Clone") {
        modeControls = `
          <div class="field-row">
            <label class="field-label">Reference Audio (3-20s, .wav)</label>
            <button class="btn-secondary" id="pick-reference-wav-btn">${state.referenceWavPath ? escapeHtml(state.referenceWavPath) : "Click to select a WAV file"}</button>
          </div>
          <div class="field-row">
            <label class="field-label">Reference Transcription (optional — better quality if provided)</label>
            <textarea class="text-input" rows="2" id="qwen-ref-text" placeholder="Exact transcription of the reference audio">${escapeHtml(state.referenceTranscript || "")}</textarea>
          </div>
          ${renderReferenceInfo("qwen-ref-text", 20)}
          ${renderLanguageSelect("qwen-language-select", "Language")}
        `;
    } else if (mode === "Voice Design") {
        modeControls = `
          <div class="field-row">
            <label class="field-label">Voice Description (in English)</label>
            <textarea class="text-input" rows="3" id="qwen-instruct-input">${escapeHtml(state.qwenInstruct || "A calm middle-aged male announcer with a deep voice")}</textarea>
          </div>
          ${renderLanguageSelect("qwen-language-select", "Language")}
        `;
    }

    return `
      ${modeBadge}
      ${modeControls}
      <details class="accordion">
        <summary>Advanced Settings</summary>
        ${paramSlider("qwen-temp", "Temperature", "temp")}
        ${paramSlider("qwen-top-k", "Top-K", "top_k")}
        ${paramSlider("qwen-top-p", "Top-P", "top_p")}
        ${paramSlider("qwen-rep-pen", "Repetition Penalty", "rep_pen")}
        ${paramSlider("qwen-max-new", "Max New Tokens", "max_new")}
        ${paramSlider("qwen-sub-temp", "Sub-talker Temperature", "sub_temp")}
        ${paramSlider("qwen-sub-top-k", "Sub-talker Top-K", "sub_top_k")}
        ${paramSlider("qwen-sub-top-p", "Sub-talker Top-P", "sub_top_p")}
        ${paramSlider("qwen-codec-chunk-dur", "Codec Decode Window (s)", "codec_chunk_dur")}
        ${paramSlider("qwen-codec-left-dur", "Codec Left Context (s)", "codec_left_dur")}
        ${paramCheckbox("qwen-greedy", "Greedy decoding (no sampling)", "greedy")}
        ${paramCheckbox("qwen-no-fa", "Disable Flash Attention", "no_fa")}
        ${paramCheckbox("qwen-clamp-fp16", "Clamp FP16 (older NVIDIA cards)", "clamp_fp16")}
        <div class="field-row">
          <label class="field-label">Seed (empty = random)</label>
          <input type="number" class="num-input" id="qwen-seed" placeholder="random" />
        </div>
      </details>
    `;
}

function renderOuteControls(): string {
    const speakers = [
        { id: "it-male-narrator", label: "IT — Male Narrator (Italian)" },
        { id: "en-female-1-neutral", label: "EN — Female 1 Neutral (Default English)" },
    ];
    if (!speakers.some((s) => s.id === state.selectedVoiceId)) {
        state.selectedVoiceId = speakers[0].id;
    }
    const speakerOptions = speakers
        .map((s) => `<option value="${s.id}" ${state.selectedVoiceId === s.id ? "selected" : ""}>${escapeHtml(s.label)}</option>`)
        .join("");

    return `
      <p class="field-help">Mode: <strong>Voice Clone</strong> — OuteTTS auto-detects the language from input text</p>
      <div class="field-row">
        <label class="field-label">Speaker / Voice</label>
        <select class="select" id="oute-speaker-select">${speakerOptions}</select>
      </div>
      <div class="field-row">
        <label class="field-label">Custom Speaker Profile (optional .json)</label>
        <button class="btn-secondary" id="pick-speaker-json-btn">${state.outeSpeakerJsonPath ? escapeHtml(state.outeSpeakerJsonPath) : "Import a custom speaker JSON file..."}</button>
      </div>
      <details class="accordion">
        <summary>Advanced Settings</summary>
        ${paramSlider("oute-temperature", "Temperature", "temperature")}
        ${paramSlider("oute-top-k", "Top-K", "top_k")}
        ${paramSlider("oute-top-p", "Top-P", "top_p")}
        ${paramSlider("oute-min-p", "Min-P", "min_p")}
        ${paramSlider("oute-rep-pen", "Repetition Penalty", "repetition_penalty")}
        ${paramSlider("oute-repeat-last-n", "Repetition Window (tokens)", "repeat_last_n")}
        ${paramSlider("oute-max-tokens", "Max Tokens", "max_tokens")}
        <div class="field-row">
          <label class="field-label">Seed (empty = one random seed per book)</label>
          <input type="number" class="num-input" id="oute-seed" placeholder="auto" />
        </div>
      </details>
    `;
}

function renderVoxControls(): string {
    const mode = state.voxMode;
    const modeOptions = [
        { id: "design", label: "Voice Design — voice from a text description" },
        { id: "clone", label: "Controllable Cloning — clone a reference voice" },
        { id: "ultimate", label: "Ultimate Cloning — reference + transcript (max fidelity)" },
    ]
        .map((m) => `<option value="${m.id}" ${mode === m.id ? "selected" : ""}>${m.label}</option>`)
        .join("");

    let descControls = "";
    if (mode === "design") {
        descControls = `
      <div class="field-row">
        <label class="field-label">Voice Description (in English, required)</label>
        <textarea class="text-input" rows="2" id="vox-voice-description">${escapeHtml(state.voxVoiceDescription || "A calm middle-aged male narrator with a deep voice")}</textarea>
        <p class="param-desc">Voice Design has no reference sample, so the model invents the voice: name the language (e.g. "speaking Italian") and the intended delivery to steer accent and pace.</p>
      </div>`;
    }

    let cloneControls = "";
    if (mode === "clone" || mode === "ultimate") {
        cloneControls = `
          <div class="field-row">
            <label class="field-label">Reference Audio (WAV, 16-bit PCM, ~10-30s)</label>
            <button class="btn-secondary" id="pick-reference-wav-btn">${state.referenceWavPath ? escapeHtml(state.referenceWavPath) : "Click to select a WAV file"}</button>
          </div>
        `;
    }
    if (mode === "ultimate") {
        cloneControls += `
          <div class="field-row">
            <label class="field-label">Reference Transcription (required — exact text of the reference audio)</label>
            <textarea class="text-input" rows="2" id="vox-ref-text" placeholder="Exact transcription of the reference audio">${escapeHtml(state.referenceTranscript || "")}</textarea>
          </div>
        `;
    }

    return `
      <p class="field-help">VoxCPM2 auto-detects the language from input text (30 languages). Output is 48 kHz.</p>
      <div class="field-row">
        <label class="field-label">Voice Mode</label>
        <select class="select" id="vox-mode-select">${modeOptions}</select>
      </div>
      ${descControls}
      ${cloneControls}
      ${mode === "clone" || mode === "ultimate" ? renderReferenceInfo(mode === "ultimate" ? "vox-ref-text" : undefined) : ""}
      <details class="accordion">
        <summary>Advanced Settings</summary>
        ${paramSlider("vox-temperature", "Noise Temperature", "temperature")}
        ${paramSlider("vox-cfg", "CFG Guidance Scale", "cfg")}
        ${paramSlider("vox-timesteps", "CFM Timesteps", "timesteps")}
        ${paramSlider("vox-steps", "Max Decode Steps", "steps")}
        <div class="field-row">
          <label class="field-label">Seed (empty = random)</label>
          <input type="number" class="num-input" id="vox-seed" placeholder="random" />
        </div>
      </details>
    `;
}

// Audio8-TTS-Realistic: one mode only (voice clone). The reference recording
// and its exact transcript are both mandatory: the runtime refuses to start
// with a reference and no transcript, and without a reference the model invents
// a different voice on every call.
function renderAudio8Controls(): string {
    return `
      <p class="field-help">Audio8-TTS-Realistic auto-detects the language from the input text (23 languages) and copies the pace of the reference voice. Output is 44.1 kHz.</p>
      ${renderLanguageSelect(
        "audio8-language-select",
        "Language",
        "Optional hint for the runtime (used for text normalization). Autodetect works for all 23 languages; the hint is accepted for Italian, English, French, German, Spanish, Dutch, Polish, Chinese, Cantonese, Japanese and Korean."
      )}
      <div class="field-row">
        <label class="field-label">Reference Audio (WAV, 10-30 s — about 20-30 spoken words)</label>
        <button class="btn-secondary" id="pick-reference-wav-btn">${state.referenceWavPath ? escapeHtml(state.referenceWavPath) : "Click to select a WAV file"}</button>
        <p class="param-desc">
          Use a clean single-speaker recording with <strong>continuous reading and no long pauses</strong>: the model copies the pace, the rhythm and the pauses of this sample.
          Keep it short (10-30 s, roughly 20-30 words) because the sample and the text share a 2048-token context.
          A long or pause-riddled sample is the usual cause of long pauses after a few words, or of a different voice between blocks.
        </p>
        ${renderReferenceInfo("audio8-ref-text")}
      </div>
      <div class="field-row">
        <label class="field-label">Reference Transcription (required — exact text of the reference audio)</label>
        <textarea class="text-input" rows="2" id="audio8-ref-text" placeholder="Exact transcription of the reference audio">${escapeHtml(state.referenceTranscript || "")}</textarea>
        <p class="field-help" id="audio8-ref-words">Transcript: ${transcriptWordCount()} words (recommended 20-30).</p>
      </div>
      <details class="accordion">
        <summary>Advanced Settings</summary>
        ${paramSlider("audio8-temperature", "Temperature", "temperature")}
        ${paramSlider("audio8-top-p", "Top P", "top_p")}
        ${paramSlider("audio8-top-k", "Top K", "top_k")}
        ${paramSlider("audio8-max-tokens", "Max Tokens per Block", "max_tokens")}
        <div class="field-row">
          <label class="field-label">Seed (empty = random)</label>
          <input type="number" class="num-input" id="audio8-seed" placeholder="random" />
        </div>
      </details>
    `;
}

export async function applyEngineDefaults(engineId: string): Promise<void> {
    try {
        const d = await invoke<EngineDefaults>("engine_defaults", { engineId });
        state.chunkStrategy = d.chunk_strategy as "Word Count Approx" | "Character Limit";
        state.chunkMaxChars = d.chunk_max_chars;
        state.chunkMaxCharsByLang = d.chunk_max_chars_by_lang ?? {};
        if (d.chunk_min_words !== null) state.chunkMinWords = d.chunk_min_words;
        if (d.chunk_max_words !== null) state.chunkMaxWords = d.chunk_max_words;
        state.selectedSeparator = d.separator;
        state.replaceGuillemets = d.replace_guillemets;
        state.engineVoices = d.voices;
        state.engineSupportedLanguages = d.supported_languages;
        state.engineVoiceCloning = d.voice_cloning;
        state.engineGeneration = d.generation ?? {};

        if (d.voices.length > 0 && !state.selectedVoiceId) {
            state.selectedVoiceId = d.voices[0].id;
        }

        // Pre-fill a calm-narration instruct for Qwen CustomVoice when the
        // field is empty. The user can edit or delete it; it is re-applied
        // on engine change if left empty.
        if (engineId.includes("CustomVoice") && !state.qwenInstruct) {
            state.qwenInstruct = DEFAULT_NARRATION_INSTRUCT;
        }
    } catch (e) {
        console.warn("engine_defaults failed:", e);
    }
}

export function attachConfigurationListeners(render: () => void): void {
    const engineSelect = document.getElementById("engine-select") as HTMLSelectElement | null;
    if (engineSelect) {
        engineSelect.addEventListener("change", async () => {
            const status = await refreshEngineStatus();
            const selected = status.engines.find((e) => e.id === engineSelect.value);
            if (!selected || !selected.installed) {
                engineSelect.value = state.selectedEngineId;
                return;
            }
            state.selectedEngineId = engineSelect.value;
            await applyEngineDefaults(state.selectedEngineId);
            render();
        });
    }

    const speakerSelect = document.getElementById("qwen-speaker-select") as HTMLSelectElement | null;
    if (speakerSelect) {
        speakerSelect.addEventListener("change", () => {
            state.selectedVoiceId = speakerSelect.value;
        });
    }

    for (const id of ["qwen-language-select", "audio8-language-select"]) {
        const langSelect = document.getElementById(id) as HTMLSelectElement | null;
        if (langSelect) {
            langSelect.addEventListener("change", () => {
                state.selectedLanguage = langSelect.value;
            });
        }
    }

    const instructInput = document.getElementById("qwen-instruct-input") as HTMLInputElement | HTMLTextAreaElement | null;
    if (instructInput) {
        instructInput.addEventListener("input", () => {
            state.qwenInstruct = instructInput.value;
        });
    }

    const refTextInput = document.getElementById("qwen-ref-text") as HTMLTextAreaElement | null;
    if (refTextInput) {
        refTextInput.addEventListener("input", () => {
            state.referenceTranscript = refTextInput.value;
        });
    }

    const pickRefBtn = document.getElementById("pick-reference-wav-btn");
    if (pickRefBtn) {
        pickRefBtn.addEventListener("click", async () => {
            try {
                const path = await open({
                    multiple: false,
                    filters: [{ name: "WAV audio", extensions: ["wav"] }],
                });
                if (typeof path === "string") {
                    state.referenceWavPath = path;
                    try {
                        state.referenceInfo = await invoke<import("./types").ReferenceInfo | null>(
                            "inspect_reference_audio",
                            { path },
                        );
                    } catch (e) {
                        state.referenceInfo = null;
                        console.warn("inspect_reference_audio failed:", e);
                    }
                    render();
                }
            } catch (e) {
                console.warn("dialog open failed:", e);
            }
        });
    }

    const advIds = ["qwen-temp", "qwen-top-k", "qwen-top-p", "qwen-rep-pen", "qwen-max-new", "qwen-seed",
        "qwen-sub-temp", "qwen-sub-top-k", "qwen-sub-top-p", "qwen-codec-chunk-dur", "qwen-codec-left-dur",
        "oute-temperature", "oute-top-k", "oute-top-p", "oute-min-p", "oute-rep-pen", "oute-repeat-last-n",
        "oute-max-tokens", "oute-seed",
        "vox-temperature", "vox-cfg", "vox-timesteps", "vox-steps", "vox-seed"];
    for (const id of advIds) {
        const el = document.getElementById(id) as HTMLInputElement | null;
        if (!el) continue;
        const slider = document.getElementById(`${id}-range`) as HTMLInputElement | null;
        // Re-apply the saved override: the HTML template renders the
        // registry default, which would otherwise mask the user's value
        // after every panel switch (and win over the override when the
        // panel is open during generation).
        const saved = state.engineParamOverrides[id];
        if (saved !== undefined) {
            el.value = saved;
            if (slider) slider.value = saved;
        }
        // Clamp to the registry min/max so an out-of-range value typed by
        // hand can never reach the engine.
        const clamped = (raw: string): string => {
            const n = parseFloat(raw);
            if (Number.isNaN(n)) return raw;
            const min = el.min !== "" ? parseFloat(el.min) : -Infinity;
            const max = el.max !== "" ? parseFloat(el.max) : Infinity;
            const c = Math.min(max, Math.max(min, n));
            return String(c);
        };
        el.addEventListener("input", () => {
            const v = clamped(el.value);
            if (v !== el.value) el.value = v;
            if (slider) slider.value = v;
            state.engineParamOverrides[id] = v;
        });
        if (slider) {
            slider.addEventListener("input", () => {
                el.value = slider.value;
                state.engineParamOverrides[id] = slider.value;
            });
        }
    }

    const outeSpeakerSelect = document.getElementById("oute-speaker-select") as HTMLSelectElement | null;
    if (outeSpeakerSelect) {
        outeSpeakerSelect.addEventListener("change", () => {
            state.selectedVoiceId = outeSpeakerSelect.value;
        });
    }

    const pickSpeakerJsonBtn = document.getElementById("pick-speaker-json-btn");
    if (pickSpeakerJsonBtn) {
        pickSpeakerJsonBtn.addEventListener("click", async () => {
            try {
                const path = await open({
                    multiple: false,
                    filters: [{ name: "Speaker JSON", extensions: ["json"] }],
                });
                if (typeof path === "string") {
                    state.outeSpeakerJsonPath = path;
                    render();
                }
            } catch (e) {
                console.warn("dialog open failed:", e);
            }
        });
    }

    const voxModeSelect = document.getElementById("vox-mode-select") as HTMLSelectElement | null;
    if (voxModeSelect) {
        voxModeSelect.addEventListener("change", () => {
            state.voxMode = voxModeSelect.value as "design" | "clone" | "ultimate";
            render();
        });
    }

    const voxDescInput = document.getElementById("vox-voice-description") as HTMLTextAreaElement | null;
    if (voxDescInput) {
        voxDescInput.addEventListener("input", () => {
            state.voxVoiceDescription = voxDescInput.value;
        });
    }

    const voxRefTextInput = document.getElementById("vox-ref-text") as HTMLTextAreaElement | null;
    if (voxRefTextInput) {
        voxRefTextInput.addEventListener("input", () => {
            state.referenceTranscript = voxRefTextInput.value;
        });
    }

    const audio8RefTextInput = document.getElementById("audio8-ref-text") as HTMLTextAreaElement | null;
    if (audio8RefTextInput) {
        audio8RefTextInput.addEventListener("input", () => {
            state.referenceTranscript = audio8RefTextInput.value;
            const words = document.getElementById("audio8-ref-words");
            if (words) {
                const n = transcriptWordCount();
                words.textContent = `Transcript: ${n} words (recommended 20-30).`;
            }
        });
    }

    // Boolean engine flags: persisted like the numeric overrides so a panel
    // switch does not silently disable them.
    for (const id of ["qwen-greedy", "qwen-no-fa", "qwen-clamp-fp16"]) {
        const el = document.getElementById(id) as HTMLInputElement | null;
        if (!el) continue;
        const saved = state.engineParamOverrides[id];
        if (saved !== undefined) el.checked = saved === "true";
        el.addEventListener("change", () => {
            state.engineParamOverrides[id] = el.checked ? "true" : "false";
        });
    }

    const releaseBtn = document.getElementById("release-engine-btn");
    if (releaseBtn) {
        releaseBtn.addEventListener("click", async () => {
            try {
                await invoke("unload_engine", { engineId: state.selectedEngineId, modelId: state.selectedEngineId });
                render();
            } catch (e) {
                alert(`Failed to release engine: ${e}`);
            }
        });
    }
}

async function refreshEngineStatus(): Promise<EngineStatus> {
    try {
        return await invoke<EngineStatus>("engine_status");
    } catch (e) {
        console.error("engine_status failed:", e);
        return {
            active_engine: null, active_model: null, vram_bytes: null, loaded_at: null,
            engines: [], hardware: { os: "unknown", arch: "unknown", gpus: [] },
        };
    }
}
