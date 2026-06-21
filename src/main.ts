import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";

type PanelId =
  | "models"
  | "generate"
  | "configuration"
  | "epub"
  | "demo";

interface EngineStatus {
  active_engine: string | null;
  active_model: string | null;
  vram_bytes: number | null;
  loaded_at: string | null;
}

let currentPanel: PanelId = "generate";
let engineStatus: EngineStatus = {
  active_engine: null,
  active_model: null,
  vram_bytes: null,
  loaded_at: null,
};

const PANEL_TITLES: Record<PanelId, string> = {
  models: "Models",
  generate: "Generate",
  configuration: "Configuration",
  epub: "EPUB Options",
  demo: "Demo / Test",
};

const HARDWARE_SUMMARY = "Windows · NVIDIA RTX 3090 · 24 GB VRAM · 159 GB free";

function $(sel: string): HTMLElement {
  const el = document.querySelector(sel);
  if (!el) throw new Error(`Missing element: ${sel}`);
  return el as HTMLElement;
}

function bytesToGB(n: number): string {
  return (n / 1024 / 1024 / 1024).toFixed(2);
}

function renderSidebar(): string {
  const items: { id: PanelId; icon: string; label: string }[] = [
    { id: "models", icon: "M", label: "Models" },
    { id: "generate", icon: "G", label: "Generate" },
    { id: "configuration", icon: "C", label: "Configuration" },
    { id: "epub", icon: "E", label: "EPUB Options" },
    { id: "demo", icon: "D", label: "Demo / Test" },
  ];
  return items
    .map(
      (it) =>
        `<li class="nav-item ${it.id === currentPanel ? "active" : ""}" data-panel="${it.id}">` +
        `<span class="nav-icon">${it.icon}</span>` +
        `<span class="nav-label">${it.label}</span></li>`,
    )
    .join("");
}

function renderEngineStrip(): string {
  if (!engineStatus.active_engine) {
    return `
      <div class="engine-strip engine-strip-idle">
        <span>No engine loaded. Pick one in <strong>Models</strong> to start.</span>
      </div>`;
  }
  const vram = engineStatus.vram_bytes ? bytesToGB(engineStatus.vram_bytes) : "?";
  return `
    <div class="engine-strip">
      <span>Engine loaded: <strong>${engineStatus.active_engine}</strong> · ~${vram} GB VRAM · loaded ${engineStatus.loaded_at ?? "?"}</span>
      <button class="btn-secondary" id="release-engine-btn">⏏ Release engine</button>
    </div>`;
}

function renderMainPanel(): string {
  const title = PANEL_TITLES[currentPanel];
  const body = panelBody(currentPanel);
  return `<section class="panel"><h1 class="panel-title">${title}</h1>${body}</section>`;
}

function panelBody(id: PanelId): string {
  switch (id) {
    case "generate":
      return `
        ${renderEngineStrip()}
        <div class="card">
          <h2>Drop EPUB here</h2>
          <p>Drag and drop a <code>.epub</code> file, or click to pick one.</p>
        </div>
        <div class="card">
          <h2>Engine</h2>
          <p>Current engine: <strong>${engineStatus.active_engine ?? "none"}</strong></p>
          <p>Model: <strong>${engineStatus.active_model ?? "—"}</strong></p>
        </div>
        <div class="card">
          <h2>Output</h2>
          <p>Output directory and naming pattern (placeholder for Phase 1 of the migration).</p>
        </div>`;
    case "models":
      return `
        <div class="card">
          <h2>TTS Engines</h2>
          <ul class="engine-list">
            <li><strong>Kokoro (ONNX, Apache 2.0)</strong> · CPU/GPU · 92 MB download · not downloaded</li>
            <li><strong>Qwen3-TTS (GGUF, Apache 2.0)</strong> · CPU/GPU/Vulkan · ~1-2 GB · not downloaded</li>
            <li><strong>OuteTTS 1.0 (GGUF, CC-BY-NC-SA-4.0)</strong> · CPU/GPU · ~1.2 GB · not downloaded</li>
            <li><strong>NeuTTS Air (GGUF, Apache 2.0)</strong> · CPU · ~500 MB · not downloaded</li>
          </ul>
          <p><em>Placeholder. The First-Run Wizard will appear on first launch and handle downloads.</em></p>
        </div>
        <div class="card">
          <h2>Runtime binaries</h2>
          <p>llama-server: not installed</p>
          <p>ffmpeg: bundled</p>
          <p>ONNX Runtime: not installed (needed for Kokoro)</p>
        </div>
        <div class="card">
          <h2>Hardware</h2>
          <p>${HARDWARE_SUMMARY}</p>
        </div>`;
    case "configuration":
      return `
        <div class="card">
          <h2>Engine parameters</h2>
          <p>Chunk size, max characters, sentence separator, replace guillemets (placeholder).</p>
        </div>
        <div class="card">
          <h2>Output defaults</h2>
          <p>Output dir, naming pattern, default bitrate (placeholder).</p>
        </div>
        <div class="card">
          <h2>Hardware overrides</h2>
          <p>Force CPU even if GPU is available, force Vulkan on Win/Linux when CUDA is broken (placeholder).</p>
        </div>`;
    case "epub":
      return `
        <div class="card">
          <h2>Extraction mode</h2>
          <p><strong>ToC</strong> (default, uses the publisher's Table of Contents) or <strong>Spine</strong> (every spine item is a chapter).</p>
        </div>
        <div class="card">
          <h2>Skip empty chapters</h2>
          <p>Chapters with fewer than 50 characters of text are dropped. <em>On</em> by default.</p>
        </div>
        <div class="card">
          <h2>Title overrides</h2>
          <p>Optional CSV: <code>chapter,new_title</code> (placeholder).</p>
        </div>`;
    case "demo":
      return `
        <div class="card">
          <h2>Quick test</h2>
          <p>Type a sentence, pick an engine, hit <em>Speak</em>. Same pipeline as Generate, on a single short text (placeholder).</p>
          <textarea class="demo-input" rows="4" placeholder="Type a sentence to synthesize..."></textarea>
          <button class="btn-primary" disabled>Speak</button>
        </div>`;
  }
}

function render(): void {
  const app = $("#app");
  app.innerHTML = `
    <aside class="sidebar">
      <div class="sidebar-header">
        <h1 class="sidebar-title">Audiobook Generator</h1>
        <p class="sidebar-version">v0.1.0</p>
      </div>
      <nav>
        <ul class="nav-list">${renderSidebar()}</ul>
      </nav>
      <div class="sidebar-footer">
        <p class="sidebar-footer-label">${HARDWARE_SUMMARY}</p>
      </div>
    </aside>
    <main class="main">${renderMainPanel()}</main>
  `;

  for (const li of Array.from(document.querySelectorAll<HTMLElement>(".nav-item"))) {
    li.addEventListener("click", () => {
      const id = li.dataset.panel as PanelId;
      currentPanel = id;
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
}

async function refreshEngineStatus(): Promise<void> {
  try {
    engineStatus = (await invoke("engine_status")) as EngineStatus;
  } catch {
    // Native core not reachable during dev with no engine loaded yet.
    engineStatus = {
      active_engine: null,
      active_model: null,
      vram_bytes: null,
      loaded_at: null,
    };
  }
}

async function main(): Promise<void> {
  await refreshEngineStatus();
  render();
  // Future: subscribe to engine-status events for live updates.
  await listen("engine-status-changed", () => {
    refreshEngineStatus().then(render);
  });
}

void main();
