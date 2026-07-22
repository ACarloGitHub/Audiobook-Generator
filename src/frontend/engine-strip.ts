import { invoke } from "@tauri-apps/api/core";
import { escapeHtml, bytesToGB } from "./helpers";
import type { EngineStatus } from "./types";

interface GpuDevice {
  backend: string;
  name: string;
  total_mib: number;
  free_mib: number;
}

function mibToGB(mib: number): string {
  return (mib / 1024).toFixed(1);
}

function renderVramBar(devs: GpuDevice[]): string {
  if (devs.length === 0) {
    return `<span class="field-help">VRAM: unavailable</span>`;
  }
  return devs
    .map((d) => {
      if (d.total_mib <= 0) {
        return `<span class="field-help">${escapeHtml(d.backend)} ${escapeHtml(d.name)}</span>`;
      }
      const freePct = Math.round((d.free_mib / d.total_mib) * 100);
      const color = freePct > 30 ? "#4caf50" : freePct > 10 ? "#ff9800" : "#f44336";
      return `
        <span style="display:inline-flex;align-items:center;gap:6px;" title="${escapeHtml(d.name)}">
          <span class="field-help">${escapeHtml(d.backend)} VRAM</span>
          <span style="display:inline-block;width:90px;height:8px;background:#333;border-radius:4px;overflow:hidden;">
            <span style="display:block;width:${freePct}%;height:100%;background:${color};"></span>
          </span>
          <span class="field-help">${mibToGB(d.free_mib)}/${mibToGB(d.total_mib)} GB free</span>
        </span>`;
    })
    .join(" ");
}

async function pollVram(): Promise<void> {
  const slot = document.getElementById("vram-bar-slot");
  if (!slot) return;
  try {
    const devs = await invoke<GpuDevice[]>("get_gpu_memory");
    slot.innerHTML = renderVramBar(devs);
  } catch {
    slot.innerHTML = `<span class="field-help">VRAM: unavailable</span>`;
  }
}

// Poll once per app run; the slot is re-created by panel re-renders and
// picked up again on the next tick.
let vramTimerStarted = false;
export function startVramMonitor(): void {
  if (vramTimerStarted) return;
  vramTimerStarted = true;
  void pollVram();
  window.setInterval(() => void pollVram(), 3000);
}

const vramSlot = `<span id="vram-bar-slot" style="display:inline-flex;align-items:center;gap:6px;"></span>`;

export function renderEngineStrip(status: EngineStatus): string {
  if (!status.active_engine) {
    return `
      <div class="engine-strip engine-strip-idle">
        <span class="engine-strip-msg">No engine loaded. Open <strong>Models</strong> to load one.</span>
        ${vramSlot}
      </div>`;
  }
  return `
    <div class="engine-strip">
      <span class="engine-strip-msg">
        Engine loaded: <strong>${escapeHtml(status.active_engine)}</strong>
        ${status.active_model ? `· model <code>${escapeHtml(status.active_model)}</code>` : ""}
        · ~${bytesToGB(status.vram_bytes)} GB VRAM
        ${status.loaded_at ? `· loaded ${escapeHtml(status.loaded_at)}` : ""}
      </span>
      ${vramSlot}
      <button class="btn-secondary" id="release-engine-btn">⏏ Release engine</button>
    </div>`;
}
