import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { escapeHtml, hardwareLine } from "./helpers";
import { renderEngineStrip } from "./engine-strip";
import type { EngineStatus, ModelListEntry } from "./types";

export async function loadModels(): Promise<ModelListEntry[]> {
  return await invoke<ModelListEntry[]>("list_models");
}

export function renderModels(status: EngineStatus, models: ModelListEntry[]): string {
  const installedCount = models.filter((m) => m.installed).length;
  const missingCount = models.length - installedCount;

  const engineRows = models
    .map((m) => {
      const statusBadge = m.installed
        ? `<span class="status-dot status-installed"></span> installed`
        : m.supported
        ? `<span class="status-dot status-missing"></span> not installed`
        : `<span class="status-dot status-planned"></span> planned`;
      const actionBtn = m.installed
        ? `<button class="btn-secondary btn-small" data-action="remove" data-model="${escapeHtml(m.name)}">Remove</button>`
        : m.supported
        ? `<button class="btn-primary btn-small" data-action="download" data-model="${escapeHtml(m.name)}">Download</button>`
        : `<span class="field-help">not yet implemented</span>`;
      const noteHtml = m.note ? `<br/><span class="field-help">${escapeHtml(m.note)}</span>` : "";
      return `
        <tr>
          <td><strong>${escapeHtml(m.name)}</strong><br/><span class="field-help">${escapeHtml(m.format)} · ${escapeHtml(m.license)}</span>${noteHtml}</td>
          <td>${m.size_mb} MB</td>
          <td>${statusBadge}</td>
          <td>${actionBtn}</td>
        </tr>`;
    })
    .join("");

  return `
    ${renderEngineStrip(status)}

    <div class="card">
      <div class="row">
        <div class="col">
          <label class="field-label">Summary</label>
          <textarea class="text-input" rows="3" readonly>Total models: ${models.length}
Installed: ${installedCount}
Not installed: ${missingCount}</textarea>
        </div>
        <div class="col-auto">
          <button class="btn-secondary btn-large" id="models-update-btn">🔄 Update Status</button>
        </div>
      </div>
    </div>

    <div class="card">
      <h2>Models</h2>
      <p class="field-help" id="models-path-display">Loading models path...</p>
      <div class="row" style="align-items: center; gap: 8px; margin-bottom: 8px;">
        <button class="btn-secondary" id="storage-change-btn">📁 Change storage folder…</button>
        <button class="btn-secondary" id="storage-reset-btn" hidden>Reset to default</button>
        <label class="field-help" style="display: inline-flex; align-items: center; gap: 4px;">
          <input type="checkbox" id="storage-move-check" checked /> Move existing files
        </label>
      </div>
      <p class="field-help" id="storage-status"></p>
      <table class="models-table">
        <thead>
          <tr>
            <th>Model</th>
            <th>Size</th>
            <th>Status</th>
            <th>Action</th>
          </tr>
        </thead>
        <tbody>${engineRows}</tbody>
      </table>
      <label class="field-label">Download Log</label>
      <textarea class="text-input log-area" id="models-download-log" rows="8" readonly placeholder="Download details will appear here..."></textarea>
    </div>

    <div class="card">
      <h2>Runtime Binaries</h2>
      <p class="field-help" id="models-runtime-status">
        Checking...
      </p>
      <div class="btn-row">
        <button class="btn-secondary" id="models-runtime-refresh">🔄 Refresh</button>
      </div>
      <details class="accordion">
        <summary>Manual Instructions</summary>
        <p class="field-help">
          FFmpeg and llama-server are installed by the first-run wizard.
          Re-run the wizard to install missing components.
        </p>
      </details>
    </div>

    <div class="card">
      <h2>Hardware</h2>
      <p class="field-help">${escapeHtml(hardwareLine(status))}</p>
    </div>
  `;
}

function appendLog(line: string): void {
  const log = document.getElementById("models-download-log") as HTMLTextAreaElement | null;
  if (!log) return;
  if (log.value.length > 0) log.value += "\n";
  log.value += line;
  log.scrollTop = log.scrollHeight;
}

async function refreshRuntimeStatus(): Promise<void> {
  const el = document.getElementById("models-runtime-status");
  if (!el) return;
  try {
    const deps = await invoke<{ ffmpeg_installed: boolean; llama_server_installed: boolean }>("check_dependencies");
    el.innerHTML = `
      FFmpeg: ${deps.ffmpeg_installed ? "✅ installed" : "❌ missing (run wizard)"}<br/>
      llama-server: ${deps.llama_server_installed ? "✅ installed" : "❌ missing (run wizard)"}
    `;
  } catch (e) {
    el.textContent = `Error checking runtime: ${e}`;
  }
}

export function attachModelsListeners(refresh: () => Promise<void>): void {
  const updateBtn = document.getElementById("models-update-btn");
  if (updateBtn) {
    updateBtn.addEventListener("click", async () => {
      await refresh();
    });
  }

  const runtimeRefresh = document.getElementById("models-runtime-refresh");
  if (runtimeRefresh) {
    runtimeRefresh.addEventListener("click", () => {
      refreshRuntimeStatus();
    });
  }

  // Per-model download/remove buttons
  document.querySelectorAll("button[data-action][data-model]").forEach((btn) => {
    btn.addEventListener("click", async (ev) => {
      const target = ev.currentTarget as HTMLButtonElement;
      const action = target.dataset.action;
      const model = target.dataset.model;
      if (!action || !model) return;
      target.disabled = true;
      const original = target.textContent;
      target.textContent = action === "download" ? "Downloading..." : "Removing...";
      try {
        if (action === "download") {
          appendLog(`[${new Date().toLocaleTimeString()}] Downloading ${model}...`);
          await invoke("download_model", { name: model });
          appendLog(`[${new Date().toLocaleTimeString()}] ${model} installed successfully.`);
        } else if (action === "remove") {
          appendLog(`[${new Date().toLocaleTimeString()}] Removing ${model}...`);
          await invoke("remove_model", { name: model });
          appendLog(`[${new Date().toLocaleTimeString()}] ${model} removed.`);
        }
        await refresh();
      } catch (e) {
        appendLog(`[${new Date().toLocaleTimeString()}] ERROR: ${e}`);
        target.disabled = false;
        target.textContent = original;
      }
    });
  });

  // Per-file download progress from backend
  listen<{ model: string; file: string; phase: string; bytes: number; total: number; speed_bps: number; eta_seconds: number | null }>(
    "model-progress",
    (event) => {
      const p = event.payload;
      if (p.phase === "already_present") {
        appendLog(`  ${p.file}: already on disk`);
      } else if (p.phase === "downloading" || p.phase === "resuming") {
        const pct = p.total > 0 ? `(${(p.bytes / p.total * 100).toFixed(1)}%)` : "";
        appendLog(`  ${p.file}: ${p.bytes} bytes ${pct}`);
      } else if (p.phase === "done") {
        appendLog(`  ${p.file}: done (${p.bytes} bytes total)`);
      } else if (p.phase === "error") {
        appendLog(`  ERROR: ${p.file}`);
      }
    }
  );

  // Also listen for the wizard's download-progress events so the same log
  // captures any wizard activity triggered while the user is on the Models tab.
  listen<{ id: string; name: string; phase: string; bytes: number; total: number; speed_bps: number; eta_seconds: number | null }>(
    "download-progress",
    (event) => {
      const p = event.payload;
      if (p.phase === "downloading" || p.phase === "resuming") {
        const pct = p.total > 0 ? `(${(p.bytes / p.total * 100).toFixed(1)}%)` : "";
        appendLog(`  [wizard] ${p.name}: ${p.bytes} bytes ${pct}`);
      } else if (p.phase === "extracting") {
        appendLog(`  [wizard] Extracting ${p.name}...`);
      } else if (p.phase === "done") {
        appendLog(`  [wizard] ${p.name} installed.`);
      }
    }
  );

  // Show models directory path + storage folder controls
  const refreshStorageDisplay = async () => {
    try {
      const info = await invoke<{ current: string; default: string; is_custom: boolean }>("get_storage_dir");
      const el = document.getElementById("models-path-display");
      if (el) el.textContent = `Storage folder (models + engines): ${info.current}`;
      const resetBtn = document.getElementById("storage-reset-btn") as HTMLButtonElement | null;
      if (resetBtn) resetBtn.hidden = !info.is_custom;
    } catch {
      const el = document.getElementById("models-path-display");
      if (el) el.textContent = "Storage folder: unknown";
    }
  };

  const setStorageStatus = (msg: string) => {
    const el = document.getElementById("storage-status");
    if (el) el.textContent = msg;
  };

  const applyStorageChange = async (path: string | null) => {
    const move = (document.getElementById("storage-move-check") as HTMLInputElement | null)?.checked ?? true;
    setStorageStatus("Applying… (moving large files can take a while)");
    try {
      const newPath = await invoke<string>("set_storage_dir", { path, moveExisting: move });
      setStorageStatus(`Storage folder is now: ${newPath}`);
      await refreshStorageDisplay();
    } catch (e) {
      setStorageStatus(`Failed to change storage folder: ${e}`);
    }
  };

  document.getElementById("storage-change-btn")?.addEventListener("click", async () => {
    const chosen = await openDialog({ directory: true, title: "Choose the storage folder for models and engines" });
    if (typeof chosen === "string" && chosen.length > 0) {
      await applyStorageChange(chosen);
    }
  });

  document.getElementById("storage-reset-btn")?.addEventListener("click", async () => {
    await applyStorageChange(null);
  });

  void refreshStorageDisplay();

  // Initial runtime check
  refreshRuntimeStatus();
}