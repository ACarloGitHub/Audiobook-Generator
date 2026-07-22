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

function panelBody(): string {
  switch (state.currentPanel) {
    case "configuration": return renderConfiguration(engineStatus);
    case "epub": return renderEpub();
    case "generate": return renderGenerate(engineStatus, bookInfo);
    case "recovery": return renderRecovery();
    case "demo": return renderDemo(engineStatus);
    case "models": return renderModels(engineStatus, modelList);
    case "agents": return renderAgents();
  }
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

function restoreFormValues(values: Map<string, string | boolean>): void {
  for (const [id, value] of values) {
    // Advanced engine params are restored from state.engineParamOverrides
    // by the Configuration panel itself; the snapshot may hold a stale
    // registry default and must not overwrite the user's saved value.
    if (state.engineParamOverrides[id] !== undefined) continue;
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
        <p class="sidebar-version">v0.1.0</p>
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
    if (!state.audioBookTitle) {
      state.audioBookTitle = info.title;
    }
    state.selectedChapters = new Set(info.chapters.map((c) => c.title));
    render();
  });
  attachGenerateListeners(render, bookInfo, refreshEngineStatus);
  attachRecoveryListeners(render);
  attachDemoListeners();
  attachModelsListeners(async () => {
    await refreshAll();
    render();
  });
  attachAgentsListeners();
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