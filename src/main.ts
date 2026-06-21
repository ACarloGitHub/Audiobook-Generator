import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type PanelId =
  | "configuration"
  | "epub"
  | "generate"
  | "recovery"
  | "demo"
  | "models";

interface EngineInfo {
  id: string;
  display_name: string;
  format: "ONNX" | "GGUF";
  voice_cloning: boolean;
  hardware: string[];
  license: string;
  languages: string[];
}

interface EngineStatus {
  active_engine: string | null;
  active_model: string | null;
  vram_bytes: number | null;
  loaded_at: string | null;
  engines: EngineInfo[];
  hardware: HardwareSummary;
}

interface HardwareSummary {
  os: string;
  arch: string;
  gpus: GpuInfo[];
}

interface GpuInfo {
  vendor: string;
  model: string;
  vram_bytes: number;
  backend: string;
}

interface VoiceDescriptor {
  id: string;
  display_name: string;
  language: string;
}

interface EngineDefaults {
  engine_id: string;
  chunk_strategy: string;
  chunk_min_words: number | null;
  chunk_max_words: number | null;
  chunk_max_chars: number;
  chunk_max_chars_by_lang: Record<string, number>;
  separator: string;
  replace_guillemets: boolean;
  voice_cloning: boolean;
  needs_reference_transcript: boolean;
  supported_languages: string[];
  voices: VoiceDescriptor[];
}

interface ChapterSummary {
  title: string;
  char_count: number;
}

interface BookInfo {
  title: string;
  chapters: ChapterSummary[];
}

let currentPanel: PanelId = "generate";
let engineStatus: EngineStatus = {
  active_engine: null,
  active_model: null,
  vram_bytes: null,
  loaded_at: null,
  engines: [],
  hardware: { os: "unknown", arch: "unknown", gpus: [] },
};

let bookInfo: BookInfo | null = null;

// User-controlled state.
const state = {
  selectedEngineId: "kokoro",
  selectedLanguage: "it",
  selectedVoiceId: "if_sara",
  selectedSeparator: ".",
  replaceGuillemets: false,
  chunkStrategy: "Character Limit" as "Word Count Approx" | "Character Limit",
  chunkMinWords: 100,
  chunkMaxWords: 500,
  chunkMaxChars: 2300,
  referenceWavPath: null as string | null,
  referenceTranscript: "",
  epubPath: null as string | null,
  audioBookTitle: "",
  deleteIntermediateChunks: false,
  selectedChapters: new Set<string>(),
  demoOutputPath: null as string | null,
};

const PANEL_TITLES: Record<PanelId, string> = {
  configuration: "Configuration",
  epub: "EPUB & Options",
  generate: "Generate",
  recovery: "Error Recovery",
  demo: "Demo & Test",
  models: "Models",
};

const NAV_ITEMS: { id: PanelId; label: string }[] = [
  { id: "configuration", label: "Configuration" },
  { id: "epub", label: "EPUB & Options" },
  { id: "generate", label: "Generate" },
  { id: "recovery", label: "Error Recovery" },
  { id: "demo", label: "Demo & Test" },
  { id: "models", label: "Models" },
];

const SEPARATOR_OPTIONS = [
  { value: ".", label: "Standard Period (.)" },
  { value: "|", label: "Pipe (|)" },
  { value: ";", label: "Semicolon (;)" },
  { value: "<sil>", label: "Silence Tag (<sil>)" },
  { value: "[PAUSE]", label: "Pause Tag ([PAUSE])" },
  { value: "_", label: "Underscore (_)" },
];

function $(sel: string): HTMLElement {
  const el = document.querySelector(sel);
  if (!el) throw new Error(`Missing element: ${sel}`);
  return el as HTMLElement;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function bytesToGB(n: number | null): string {
  if (n === null || n === undefined) return "?";
  return (n / 1024 / 1024 / 1024).toFixed(2);
}

function hardwareLine(): string {
  const hw = engineStatus.hardware;
  const gpu = hw.gpus[0];
  if (!gpu) return `${hw.os} · ${hw.arch} · no GPU detected`;
  return `${hw.os} · ${gpu.vendor} ${gpu.model} · ${bytesToGB(gpu.vram_bytes)} GB VRAM`;
}

// ---------- Engine strip (always visible) ----------

function renderEngineStrip(): string {
  if (!engineStatus.active_engine) {
    return `
      <div class="engine-strip engine-strip-idle">
        <span class="engine-strip-msg">No engine loaded. Open <strong>Models</strong> to load one.</span>
      </div>`;
  }
  return `
    <div class="engine-strip">
      <span class="engine-strip-msg">
        Engine loaded: <strong>${escapeHtml(engineStatus.active_engine)}</strong>
        ${engineStatus.active_model ? `· model <code>${escapeHtml(engineStatus.active_model)}</code>` : ""}
        · ~${bytesToGB(engineStatus.vram_bytes)} GB VRAM
        ${engineStatus.loaded_at ? `· loaded ${escapeHtml(engineStatus.loaded_at)}` : ""}
      </span>
      <button class="btn-secondary" id="release-engine-btn">⏏ Release engine</button>
    </div>`;
}

// ---------- Sidebar ----------

function renderSidebar(): string {
  return NAV_ITEMS.map((it) => `
    <li class="nav-item ${it.id === currentPanel ? "active" : ""}" data-panel="${it.id}">
      <span class="nav-label">${it.label}</span>
    </li>
  `).join("");
}

// ---------- Panel bodies ----------

function engineOptions(): string {
  return engineStatus.engines
    .map(
      (e) =>
        `<option value="${escapeHtml(e.id)}" ${e.id === state.selectedEngineId ? "selected" : ""}>${escapeHtml(e.display_name)} · ${escapeHtml(e.format)} · ${escapeHtml(e.license)}</option>`,
    )
    .join("");
}

async function applyEngineDefaults(engineId: string): Promise<void> {
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

function panelConfiguration(): string {
  return `
    ${renderEngineStrip()}
    <div class="card">
      <h2>TTS Engine and Voice</h2>
      <label class="field-label">TTS Model</label>
      <select id="engine-select" class="select">${engineOptions()}</select>

      <div id="reference-wav-row" class="field-row">
        <label class="field-label">Upload Reference WAV (.wav)</label>
        <button class="btn-secondary" id="pick-reference-wav-btn">Drop File Here / Click to Upload</button>
        <p class="field-help" id="reference-wav-path">${state.referenceWavPath ? escapeHtml(state.referenceWavPath) : "No reference audio selected."}</p>
      </div>

      <div id="reference-transcript-row" class="field-row">
        <label class="field-label">Reference transcript</label>
        <textarea class="text-input" rows="2" id="reference-transcript" placeholder="Exact transcription of the reference audio (required for Voice Clone)">${escapeHtml(state.referenceTranscript)}</textarea>
      </div>

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
  `;
}

function panelEpub(): string {
  const wordGroupVisible = state.chunkStrategy === "Word Count Approx";
  const charGroupVisible = state.chunkStrategy === "Character Limit";
  const sepOptions = SEPARATOR_OPTIONS.map(
    (o) =>
      `<option value="${escapeHtml(o.value)}" ${o.value === state.selectedSeparator ? "selected" : ""}>${escapeHtml(o.label)}</option>`,
  ).join("");

  return `
    <div class="card">
      <h2>Upload EPUB</h2>
      <button class="btn-secondary btn-large" id="pick-epub-btn">Drop File Here / Click to Upload</button>
      <p class="field-help" id="epub-path">${state.epubPath ? escapeHtml(state.epubPath) : "No EPUB selected."}</p>
    </div>

    <div class="card">
      <h2>Audiobook Title</h2>
      <textarea class="text-input" rows="1" id="audiobook-title" placeholder="Enter title or leave blank">${escapeHtml(state.audioBookTitle)}</textarea>
    </div>

    <div class="card">
      <h2>Text cleanup</h2>
      <label class="checkbox-row">
        <input type="checkbox" id="replace-guillemets" ${state.replaceGuillemets ? "checked" : ""} />
        <span>Replace Guillemets (« »)</span>
      </label>
      <label class="field-label">Sentence Separator</label>
      <select class="select" id="separator-select">${sepOptions}</select>
    </div>

    <div class="card">
      <h2>Chunking strategy</h2>
      <div class="radio-row">
        <label class="radio-pill">
          <input type="radio" name="chunk-strategy" value="Word Count Approx" ${state.chunkStrategy === "Word Count Approx" ? "checked" : ""} />
          <span>Word Count Approx</span>
        </label>
        <label class="radio-pill">
          <input type="radio" name="chunk-strategy" value="Character Limit" ${state.chunkStrategy === "Character Limit" ? "checked" : ""} />
          <span>Character Limit</span>
        </label>
      </div>

      <div class="group ${wordGroupVisible ? "" : "hidden"}" id="word-count-group">
        <label class="field-label">Min Words</label>
        <input type="number" class="num-input" id="min-words" min="10" step="10" value="${state.chunkMinWords}" />
        <label class="field-label">Max Words</label>
        <input type="number" class="num-input" id="max-words" min="50" step="10" value="${state.chunkMaxWords}" />
      </div>

      <div class="group ${charGroupVisible ? "" : "hidden"}" id="char-limit-group">
        <label class="field-label">Max Chars</label>
        <input type="number" class="num-input" id="max-chars" min="100" step="50" value="${state.chunkMaxChars}" />
        <p class="field-help">Max value auto-loaded from the selected engine on Configuration. Override manually if needed.</p>
      </div>
    </div>
  `;
}

function panelGenerate(): string {
  const chapterRows = bookInfo
    ? bookInfo.chapters
        .map((c) => {
          const checked = state.selectedChapters.has(c.title) ? "checked" : "";
          return `
            <label class="chapter-row">
              <input type="checkbox" class="chapter-cb" data-title="${escapeHtml(c.title)}" ${checked} />
              <span>${escapeHtml(c.title)} (${c.char_count} chars)</span>
            </label>`;
        })
        .join("")
    : "";
  const chapterStatus = bookInfo
    ? `Book: <strong>${escapeHtml(bookInfo.title)}</strong> · ${bookInfo.chapters.length} chapters`
    : "Drop an EPUB on the EPUB & Options panel to load chapters.";
  const canGenerate = bookInfo !== null && state.selectedChapters.size > 0;

  return `
    ${renderEngineStrip()}
    <div class="card">
      <div class="btn-row">
        <button class="btn-secondary" id="select-all-btn" ${bookInfo ? "" : "disabled"}>Select All</button>
        <button class="btn-secondary" id="invert-btn" ${bookInfo ? "" : "disabled"}>Invert</button>
      </div>
      <p class="field-help" id="chapter-status">${chapterStatus}</p>
      <div class="chapter-list" id="chapter-list">${chapterRows}</div>
    </div>

    <div class="card">
      <div class="btn-row btn-row-large">
        <button class="btn-primary" id="generate-btn" ${canGenerate ? "" : "disabled"}>Generate Audiobook</button>
        <button class="btn-stop" id="stop-btn" disabled>🛑 Stop</button>
      </div>
      <label class="checkbox-row">
        <input type="checkbox" id="delete-chunks" ${state.deleteIntermediateChunks ? "checked" : ""} />
        <span>Delete intermediate chunks?</span>
      </label>
    </div>

    <div class="card">
      <h2>Progress</h2>
      <textarea class="text-input log-area" id="progress-log" rows="12" readonly placeholder="No generation running."></textarea>
    </div>
  `;
}

function panelRecovery(): string {
  return `
    <div class="card">
      <h2>🔄 Synthesis Errors Recovery System</h2>
      <div class="row">
        <div class="col">
          <label class="field-label">Audiobooks with Errors</label>
          <select class="select" id="recovery-book-select"><option>— scan —</option></select>
        </div>
        <div class="col">
          <label class="field-label">Chapters with Errors</label>
          <select class="select" id="recovery-chapter-select"><option>— pick a book —</option></select>
        </div>
        <div class="col-auto">
          <button class="btn-secondary btn-large" id="recovery-refresh-btn">🔄 Refresh</button>
        </div>
      </div>

      <h3>Failed Chunks</h3>
      <textarea class="text-input log-area" id="failed-chunks" rows="10" readonly placeholder="Pick a book and chapter to see the failed chunks for that chapter."></textarea>

      <div class="btn-row btn-row-large">
        <button class="btn-primary" id="retry-btn" disabled>🔁 Retry Synthesis</button>
        <button class="btn-secondary" id="merge-btn" disabled>🔗 Merge All Chunks</button>
        <button class="btn-stop" id="recovery-stop-btn" disabled>🛑 Stop</button>
      </div>

      <details class="accordion">
        <summary>✏️ Manual Editing</summary>
        <p class="field-help">Per-chunk manual override and split (TBD).</p>
      </details>
    </div>
  `;
}

function panelDemo(): string {
  return `
    <div class="card">
      <h2>Demo synthesis</h2>
      <label class="field-label">Text</label>
      <textarea class="text-input" id="demo-text" rows="3" placeholder="Type a sentence to synthesize..."></textarea>
      <div class="btn-row">
        <button class="btn-secondary" id="demo-pick-output-btn">Choose output path...</button>
        <span class="field-help" id="demo-output-path">${state.demoOutputPath ? escapeHtml(state.demoOutputPath) : "Default: <output_dir>/demo_<timestamp>.wav"}</span>
      </div>
      <button class="btn-secondary btn-large" id="demo-generate-btn" disabled>Generate Demo</button>
      <label class="field-label">Status</label>
      <textarea class="text-input" id="demo-status" rows="1" readonly placeholder="Status"></textarea>
      <audio id="demo-audio" controls style="display:none; width:100%; margin-top:8px;"></audio>
    </div>

    <div class="card">
      <h2>Test file generation</h2>
      <p class="field-help">Runs bundled mini-EPUBs end-to-end through the same pipeline as a real book.</p>
      <button class="btn-secondary btn-large" id="test-file-btn">Run Test File Generation</button>
      <label class="field-label">Test Status</label>
      <textarea class="text-input log-area" id="test-status" rows="8" readonly placeholder="No test run yet."></textarea>
    </div>
  `;
}

function panelModels(): string {
  const enginesList = engineStatus.engines
    .map(
      (e) => `
      <li>
        <span class="status-dot"></span>
        <strong>${escapeHtml(e.display_name)}</strong>
        · ${escapeHtml(e.format)} · ${escapeHtml(e.license)}
        · ${e.hardware.join(", ")}
        · languages: ${e.languages.length}
        · voice cloning: ${e.voice_cloning ? "yes" : "no"}
      </li>`,
    )
    .join("");
  const gpu = engineStatus.hardware.gpus[0];

  return `
    ${renderEngineStrip()}
    <div class="card">
      <h2>📦 TTS Engines Status</h2>
      <ul class="engine-list">${enginesList}</ul>
    </div>

    <div class="card">
      <div class="row">
        <div class="col">
          <label class="field-label">Summary</label>
          <textarea class="text-input" rows="3" readonly>Available engines: ${engineStatus.engines.length}
Installed: ${engineStatus.engines.length}
Missing: 0</textarea>
        </div>
        <div class="col-auto">
          <button class="btn-secondary btn-large" id="models-update-btn">🔄 Update Status</button>
        </div>
      </div>
    </div>

    <div class="card">
      <h2>🛒 Select Engines to Download</h2>
      <div class="checkbox-grid">
        ${engineStatus.engines
          .map(
            (e) => `
          <label class="checkbox-row">
            <input type="checkbox" />
            <span>${escapeHtml(e.display_name)}</span>
          </label>`,
          )
          .join("")}
      </div>
      <div class="btn-row btn-row-large">
        <button class="btn-primary">📥 Download Selected Engines</button>
        <button class="btn-stop">🛑 Stop Download</button>
        <button class="btn-secondary">✓ Select All Missing</button>
        <button class="btn-secondary">✗ Deselect All</button>
      </div>
      <label class="field-label">Download Log</label>
      <textarea class="text-input log-area" rows="8" readonly placeholder="Download details will appear here..."></textarea>
    </div>

    <div class="card">
      <h2>🛠 Runtime Binaries</h2>
      <p class="field-help">
        FFmpeg: ✅ found in system PATH<br/>
        llama-server: ❌ not installed (install via wizard)<br/>
        CUDA runtime: ${gpu ? `✅ ${escapeHtml(gpu.model)} (${bytesToGB(gpu.vram_bytes)} GB)` : "❌ not available"}
      </p>
      <p class="field-help">If FFmpeg is unavailable, the app uses a slower pure-Rust merge fallback.</p>
      <div class="btn-row">
        <button class="btn-secondary">📥 Install/Reinstall FFmpeg</button>
        <button class="btn-secondary">🔄 Update Status</button>
      </div>
      <label class="field-label">Installation Log</label>
      <textarea class="text-input log-area" rows="6" readonly placeholder="Installation details will appear here..."></textarea>
      <details class="accordion">
        <summary>📖 Manual Instructions</summary>
        <p class="field-help">
          Windows: <code>choco install ffmpeg</code><br/>
          macOS: <code>brew install ffmpeg</code><br/>
          Linux: <code>sudo apt install ffmpeg</code> (Debian/Ubuntu) or <code>sudo dnf install ffmpeg</code> (Fedora)
        </p>
      </details>
    </div>

    <div class="card">
      <h2>💻 Hardware</h2>
      <p class="field-help">${escapeHtml(hardwareLine())}</p>
    </div>
  `;
}

function panelBody(): string {
  switch (currentPanel) {
    case "configuration": return panelConfiguration();
    case "epub": return panelEpub();
    case "generate": return panelGenerate();
    case "recovery": return panelRecovery();
    case "demo": return panelDemo();
    case "models": return panelModels();
  }
}

function renderMainPanel(): string {
  const title = PANEL_TITLES[currentPanel];
  return `<section class="panel">
    <h1 class="panel-title">${escapeHtml(title)}</h1>
    ${panelBody()}
  </section>`;
}

function render(): void {
  const app = $("#app");
  app.innerHTML = `
    <aside class="sidebar">
      <div class="sidebar-header">
        <h1 class="sidebar-title">Audiobook Generator</h1>
        <p class="sidebar-version">v0.1.0</p>
      </div>
      <nav><ul class="nav-list">${renderSidebar()}</ul></nav>
      <div class="sidebar-footer">
        <p class="sidebar-footer-label">${escapeHtml(hardwareLine())}</p>
        <details class="sidebar-about">
          <summary>About</summary>
          <p class="sidebar-footer-detail">Built with Tauri 2.5 · llama-server + ort · MIT license</p>
        </details>
      </div>
    </aside>
    <main class="main">${renderMainPanel()}</main>
  `;
  attachListeners();
}

// ---------- Event wiring ----------

function attachListeners(): void {
  for (const li of Array.from(document.querySelectorAll<HTMLElement>(".nav-item"))) {
    li.addEventListener("click", () => {
      currentPanel = li.dataset.panel as PanelId;
      render();
    });
  }

  const releaseBtn = document.getElementById("release-engine-btn");
  if (releaseBtn) {
    releaseBtn.addEventListener("click", async () => {
      try {
        await invoke("unload_engine");
        await refreshEngineStatus();
        render();
      } catch (e) {
        alert(`Failed to release engine: ${e}`);
      }
    });
  }

  // Configuration panel.
  const engineSelect = document.getElementById("engine-select") as HTMLSelectElement | null;
  if (engineSelect) {
    engineSelect.addEventListener("change", async () => {
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

  // EPUB Options panel.
  const pickEpubBtn = document.getElementById("pick-epub-btn");
  if (pickEpubBtn) {
    pickEpubBtn.addEventListener("click", async () => {
      try {
        const path = await open({ multiple: false, filters: [{ name: "EPUB", extensions: ["epub"] }] });
        if (typeof path === "string") {
          state.epubPath = path;
          await loadEpub(path);
        }
      } catch (e) {
        console.warn("dialog open failed:", e);
      }
    });
  }

  const audiobookTitle = document.getElementById("audiobook-title") as HTMLTextAreaElement | null;
  if (audiobookTitle) {
    audiobookTitle.addEventListener("input", () => {
      state.audioBookTitle = audiobookTitle.value;
    });
  }

  const sepSelect = document.getElementById("separator-select") as HTMLSelectElement | null;
  if (sepSelect) {
    sepSelect.addEventListener("change", () => {
      state.selectedSeparator = sepSelect.value;
    });
  }

  const guillemetCb = document.getElementById("replace-guillemets") as HTMLInputElement | null;
  if (guillemetCb) {
    guillemetCb.addEventListener("change", () => {
      state.replaceGuillemets = guillemetCb.checked;
    });
  }

  for (const r of Array.from(document.querySelectorAll<HTMLInputElement>("input[name='chunk-strategy']"))) {
    r.addEventListener("change", () => {
      if (r.checked) {
        state.chunkStrategy = r.value as "Word Count Approx" | "Character Limit";
        render();
      }
    });
  }

  const minW = document.getElementById("min-words") as HTMLInputElement | null;
  if (minW) minW.addEventListener("input", () => (state.chunkMinWords = Number(minW.value)));
  const maxW = document.getElementById("max-words") as HTMLInputElement | null;
  if (maxW) maxW.addEventListener("input", () => (state.chunkMaxWords = Number(maxW.value)));
  const maxC = document.getElementById("max-chars") as HTMLInputElement | null;
  if (maxC) maxC.addEventListener("input", () => (state.chunkMaxChars = Number(maxC.value)));

  // Generate panel.
  const selectAllBtn = document.getElementById("select-all-btn");
  if (selectAllBtn && bookInfo) {
    selectAllBtn.addEventListener("click", () => {
      const allSelected = bookInfo!.chapters.every((c) => state.selectedChapters.has(c.title));
      if (allSelected) {
        state.selectedChapters.clear();
      } else {
        state.selectedChapters = new Set(bookInfo!.chapters.map((c) => c.title));
      }
      render();
    });
  }

  const invertBtn = document.getElementById("invert-btn");
  if (invertBtn && bookInfo) {
    invertBtn.addEventListener("click", () => {
      const next = new Set<string>();
      for (const c of bookInfo!.chapters) {
        if (!state.selectedChapters.has(c.title)) next.add(c.title);
      }
      state.selectedChapters = next;
      render();
    });
  }

  for (const cb of Array.from(document.querySelectorAll<HTMLInputElement>(".chapter-cb"))) {
    cb.addEventListener("change", () => {
      const title = cb.dataset.title!;
      if (cb.checked) state.selectedChapters.add(title);
      else state.selectedChapters.delete(title);
      const genBtn = document.getElementById("generate-btn") as HTMLButtonElement | null;
      if (genBtn) genBtn.disabled = state.selectedChapters.size === 0;
    });
  }

  const generateBtn = document.getElementById("generate-btn");
  if (generateBtn) {
    generateBtn.addEventListener("click", async () => {
      const log = document.getElementById("progress-log") as HTMLTextAreaElement;
      log.value = "[INFO] Starting generation...\n";
      try {
        log.value += "[INFO] Calling backend (TBD)...\n";
        log.value += "[INFO] Backend command not yet wired to UI (Phase 2).\n";
      } catch (e) {
        log.value += `[ERROR] ${e}\n`;
      }
    });
  }

  const deleteChunks = document.getElementById("delete-chunks") as HTMLInputElement | null;
  if (deleteChunks) {
    deleteChunks.addEventListener("change", () => {
      state.deleteIntermediateChunks = deleteChunks.checked;
    });
  }
}

// ---------- Bootstrap ----------

async function loadEpub(path: string): Promise<void> {
  try {
    bookInfo = await invoke<BookInfo>("load_epub", { path });
    if (bookInfo && !state.audioBookTitle) {
      state.audioBookTitle = bookInfo.title;
    }
    state.selectedChapters = new Set(bookInfo.chapters.map((c) => c.title));
    render();
  } catch (e) {
    alert(`Failed to parse EPUB: ${e}`);
  }
}

async function refreshEngineStatus(): Promise<void> {
  try {
    engineStatus = await invoke<EngineStatus>("engine_status");
  } catch {
    engineStatus = {
      active_engine: null,
      active_model: null,
      vram_bytes: null,
      loaded_at: null,
      engines: [],
      hardware: { os: "unknown", arch: "unknown", gpus: [] },
    };
  }
}

async function main(): Promise<void> {
  await refreshEngineStatus();
  if (engineStatus.engines.length > 0) {
    state.selectedEngineId = engineStatus.engines[0].id;
  }
  await applyEngineDefaults(state.selectedEngineId);
  render();
  await listen("engine-status-changed", () => {
    refreshEngineStatus().then(render);
  });
}

void main();