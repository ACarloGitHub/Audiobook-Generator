import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { escapeHtml, ts, pickOutputDir, collectParamExtras } from "./helpers";
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
      <textarea class="text-input log-area" id="progress-log" rows="12" readonly placeholder="No generation running.">${escapeHtml(state.progressLog)}</textarea>
    </div>
  `;
}

export function attachGenerateListeners(
  bookInfo: BookInfo | null,
  refreshStatus: () => Promise<EngineStatus>,
): void {
  function pushProgress(text: string, reset = false): void {
    if (reset) state.progressLog = text;
    else state.progressLog += text;
    const el = document.getElementById("progress-log") as HTMLTextAreaElement | null;
    if (el) {
      el.value = state.progressLog;
      el.scrollTop = el.scrollHeight;
    }
  }

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
      for (const cb of Array.from(document.querySelectorAll<HTMLInputElement>(".chapter-cb"))) {
        cb.checked = state.selectedChapters.has(cb.dataset.title!);
      }
      const genBtn = document.getElementById("generate-btn") as HTMLButtonElement | null;
      if (genBtn) genBtn.disabled = state.selectedChapters.size === 0;
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
      for (const cb of Array.from(document.querySelectorAll<HTMLInputElement>(".chapter-cb"))) {
        cb.checked = state.selectedChapters.has(cb.dataset.title!);
      }
      const genBtn = document.getElementById("generate-btn") as HTMLButtonElement | null;
      if (genBtn) genBtn.disabled = state.selectedChapters.size === 0;
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
    if (state.generationRunning) {
      generateBtn.disabled = true;
      stopBtn.disabled = false;
    }
    generateBtn.addEventListener("click", async () => {
      if (!bookInfo || state.selectedChapters.size === 0) return;
      const status = await refreshStatus();
      const engine = status.engines.find((e) => e.id === state.selectedEngineId);
      if (!engine || !engine.installed) {
        pushProgress("[ERROR] No installed engine selected. Pick one in Configuration.\n", true);
        return;
      }
      const safeTitle = bookInfo.title.replace(/[^a-zA-Z0-9-_ ]/g, "_").trim() || "audiobook";
      const outputDir = state.generateOutputPath
        ? `${state.generateOutputPath}/${safeTitle}`
        : pickOutputDir(bookInfo.title);
      generateBtn.disabled = true;
      stopBtn.disabled = false;
      state.generationRunning = true;
      const t0 = Date.now();
      pushProgress(`[INFO] Book: ${bookInfo.title}\n`, true);
      pushProgress(`[INFO] Selected engine: ${engine.display_name}\n`);
      pushProgress(`[INFO] Chapters: ${state.selectedChapters.size}\n`);
      pushProgress(`[INFO] Output: ${outputDir}\n`);
      pushProgress(`[INFO] --- starting generation ---\n`);

      let unlistenProgress: (() => void) | null = null;
      let unlistenComplete: (() => void) | null = null;
      try {
        unlistenProgress = await listen<string>("generation-progress", (e) => {
          pushProgress(`[${ts()}] ${e.payload}\n`);
        });
        unlistenComplete = await listen("generation-complete", () => {
          const secs = ((Date.now() - t0) / 1000).toFixed(1);
          pushProgress(`[${ts()}] [INFO] Generation finished in ${secs}s\n`);
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
          deleteIntermediateChunks: state.deleteIntermediateChunks,
          referenceAudio:
            state.selectedEngineId.startsWith("VoxCPM2") && state.voxMode === "design"
              ? null
              : state.referenceWavPath,
        });
      } catch (e) {
        pushProgress(`[${ts()}] [ERROR] ${e}\n`);
      } finally {
        if (unlistenProgress) unlistenProgress();
        if (unlistenComplete) unlistenComplete();
        state.generationRunning = false;
        generateBtn.disabled = state.selectedChapters.size === 0;
        stopBtn.disabled = true;
        await refreshStatus();
      }
    });

    stopBtn.addEventListener("click", async () => {
      try {
        await invoke("stop_generation");
        pushProgress(`[${ts()}] [WARN] Stop requested.\n`);
        stopBtn.disabled = true;
      } catch (e) {
        pushProgress(`[${ts()}] [ERROR] stop failed: ${e}\n`);
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