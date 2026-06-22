import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { escapeHtml } from "./helpers";
import { renderEngineStrip } from "./engine-strip";
import { state } from "./state";
import type { EngineStatus, EngineDefaults } from "./types";

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

  return `
    ${renderEngineStrip(status)}
    <div class="card">
      <h2>TTS Engine and Voice</h2>
      <label class="field-label">TTS Model</label>
      <select id="engine-select" class="select">${engineOptions}</select>

      <div id="reference-wav-row" class="field-row">
        <label class="field-label">Upload Reference WAV (.wav)</label>
        <button class="btn-secondary" id="pick-reference-wav-btn">Drop File Here / Click to Upload</button>
        <p class="field-help" id="reference-wav-path">${state.referenceWavPath ? escapeHtml(state.referenceWavPath) : "No reference audio selected."}</p>
      </div>

      <div id="reference-transcript-row" class="field-row">
        <label class="field-label">Reference transcript</label>
        <textarea class="text-input" rows="2" id="reference-transcript" placeholder="Exact transcription of the reference audio (required for Voice Clone)">${escapeHtml(state.referenceTranscript)}</textarea>
      </div>

      <p class="field-help">
        Engine is loaded automatically when you click Generate Audiobook. Use the engine strip header to release VRAM between books.
      </p>

      <div class="field-row">
        <label class="field-label">Language</label>
        <select class="select" id="language-select">
          <option value="it" ${state.selectedLanguage === "it" ? "selected" : ""}>Italian</option>
          <option value="en" ${state.selectedLanguage === "en" ? "selected" : ""}>English</option>
          <option value="fr" ${state.selectedLanguage === "fr" ? "selected" : ""}>French</option>
          <option value="ja" ${state.selectedLanguage === "ja" ? "selected" : ""}>Japanese</option>
          <option value="zh-cn" ${state.selectedLanguage === "zh-cn" ? "selected" : ""}>Chinese</option>
        </select>
      </div>

      <div class="field-row" id="voice-row">
        <label class="field-label">Voice</label>
        <select class="select" id="voice-select">
          <option value="">— engine not yet selected —</option>
        </select>
      </div>
    </div>

    <div class="card">
      <h2>Engine parameters</h2>
      <div class="field-row">
        <label class="field-label">Speed</label>
        <input type="number" class="num-input" min="0.5" max="2.0" step="0.1" value="1.0" />
      </div>
      <p class="field-help">Defaults auto-load from the selected engine. Override any value; the override persists in localStorage.</p>
    </div>

    <div class="card">
      <h2>Debug</h2>
      <p class="field-help" id="debug-config">
        engines from backend: ${status.engines.length}
        · selected: ${escapeHtml(state.selectedEngineId)}
        · first id: ${escapeHtml(status.engines[0]?.id ?? "(none)")}
      </p>
    </div>
  `;
}

export async function applyEngineDefaults(engineId: string): Promise<void> {
  try {
    const d = await invoke<EngineDefaults>("engine_defaults", { engineId });
    state.chunkStrategy = d.chunk_strategy as "Word Count Approx" | "Character Limit";
    state.chunkMaxChars = d.chunk_max_chars;
    if (d.chunk_min_words !== null) state.chunkMinWords = d.chunk_min_words;
    if (d.chunk_max_words !== null) state.chunkMaxWords = d.chunk_max_words;
    state.selectedSeparator = d.separator;
    state.replaceGuillemets = d.replace_guillemets;
    if (d.supported_languages.length > 0) {
      state.selectedLanguage = d.supported_languages[0].toLowerCase().startsWith("it")
        ? "it"
        : d.supported_languages[0];
    }
    if (d.voices.length > 0) state.selectedVoiceId = d.voices[0].id;
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

  const refTranscript = document.getElementById("reference-transcript") as HTMLTextAreaElement | null;
  if (refTranscript) {
    refTranscript.addEventListener("input", () => {
      state.referenceTranscript = refTranscript.value;
    });
  }

  const releaseBtn = document.getElementById("release-engine-btn");
  if (releaseBtn) {
    releaseBtn.addEventListener("click", async () => {
      try {
        await invoke("unload_engine", { engineId: state.selectedEngineId, modelId: "kokoro-82M-quantized" });
        render();
      } catch (e) {
        alert(`Failed to release engine: ${e}`);
      }
    });
  }
}

async function refreshEngineStatus(): Promise<import("./types").EngineStatus> {
  try {
    return await invoke<import("./types").EngineStatus>("engine_status");
  } catch (e) {
    console.error("engine_status failed:", e);
    return {
      active_engine: null,
      active_model: null,
      vram_bytes: null,
      loaded_at: null,
      engines: [],
      hardware: { os: "unknown", arch: "unknown", gpus: [] },
    };
  }
}