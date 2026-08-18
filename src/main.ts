import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";
import { $, escapeHtml, hardwareLine } from "./frontend/helpers";
import { state, PANEL_TITLES } from "./frontend/state";
import type { EngineStatus, BookInfo, ModelListEntry } from "./frontend/types";
import { renderSidebar, attachSidebarListeners } from "./frontend/sidebar";
import { applyEngineDefaults, attachConfigurationListeners, renderConfiguration } from "./frontend/configuration";
import { renderEpub, attachEpubListeners } from "./frontend/epub-options";
import { renderGenerate, attachGenerateListeners } from "./frontend/generate";
import { renderRecovery, attachRecoveryListeners } from "./frontend/recovery";
import { renderDemo, attachDemoListeners } from "./frontend/demo";
import { renderModels, attachModelsListeners, loadModels } from "./frontend/models";
import { startVramMonitor, renderVramSlot } from "./frontend/engine-strip";
import { renderAgents, attachAgentsListeners } from "./frontend/agents";
import {
  renderHistory,
  attachHistoryListeners,
  loadHistory,
  confirmMissingFolder,
} from "./frontend/history";
import type { HistoryJobView } from "./frontend/types";
import {
  renderAurawrite,
  attachAurawriteListeners,
  checkAurawriteState,
  takeProposal,
  confirmProposal,
  showBusyWarning,
} from "./frontend/aurawrite";
import type { AurawriteState } from "./frontend/aurawrite";
import { initWizard, renderWizard, attachWizardListeners } from "./frontend/wizard";

let engineStatus: EngineStatus = {
  active_engine: null,
  active_model: null,
  vram_bytes: null,
  loaded_at: null,
  engines: [],
  hardware: { os: "unknown", arch: "unknown", gpus: [] },
};

let modelList: ModelListEntry[] = [];
let bookInfo: BookInfo | null = null;
let aurawriteLoaded = false;
let aurawriteState: AurawriteState = { found: false, books: [] };
let historyLoaded = false;
let historyJobs: HistoryJobView[] = [];

function panelBody(): string {
  switch (state.currentPanel) {
    case "configuration": return renderConfiguration(engineStatus);
    case "epub": return renderEpub();
    case "generate": return renderGenerate(engineStatus, bookInfo);
    case "recovery": return renderRecovery();
    case "history": return renderHistory(historyJobs, historyLoaded);
    case "demo": return renderDemo(engineStatus);
    case "models": return renderModels(engineStatus, modelList);
    case "agents": return renderAgents();
    case "aurawrite": return renderAurawrite(aurawriteState, aurawriteJobs(), aurawriteLoaded);
  }
}

function aurawriteJobs(): HistoryJobView[] {
  return historyJobs.filter((j) => j.aurawrite_book_id);
}

function renderMainPanel(): string {
  const title = PANEL_TITLES[state.currentPanel];
  return `<section class="panel">
    <h1 class="panel-title">${escapeHtml(title)}</h1>
    ${panelBody()}
  </section>`;
}

let showWizard = false;

// Snapshot every input/textarea/select value before a full re-render and
// restore it afterwards: switching panels must not wipe what the user
// typed (e.g. the Demo & Test text).
function snapshotFormValues(): Map<string, string | boolean> {
  const values = new Map<string, string | boolean>();
  document
    .querySelectorAll("#app input[id], #app textarea[id], #app select[id]")
    .forEach((el) => {
      if (el instanceof HTMLInputElement) {
        if (el.type === "file") return;
        values.set(el.id, el.type === "checkbox" || el.type === "radio" ? el.checked : el.value);
      } else if (el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) {
        values.set(el.id, el.value);
      }
    });
  return values;
}

// Fields whose value always comes from `state.*` (rendered by the panel)
// must NOT be restored from the DOM snapshot: a programmatic state update
// (e.g. auto-filling the audiobook title on EPUB load) would be overwritten
// by the stale pre-render DOM value.
const STATE_DRIVEN_FIELDS: Set<string> = new Set(["audiobook-title", "progress-log"]);

function restoreFormValues(values: Map<string, string | boolean>): void {
  for (const [id, value] of values) {
    // Advanced engine params are restored from state.engineParamOverrides
    // by the Configuration panel itself; the snapshot may hold a stale
    // registry default and must not overwrite the user's saved value.
    if (state.engineParamOverrides[id] !== undefined) continue;
    // Fields rendered from state.* (title, etc.) are already correct in the
    // freshly rendered HTML; restoring the old snapshot would wipe them.
    if (STATE_DRIVEN_FIELDS.has(id)) continue;
    const el = document.getElementById(id);
    if (!el) continue;
    if (el instanceof HTMLInputElement) {
      if (el.type === "checkbox" || el.type === "radio") {
        el.checked = Boolean(value);
      } else if (el.type !== "file") {
        el.value = String(value);
      }
    } else if (el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) {
      el.value = String(value);
    }
  }
}

function render(): void {
  if (showWizard) {
    const app = $("#app");
    app.innerHTML = renderWizard();
    attachWizardListeners(render, async () => {
      showWizard = false;
      await refreshAll();
      render();
    });
    return;
  }

  const savedValues = snapshotFormValues();
  const app = $("#app");
  app.innerHTML = `
    <aside class="sidebar">
      <div class="sidebar-header">
        <h1 class="sidebar-title">Audiobook Generator</h1>
        <p class="sidebar-version">v0.2.2</p>
      </div>
      <nav><ul class="nav-list">${renderSidebar(state.currentPanel)}</ul></nav>
      <div class="sidebar-footer">
        <p class="sidebar-footer-vram">${renderVramSlot()}</p>
        <p class="sidebar-footer-label">${escapeHtml(hardwareLine(engineStatus))}</p>
        <details class="sidebar-about">
          <summary>About</summary>
          <p class="sidebar-footer-detail">Built with Tauri 2.5 · llama-server + ort · MIT license</p>
        </details>
      </div>
    </aside>
    <main class="main">${renderMainPanel()}</main>
  `;
  attachAllListeners();
  restoreFormValues(savedValues);
}

function attachAllListeners(): void {
  attachSidebarListeners((panel) => {
    state.currentPanel = panel;
    render();
  });

  attachConfigurationListeners(render);
  attachEpubListeners(render, (info) => {
    bookInfo = info;
    state.selectedChapters = new Set(info.chapters.map((c) => c.title));
    render();
  });
  attachGenerateListeners(bookInfo, refreshEngineStatus);
  attachRecoveryListeners(render);
  attachDemoListeners();
  attachModelsListeners(async () => {
    await refreshAll();
    render();
  });
  attachAgentsListeners();
  attachAurawriteListeners();
  attachHistoryListeners();
}

async function refreshHistory(): Promise<void> {
  historyJobs = await loadHistory();
  historyLoaded = true;
  render();
}

async function refreshAurawrite(): Promise<void> {
  aurawriteState = await checkAurawriteState();
  aurawriteLoaded = true;
  render();
}

/** Consume a pending AuraWrite proposal: confirm, guard busy, load the book. */
async function checkProposalFlow(): Promise<void> {
  const proposal = await takeProposal();
  if (!proposal) return;
  const ok = await confirmProposal(proposal);
  if (!ok) return;
  if (state.generationRunning) {
    showBusyWarning();
    return;
  }
  await loadBookIntoGenerate(proposal.input);
}

/** Open a catalog book from the AuraWrite panel (Reader entries only). */
async function openCatalogBook(path: string): Promise<void> {
  if (state.generationRunning) {
    showBusyWarning();
    return;
  }
  await loadBookIntoGenerate(path);
}

/** Load an ebook and switch to the Generate panel (shared by proposal + Open). */
async function loadBookIntoGenerate(path: string): Promise<void> {
  try {
    const info = await invoke<BookInfo>("load_epub", { path });
    state.epubPath = path;
    const fromFile = path.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? "";
    state.audioBookTitle = info.title.trim() || fromFile;
    bookInfo = info;
    state.selectedChapters = new Set(info.chapters.map((c) => c.title));
    state.currentPanel = "generate";
    render();
  } catch (e) {
    console.error("[aurawrite] failed to load book:", e);
  }
}

/** Open a job from History / AuraWrite, resuming or restarting it. */
async function openJob(dir: string): Promise<void> {
  if (state.generationRunning) {
    showBusyWarning();
    return;
  }
  let job: HistoryJobView;
  try {
    job = await invoke<HistoryJobView>("history_open", { bookDir: dir });
  } catch (e) {
    console.error("[history] open failed:", e);
    return;
  }
  let restart = false;
  if (!job.dir_exists) {
    const action = await confirmMissingFolder();
    if (!action) return;
    restart = action === "restart";
    try {
      job = await invoke<HistoryJobView>(restart ? "history_restart" : "history_continue", {
        bookDir: dir,
      });
    } catch (e) {
      console.error("[history] recreate folder failed:", e);
      return;
    }
  }
  await loadJob(job, restart);
}

/** Load the job's source document with its saved settings and preselect
 * the chapters that are not converted yet. */
async function loadJob(job: HistoryJobView, restart: boolean): Promise<void> {
  if (!job.source_document) {
    alert("The original document for this job is no longer available, so it cannot be loaded.");
    return;
  }
  try {
    const info = await invoke<BookInfo>("load_epub", { path: job.source_document });
    state.epubPath = job.source_document;
    state.audioBookTitle = info.title.trim() || job.title || "audiobook";
    bookInfo = info;
    applySavedSettings(job);
    if (job.engine_id) await applyEngineDefaults(job.engine_id);
    state.selectedChapters = restart
      ? new Set(info.chapters.map((c) => c.title))
      : remainingChapters(info, job);
    state.currentPanel = "generate";
    render();
  } catch (e) {
    console.error("[history] failed to load job:", e);
  }
}

function remainingChapters(info: BookInfo, job: HistoryJobView): Set<string> {
  const converted = new Set(job.converted_chapters);
  const remaining = info.chapters.filter((c) => !converted.has(c.title)).map((c) => c.title);
  if (remaining.length > 0) return new Set(remaining);
  // Already fully converted: preselect everything so the user can re-generate.
  return new Set(info.chapters.map((c) => c.title));
}

/** Apply the saved engine/voice/language/reference settings of a job. */
function applySavedSettings(job: HistoryJobView): void {
  if (job.engine_id) state.selectedEngineId = job.engine_id;
  if (job.voice) state.selectedVoiceId = job.voice;
  if (job.language) state.selectedLanguage = job.language;
  if (job.reference_audio) state.referenceWavPath = job.reference_audio;
  const p = job.params ?? {};
  if (p["instruct"]) state.qwenInstruct = p["instruct"];
  if (p["ref_text"]) state.referenceTranscript = p["ref_text"];
  if (p["prompt_text"]) state.referenceTranscript = p["prompt_text"];
  if (p["speaker_json"]) state.outeSpeakerJsonPath = p["speaker_json"];
  if (p["voice_mode"] === "design") state.voxMode = "design";
  else if (p["voice_mode"] === "clone") state.voxMode = "clone";
  else if (p["voice_mode"] === "ultimate") state.voxMode = "ultimate";
  if (p["voice_description"]) state.voxVoiceDescription = p["voice_description"];

  const engine = state.selectedEngineId;
  const overrides: Record<string, string> = {};
  const set = (key: string, id: string | undefined): void => {
    if (id && p[key]) overrides[id] = p[key];
  };
  if (engine.startsWith("Qwen3-TTS")) {
    set("temp", "qwen-temp");
    set("top_k", "qwen-top-k");
    set("top_p", "qwen-top-p");
    set("rep_pen", "qwen-rep-pen");
    set("max_new", "qwen-max-new");
    set("seed", "qwen-seed");
  } else if (engine.startsWith("OuteTTS")) {
    set("temperature", "oute-temperature");
    set("top_k", "oute-top-k");
    set("top_p", "oute-top-p");
    set("min_p", "oute-min-p");
    set("repetition_penalty", "oute-rep-pen");
    set("max_tokens", "oute-max-tokens");
  } else if (engine.startsWith("VoxCPM2")) {
    set("cfg", "vox-cfg");
    set("timesteps", "vox-timesteps");
    set("steps", "vox-steps");
    set("seed", "vox-seed");
  }
  for (const [id, val] of Object.entries(overrides)) {
    state.engineParamOverrides[id] = val;
  }
}

async function refreshEngineStatus(): Promise<EngineStatus> {
  console.log("[refreshEngineStatus] calling engine_status...");
  try {
    engineStatus = await invoke<EngineStatus>("engine_status");
    console.log("[refreshEngineStatus] got:", JSON.stringify(engineStatus).slice(0, 200));
  } catch (e) {
    console.error("[refreshEngineStatus] failed:", e);
    engineStatus = {
      active_engine: null,
      active_model: null,
      vram_bytes: null,
      loaded_at: null,
      engines: [],
      hardware: { os: "unknown", arch: "unknown", gpus: [] },
    };
  }
  return engineStatus;
}

async function refreshModelList(): Promise<ModelListEntry[]> {
  try {
    modelList = await loadModels();
  } catch (e) {
    console.error("[refreshModelList] failed:", e);
    modelList = [];
  }
  return modelList;
}

async function refreshAll(): Promise<void> {
  await refreshEngineStatus();
  await refreshModelList();
}

async function main(): Promise<void> {
  console.log("[main] starting Audiobook Generator UI");

  startVramMonitor();

  const needsWizard = await initWizard();
  if (needsWizard) {
    showWizard = true;
    render();
    return;
  }

  await refreshAll();
  // Re-apply engine defaults in case the user just downloaded a model
  // and the engine list changed (e.g. Kokoro became installed).
  const installedEngine = engineStatus.engines.find((e) => e.installed);
  if (installedEngine) {
    state.selectedEngineId = installedEngine.id;
    await applyEngineDefaults(state.selectedEngineId);
  }
  render();
  void refreshAurawrite();
  void refreshHistory();
  void checkProposalFlow();
  await listen("aurawrite:proposal-arrived", () => void checkProposalFlow());
  window.addEventListener("aurawrite:refresh-requested", () => void refreshAurawrite());
  window.addEventListener("aurawrite:open-catalog-book", (e) => {
    const detail = (e as CustomEvent<{ path: string }>).detail;
    void openCatalogBook(detail.path);
  });
  window.addEventListener("history:refresh-requested", () => void refreshHistory());
  window.addEventListener("history:open-job", (e) => {
    const detail = (e as CustomEvent<{ dir: string }>).detail;
    void openJob(detail.dir);
  });
  await listen("engine-status-changed", () => {
    refreshAll().then(async () => {
      const currentInstalled = engineStatus.engines.find(
        (e) => e.id === state.selectedEngineId && e.installed
      );
      if (!currentInstalled) {
        const inst = engineStatus.engines.find((e) => e.installed);
        if (inst) {
          state.selectedEngineId = inst.id;
          await applyEngineDefaults(state.selectedEngineId);
        }
      }
      render();
    });
  });
}

void main();