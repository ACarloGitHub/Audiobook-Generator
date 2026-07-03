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
    const qwenControls = isQwen ? renderQwenControls() : "";
    const outeControls = isOute ? renderOuteControls() : "";

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
    </div>
  `;
}

function renderQwenControls(): string {
    const mode = qwenModeFromEngineId(state.selectedEngineId);
    const modeBadge = `<p class="field-help">Mode: <strong>${mode}</strong></p>`;

    let modeControls = "";

    const langOptions = ["Auto", "Chinese", "English", "German", "Italian", "Portuguese", "Spanish", "Japanese", "Korean", "French", "Russian"]
        .map((l) => `<option value="${l}" ${state.selectedLanguage === l ? "selected" : ""}>${l}</option>`)
        .join("");

    if (mode === "Custom Voice") {
        const voices = ["Vivian", "Serena", "Uncle_Fu", "Dylan", "Eric", "Ryan", "Aiden", "Ono_Anna", "Sohee"];
        const voiceOptions = voices
            .map((v) => `<option value="${v}" ${state.selectedVoiceId === v ? "selected" : ""}>${v}</option>`)
            .join("");
        modeControls = `
          <div class="field-row">
            <label class="field-label">Preset Voice</label>
            <select class="select" id="qwen-speaker-select">${voiceOptions}</select>
          </div>
          <div class="field-row">
            <label class="field-label">Language</label>
            <select class="select" id="qwen-language-select">${langOptions}</select>
          </div>
          <div class="field-row">
            <label class="field-label">Additional Instructions (optional)</label>
            <input type="text" class="text-input" id="qwen-instruct-input" placeholder="Speak slowly. With excitement. In a calm tone." value="${escapeHtml(state.qwenInstruct || "")}" />
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
          <div class="field-row">
            <label class="field-label">Language</label>
            <select class="select" id="qwen-language-select">${langOptions}</select>
          </div>
        `;
    } else if (mode === "Voice Design") {
        const designLangs = ["Chinese", "English", "German", "Italian", "Portuguese", "Spanish", "Japanese", "Korean", "French", "Russian"]
            .map((l) => `<option value="${l}" ${state.selectedLanguage === l ? "selected" : ""}>${l}</option>`)
            .join("");
        modeControls = `
          <div class="field-row">
            <label class="field-label">Voice Description (in English)</label>
            <textarea class="text-input" rows="3" id="qwen-instruct-input" placeholder="A calm middle-aged male announcer with a deep voice">${escapeHtml(state.qwenInstruct || "")}</textarea>
          </div>
          <div class="field-row">
            <label class="field-label">Language</label>
            <select class="select" id="qwen-language-select">${designLangs}</select>
          </div>
        `;
    }

    return `
      ${modeBadge}
      ${modeControls}
      <details class="accordion">
        <summary>Advanced Settings</summary>
        <div class="field-row">
          <label class="field-label">Temperature</label>
          <input type="number" class="num-input" id="qwen-temp" min="0" max="2" step="0.05" value="0.9" />
        </div>
        <div class="field-row">
          <label class="field-label">Top-K</label>
          <input type="number" class="num-input" id="qwen-top-k" min="0" max="100" step="1" value="50" />
        </div>
        <div class="field-row">
          <label class="field-label">Top-P</label>
          <input type="number" class="num-input" id="qwen-top-p" min="0" max="1" step="0.05" value="1.0" />
        </div>
        <div class="field-row">
          <label class="field-label">Repetition Penalty</label>
          <input type="number" class="num-input" id="qwen-rep-pen" min="1" max="2" step="0.01" value="1.05" />
        </div>
        <div class="field-row">
          <label class="field-label">Max New Tokens</label>
          <input type="number" class="num-input" id="qwen-max-new" min="256" max="16384" step="256" value="8192" />
        </div>
        <div class="field-row">
          <label class="field-label">Seed (empty = random)</label>
          <input type="number" class="num-input" id="qwen-seed" placeholder="random" />
        </div>
      </details>
    `;
}

function renderOuteControls(): string {
    const langOptions = ["Auto", "English", "Chinese", "Dutch", "French", "Georgian", "German", "Hungarian", "Italian", "Japanese", "Korean", "Latvian", "Polish", "Russian", "Spanish"]
        .map((l) => `<option value="${l}" ${state.selectedLanguage === l ? "selected" : ""}>${l}</option>`)
        .join("");

    return `
      <p class="field-help">Mode: <strong>Voice Clone</strong></p>
      <div class="field-row">
        <label class="field-label">Reference Audio (~10s, .wav mono)</label>
        <button class="btn-secondary" id="pick-reference-wav-btn">${state.referenceWavPath ? escapeHtml(state.referenceWavPath) : "Click to select a WAV file"}</button>
      </div>
      <div class="field-row">
        <label class="field-label">Language</label>
        <select class="select" id="qwen-language-select">${langOptions}</select>
      </div>
      <details class="accordion">
        <summary>Advanced Settings</summary>
        <div class="field-row">
          <label class="field-label">Temperature</label>
          <input type="number" class="num-input" id="oute-temperature" min="0" max="2" step="0.05" value="0.4" />
        </div>
        <div class="field-row">
          <label class="field-label">Top-K</label>
          <input type="number" class="num-input" id="oute-top-k" min="1" max="100" step="1" value="40" />
        </div>
        <div class="field-row">
          <label class="field-label">Top-P</label>
          <input type="number" class="num-input" id="oute-top-p" min="0" max="1" step="0.05" value="0.9" />
        </div>
        <div class="field-row">
          <label class="field-label">Min-P</label>
          <input type="number" class="num-input" id="oute-min-p" min="0" max="1" step="0.01" value="0.05" />
        </div>
        <div class="field-row">
          <label class="field-label">Repetition Penalty</label>
          <input type="number" class="num-input" id="oute-rep-pen" min="1" max="2" step="0.01" value="1.1" />
        </div>
        <div class="field-row">
          <label class="field-label">Max Tokens</label>
          <input type="number" class="num-input" id="oute-max-tokens" min="256" max="16384" step="256" value="8192" />
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

        if (d.voices.length > 0 && !state.selectedVoiceId) {
            state.selectedVoiceId = d.voices[0].id;
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

    const langSelect = document.getElementById("qwen-language-select") as HTMLSelectElement | null;
    if (langSelect) {
        langSelect.addEventListener("change", () => {
            state.selectedLanguage = langSelect.value;
        });
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
                    render();
                }
            } catch (e) {
                console.warn("dialog open failed:", e);
            }
        });
    }

    const advIds = ["qwen-temp", "qwen-top-k", "qwen-top-p", "qwen-rep-pen", "qwen-max-new", "qwen-seed",
        "oute-temperature", "oute-top-k", "oute-top-p", "oute-min-p", "oute-rep-pen", "oute-max-tokens"];
    for (const id of advIds) {
        const el = document.getElementById(id) as HTMLInputElement | null;
        if (el) {
            el.addEventListener("input", () => {
                (el as any)._qwenParam = el.value;
            });
        }
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
