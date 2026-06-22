import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { escapeHtml } from "./helpers";
import { state } from "./state";
import type { EngineStatus } from "./types";

export function renderDemo(_status: EngineStatus): string {
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

export function attachDemoListeners(): void {
  const demoOutputBtn = document.getElementById("demo-pick-output-btn");
  if (demoOutputBtn) {
    demoOutputBtn.addEventListener("click", async () => {
      try {
        const path = await open({ multiple: false, directory: true });
        if (typeof path === "string") {
          state.demoOutputPath = path;
          const pathEl = document.getElementById("demo-output-path");
          if (pathEl) pathEl.textContent = path;
        }
      } catch (e) {
        console.warn("dialog open failed:", e);
      }
    });
  }

  const demoGenBtn = document.getElementById("demo-generate-btn") as HTMLButtonElement | null;
  if (demoGenBtn) {
    demoGenBtn.disabled = false;
    demoGenBtn.addEventListener("click", async () => {
      const text = (document.getElementById("demo-text") as HTMLTextAreaElement | null)?.value ?? "";
      if (!text.trim()) {
        alert("Type some text first.");
        return;
      }
      const status = document.getElementById("demo-status") as HTMLTextAreaElement | null;
      const audio = document.getElementById("demo-audio") as HTMLAudioElement | null;
      const outDir = state.demoOutputPath ?? "Generated_Audiobooks/demo";
      const out = `${outDir}/demo_${Date.now()}.wav`;
      if (status) status.value = `[INFO] Synthesizing...\n`;
      try {
        await invoke("synthesize_demo", {
          text,
          voice: state.selectedVoiceId || null,
          outputWav: out,
        });
        if (status) status.value = `[INFO] Saved to ${out}\n`;
        if (audio) {
          audio.src = `file:///${out.replace(/\\/g, "/")}`;
          audio.style.display = "block";
        }
      } catch (e) {
        if (status) status.value = `[ERROR] ${e}\n`;
      }
    });
  }
}