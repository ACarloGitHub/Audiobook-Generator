import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { escapeHtml, ts, pickOutputDir, appendLog, setLog, collectParamExtras } from "./helpers";
import { renderEngineStrip } from "./engine-strip";
import { state } from "./state";
import type { BookInfo, EngineStatus } from "./types";

export function renderGenerate(status: EngineStatus, bookInfo: BookInfo | null): string {
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
    ${renderEngineStrip(status)}
    <div class="card">
      <div class="btn-row">
        <button class="btn-secondary" id="select-all-btn" ${bookInfo ? "" : "disabled"}>Select All</button>
        <button class="btn-secondary" id="invert-btn" ${bookInfo ? "" : "disabled"}>Invert</button>
      </div>
      <p class="field-help" id="chapter-status">${chapterStatus}</p>
      <div class="chapter-list" id="chapter-list">${chapterRows}</div>
    </div>

    <div class="card">
      <div class="btn-row">
        <button class="btn-secondary" id="gen-pick-output-btn">Choose output path...</button>
        <span class="field-help" id="gen-output-path">${state.generateOutputPath ? escapeHtml(state.generateOutputPath) : "Default: app data folder / Generated_Audiobooks"}</span>
      </div>
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

export function attachGenerateListeners(
  render: () => void,
  bookInfo: BookInfo | null,
  refreshStatus: () => Promise<EngineStatus>,
): void {
  const genPickBtn = document.getElementById("gen-pick-output-btn");
  if (genPickBtn) {
    genPickBtn.addEventListener("click", async () => {
      try {
        const path = await open({ multiple: false, directory: true });
        if (typeof path === "string") {
          state.generateOutputPath = path;
          const pathEl = document.getElementById("gen-output-path");
          if (pathEl) pathEl.textContent = path;
        }
      } catch (e) {
        console.warn("dialog open failed:", e);
      }
    });
  }

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

  const generateBtn = document.getElementById("generate-btn") as HTMLButtonElement | null;
  const stopBtn = document.getElementById("stop-btn") as HTMLButtonElement | null;
  const progressLog = document.getElementById("progress-log") as HTMLTextAreaElement | null;

  if (generateBtn && stopBtn && progressLog && bookInfo) {
    generateBtn.addEventListener("click", async () => {
      if (!bookInfo || state.selectedChapters.size === 0) return;
      const status = await refreshStatus();
      const engine = status.engines.find((e) => e.id === state.selectedEngineId);
      if (!engine || !engine.installed) {
        progressLog.value = `[ERROR] No installed engine selected. Pick one in Configuration.\n`;
        return;
      }
      const safeTitle = bookInfo.title.replace(/[^a-zA-Z0-9-_ ]/g, "_").trim() || "audiobook";
      const outputDir = state.generateOutputPath
        ? `${state.generateOutputPath}/${safeTitle}`
        : pickOutputDir(bookInfo.title);
      generateBtn.disabled = true;
      stopBtn.disabled = false;
      const t0 = Date.now();
      setLog("progress-log", `[INFO] Book: ${bookInfo.title}\n`);
      appendLog("progress-log", `[INFO] Selected engine: ${engine.display_name}\n`);
      appendLog("progress-log", `[INFO] Chapters: ${state.selectedChapters.size}\n`);
      appendLog("progress-log", `[INFO] Output: ${outputDir}\n`);
      appendLog("progress-log", `[INFO] --- starting generation ---\n`);

      let unlistenProgress: (() => void) | null = null;
      let unlistenComplete: (() => void) | null = null;
      try {
        unlistenProgress = await listen<string>("generation-progress", (e) => {
          appendLog("progress-log", `[${ts()}] ${e.payload}\n`);
        });
        unlistenComplete = await listen("generation-complete", () => {
          const secs = ((Date.now() - t0) / 1000).toFixed(1);
          appendLog("progress-log", `[${ts()}] [INFO] Generation finished in ${secs}s\n`);
        });
        const maxCharsForLang =
          state.chunkMaxCharsByLang[state.selectedLanguage] ?? state.chunkMaxChars;

        // Engine params: shared resolver (Configuration values included).
        const extra = collectParamExtras();
        // When using Character Limit strategy, disable word count limit
        const effectiveMaxWords =
          state.chunkStrategy === "Character Limit" ? 999999 : state.chunkMaxWords;

        await invoke("start_generation", {
          engineId: state.selectedEngineId,
          voice: state.selectedVoiceId || null,
          language: state.selectedLanguage || null,
          speed: state.speed,
          epubPath: state.epubPath,
          outputDir,
          maxWords: effectiveMaxWords,
          maxChars: maxCharsForLang,
          extra,
          onlyChapters: Array.from(state.selectedChapters),
          referenceAudio:
            state.selectedEngineId.startsWith("VoxCPM2") && state.voxMode === "design"
              ? null
              : state.referenceWavPath,
        });
      } catch (e) {
        appendLog("progress-log", `[${ts()}] [ERROR] ${e}\n`);
      } finally {
        if (unlistenProgress) unlistenProgress();
        if (unlistenComplete) unlistenComplete();
        generateBtn.disabled = state.selectedChapters.size === 0;
        stopBtn.disabled = true;
        await refreshStatus();
      }
    });

    stopBtn.addEventListener("click", async () => {
      try {
        await invoke("stop_generation");
        if (progressLog) progressLog.value += `[${ts()}] [WARN] Stop requested.\n`;
        stopBtn.disabled = true;
      } catch (e) {
        if (progressLog) progressLog.value += `[${ts()}] [ERROR] stop failed: ${e}\n`;
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