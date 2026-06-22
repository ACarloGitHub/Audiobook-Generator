import { escapeHtml, bytesToGB, hardwareLine } from "./helpers";
import { renderEngineStrip } from "./engine-strip";
import type { EngineStatus } from "./types";

export function renderModels(status: EngineStatus): string {
  const enginesList = status.engines
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
  const gpu = status.hardware.gpus[0];

  return `
    ${renderEngineStrip(status)}
    <div class="card">
      <h2>📦 TTS Engines Status</h2>
      <ul class="engine-list">${enginesList}</ul>
    </div>

    <div class="card">
      <div class="row">
        <div class="col">
          <label class="field-label">Summary</label>
          <textarea class="text-input" rows="3" readonly>Available engines: ${status.engines.length}
Installed: ${status.engines.length}
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
        ${status.engines
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
      <p class="field-help">${escapeHtml(hardwareLine(status))}</p>
    </div>
  `;
}

export function attachModelsListeners(refresh: () => void): void {
  const updateBtn = document.getElementById("models-update-btn");
  if (updateBtn) {
    updateBtn.addEventListener("click", () => {
      refresh();
    });
  }
}