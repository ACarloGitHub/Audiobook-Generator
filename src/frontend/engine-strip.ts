import { escapeHtml, bytesToGB } from "./helpers";
import type { EngineStatus } from "./types";

export function renderEngineStrip(status: EngineStatus): string {
  if (!status.active_engine) {
    return `
      <div class="engine-strip engine-strip-idle">
        <span class="engine-strip-msg">No engine loaded. Open <strong>Models</strong> to load one.</span>
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
      <button class="btn-secondary" id="release-engine-btn">⏏ Release engine</button>
    </div>`;
}