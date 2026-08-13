import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { escapeHtml, ts } from "./helpers";
import { state } from "./state";
import type { BookErrorSummary, FailedChunkInfo } from "./types";

let recoveryBooks: BookErrorSummary[] = [];
let recoverySelectedBook: BookErrorSummary | null = null;
let recoverySelectedChapter: string | null = null;
let failedChunks: FailedChunkInfo[] = [];
let selectedChunkIndices = new Set<number>();
let manualSelectedIndex: number | null = null;
let operationRunning = false;
let initialScanDone = false;

export function renderRecovery(): string {
  const bookOpts = recoveryBooks
    .map(
      (b) =>
        `<option value="${escapeHtml(b.book_title)}" ${recoverySelectedBook?.book_title === b.book_title ? "selected" : ""}>${escapeHtml(b.book_title)} (${b.chapters_with_errors.length} chapter(s))</option>`,
    )
    .join("");
  const chapterOpts = recoverySelectedBook
    ? recoverySelectedBook.chapters_with_errors
        .map((c) => {
          const tag =
            c.failed_chunks > 0
              ? `${c.failed_chunks}/${c.total_chunks} failed`
              : `${c.total_chunks} chunks · merge pending`;
          return `<option value="${escapeHtml(c.title)}" ${recoverySelectedChapter === c.title ? "selected" : ""}>${escapeHtml(c.title)} (${tag})</option>`;
        })
        .join("")
    : "";

  const chunkRows = failedChunks
    .map((f) => {
      const checked = selectedChunkIndices.has(f.chunk_index) ? "checked" : "";
      const preview = f.text.length > 80 ? f.text.slice(0, 80) + "..." : f.text;
      const errLine = f.error.split("\n")[0];
      return `
        <label class="chapter-row">
          <input type="checkbox" class="failed-chunk-cb" data-index="${f.chunk_index}" ${checked} />
          <span>chunk ${f.chunk_index + 1} — "${escapeHtml(preview)}" <em class="field-help">(${escapeHtml(errLine)})</em></span>
        </label>`;
    })
    .join("");

  const manualOpts = failedChunks
    .map(
      (f) =>
        `<option value="${f.chunk_index}" ${manualSelectedIndex === f.chunk_index ? "selected" : ""}>chunk ${f.chunk_index + 1}</option>`,
    )
    .join("");

  const hasChapter = recoverySelectedBook !== null && recoverySelectedChapter !== null;
  const hasSelection = selectedChunkIndices.size > 0;
  const dis = operationRunning ? "disabled" : "";

  return `
    <div class="card">
      <h2>🔄 Synthesis Errors Recovery System</h2>
      <div class="row">
        <div class="col">
          <label class="field-label">Audiobooks with Errors</label>
          <select class="select" id="recovery-book-select">
            <option value="">— select a book —</option>
            ${bookOpts}
          </select>
        </div>
        <div class="col">
          <label class="field-label">Chapters with Errors</label>
          <select class="select" id="recovery-chapter-select" ${recoverySelectedBook ? "" : "disabled"}>
            <option value="">— select a chapter —</option>
            ${chapterOpts}
          </select>
        </div>
        <div class="col-auto">
          <button class="btn-secondary btn-large" id="recovery-refresh-btn" ${dis}>🔄 Refresh</button>
          <button class="btn-secondary btn-large" id="recovery-browse-btn" ${dis}>📂 Browse…</button>
        </div>
      </div>

      <h3>Failed Chunks</h3>
      <div class="chapter-list" id="failed-chunk-list">
        ${chunkRows || (recoverySelectedChapter ? '<p class="field-help">No failed chunks. Use "Merge All Chunks" to build the chapter MP3.</p>' : '<p class="field-help">Pick a book and chapter to see the failed chunks for that chapter.</p>')}
      </div>

      <div class="btn-row btn-row-large">
        <button class="btn-primary" id="retry-btn" ${hasSelection && !operationRunning ? "" : "disabled"}>🔁 Retry Synthesis</button>
        <button class="btn-secondary" id="merge-btn" ${hasChapter && !operationRunning ? "" : "disabled"}>🔗 Merge All Chunks</button>
        <button class="btn-stop" id="recovery-stop-btn" ${operationRunning ? "" : "disabled"}>🛑 Stop</button>
      </div>

      <details class="accordion">
        <summary>✏️ Manual Editing</summary>
        <div class="row">
          <div class="col">
            <label class="field-label">Failed chunk</label>
            <select class="select" id="manual-chunk-select" ${failedChunks.length ? "" : "disabled"}>
              <option value="">— select a chunk —</option>
              ${manualOpts}
            </select>
          </div>
          <div class="col-auto">
            <label class="field-label">Parts (2–10)</label>
            <input type="number" class="text-input" id="manual-parts" min="2" max="10" value="2" style="width: 5rem" />
          </div>
        </div>
        <label class="field-label">Chunk text (edit before retrying)</label>
        <textarea class="text-input" id="manual-chunk-text" rows="6" placeholder="Select a failed chunk to load its text here."></textarea>
        <div class="btn-row">
          <button class="btn-primary" id="manual-retry-btn" ${manualSelectedIndex !== null && !operationRunning ? "" : "disabled"}>🔁 Retry with edited text</button>
          <button class="btn-secondary" id="manual-split-btn" ${manualSelectedIndex !== null && !operationRunning ? "" : "disabled"}>✂️ Split &amp; retry</button>
        </div>
      </details>

      <h3>Operations Log</h3>
      <textarea class="text-input log-area" id="recovery-log" rows="10" readonly placeholder="No recovery operation running."></textarea>
    </div>
  `;
}

function logLine(msg: string): void {
  const log = document.getElementById("recovery-log") as HTMLTextAreaElement | null;
  if (!log) return;
  log.value += `[${ts()}] ${msg}\n`;
  log.scrollTop = log.scrollHeight;
}

export function attachRecoveryListeners(render: () => void): void {
  const recoveryRefreshBtn = document.getElementById("recovery-refresh-btn");
  if (recoveryRefreshBtn) {
    recoveryRefreshBtn.addEventListener("click", async () => {
      await scanRecoveryBooks();
      render();
    });
  }
  const recoveryBrowseBtn = document.getElementById("recovery-browse-btn");
  if (recoveryBrowseBtn) {
    recoveryBrowseBtn.addEventListener("click", async () => {
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === "string") {
        // Register any books found in this folder so they stay discoverable
        // from the persistent registry on later launches.
        await invoke("scan_recovery_books", { rootDir: selected });
        await scanRecoveryBooks();
        render();
      }
    });
  }
  // Populate the book list once per app session (covers the case where the
  // panel is opened before any Refresh was ever pressed).
  if (!initialScanDone) {
    initialScanDone = true;
    scanRecoveryBooks().then(render);
  }
  const recoveryBookSelect = document.getElementById("recovery-book-select") as HTMLSelectElement | null;
  if (recoveryBookSelect) {
    recoveryBookSelect.addEventListener("change", async () => {
      const title = recoveryBookSelect.value;
      recoverySelectedBook = recoveryBooks.find((b) => b.book_title === title) ?? null;
      recoverySelectedChapter = null;
      failedChunks = [];
      selectedChunkIndices.clear();
      manualSelectedIndex = null;
      render();
    });
  }
  const recoveryChapterSelect = document.getElementById("recovery-chapter-select") as HTMLSelectElement | null;
  if (recoveryChapterSelect) {
    recoveryChapterSelect.addEventListener("change", async () => {
      recoverySelectedChapter = recoveryChapterSelect.value || null;
      selectedChunkIndices.clear();
      manualSelectedIndex = null;
      if (recoverySelectedBook && recoverySelectedChapter) {
        await loadFailedChunks(recoverySelectedBook.book_dir, recoverySelectedChapter);
      }
      render();
    });
  }

  for (const cb of Array.from(document.querySelectorAll<HTMLInputElement>(".failed-chunk-cb"))) {
    cb.addEventListener("change", () => {
      const idx = Number(cb.dataset.index);
      if (cb.checked) selectedChunkIndices.add(idx);
      else selectedChunkIndices.delete(idx);
      const retryBtn = document.getElementById("retry-btn") as HTMLButtonElement | null;
      if (retryBtn) retryBtn.disabled = selectedChunkIndices.size === 0 || operationRunning;
    });
  }

  const retryBtn = document.getElementById("retry-btn") as HTMLButtonElement | null;
  if (retryBtn) {
    retryBtn.addEventListener("click", async () => {
      if (!recoverySelectedBook || !recoverySelectedChapter || selectedChunkIndices.size === 0) return;
      const indices = Array.from(selectedChunkIndices).sort((a, b) => a - b);
      await runOperation(render, `Retrying ${indices.length} chunk(s)...`, () =>
        invoke<string>("retry_failed_chunks", {
          bookDir: recoverySelectedBook!.book_dir,
          chapter: recoverySelectedChapter,
          chunkIndices: indices,
          textsOverride: null,
          engineId: state.selectedEngineId || null,
          referenceAudio: state.referenceWavPath,
        }),
      );
    });
  }

  const mergeBtn = document.getElementById("merge-btn") as HTMLButtonElement | null;
  if (mergeBtn) {
    mergeBtn.addEventListener("click", async () => {
      if (!recoverySelectedBook || !recoverySelectedChapter) return;
      await runOperation(render, "Merging chapter chunks into MP3...", () =>
        invoke<string>("merge_chapter_chunks", {
          bookDir: recoverySelectedBook!.book_dir,
          chapter: recoverySelectedChapter,
        }),
      );
    });
  }

  const stopBtn = document.getElementById("recovery-stop-btn") as HTMLButtonElement | null;
  if (stopBtn) {
    stopBtn.addEventListener("click", async () => {
      try {
        await invoke("stop_generation");
        logLine("[WARN] Stop requested.");
        stopBtn.disabled = true;
      } catch (e) {
        logLine(`[ERROR] stop failed: ${e}`);
      }
    });
  }

  const manualSelect = document.getElementById("manual-chunk-select") as HTMLSelectElement | null;
  if (manualSelect) {
    manualSelect.addEventListener("change", () => {
      const val = manualSelect.value;
      manualSelectedIndex = val === "" ? null : Number(val);
      const textarea = document.getElementById("manual-chunk-text") as HTMLTextAreaElement | null;
      const chunk = failedChunks.find((f) => f.chunk_index === manualSelectedIndex);
      if (textarea) textarea.value = chunk ? chunk.text : "";
      // Update the buttons in place instead of re-rendering the whole panel:
      // a full render() would rebuild the DOM and collapse the open
      // <details> accordion the user just expanded.
      const retryBtn = document.getElementById("manual-retry-btn") as HTMLButtonElement | null;
      const splitBtn = document.getElementById("manual-split-btn") as HTMLButtonElement | null;
      const enabled = manualSelectedIndex !== null && !operationRunning;
      if (retryBtn) retryBtn.disabled = !enabled;
      if (splitBtn) splitBtn.disabled = !enabled;
    });
  }

  const manualRetryBtn = document.getElementById("manual-retry-btn") as HTMLButtonElement | null;
  if (manualRetryBtn) {
    manualRetryBtn.addEventListener("click", async () => {
      if (!recoverySelectedBook || !recoverySelectedChapter || manualSelectedIndex === null) return;
      const textarea = document.getElementById("manual-chunk-text") as HTMLTextAreaElement | null;
      const edited = textarea?.value.trim() ?? "";
      if (!edited) {
        logLine("[ERROR] edited text is empty.");
        return;
      }
      const override: Record<string, string> = { [manualSelectedIndex.toString()]: edited };
      await runOperation(render, `Retrying chunk ${manualSelectedIndex + 1} with edited text...`, () =>
        invoke<string>("retry_failed_chunks", {
          bookDir: recoverySelectedBook!.book_dir,
          chapter: recoverySelectedChapter,
          chunkIndices: [manualSelectedIndex],
          textsOverride: override,
          engineId: state.selectedEngineId || null,
          referenceAudio: state.referenceWavPath,
        }),
      );
    });
  }

  const manualSplitBtn = document.getElementById("manual-split-btn") as HTMLButtonElement | null;
  if (manualSplitBtn) {
    manualSplitBtn.addEventListener("click", async () => {
      if (!recoverySelectedBook || !recoverySelectedChapter || manualSelectedIndex === null) return;
      const partsInput = document.getElementById("manual-parts") as HTMLInputElement | null;
      const nParts = Math.min(10, Math.max(2, Number(partsInput?.value) || 2));
      await runOperation(render, `Splitting chunk ${manualSelectedIndex + 1} into ${nParts} parts...`, () =>
        invoke<string>("split_and_retry_chunk", {
          bookDir: recoverySelectedBook!.book_dir,
          chapter: recoverySelectedChapter,
          chunkIndex: manualSelectedIndex,
          nParts,
          engineId: state.selectedEngineId || null,
          referenceAudio: state.referenceWavPath,
        }),
      );
    });
  }
}

async function runOperation(
  render: () => void,
  startMsg: string,
  op: () => Promise<string>,
): Promise<void> {
  operationRunning = true;
  render();
  logLine(`[INFO] ${startMsg}`);
  const unlisten = await listen<string>("generation-progress", (e) => {
    logLine(e.payload);
  });
  try {
    const result = await op();
    logLine(`[INFO] ${result}`);
  } catch (e) {
    logLine(`[ERROR] ${e}`);
  } finally {
    unlisten();
    operationRunning = false;
    // Refresh the recovery view: chunk states may have changed on disk.
    await scanRecoveryBooks();
    if (recoverySelectedBook) {
      recoverySelectedBook =
        recoveryBooks.find((b) => b.book_title === recoverySelectedBook!.book_title) ?? null;
    }
    if (!recoverySelectedBook) {
      recoverySelectedChapter = null;
      failedChunks = [];
    } else if (recoverySelectedChapter) {
      const stillThere = recoverySelectedBook.chapters_with_errors.some(
        (c) => c.title === recoverySelectedChapter,
      );
      if (stillThere) {
        await loadFailedChunks(recoverySelectedBook.book_dir, recoverySelectedChapter);
      } else {
        recoverySelectedChapter = null;
        failedChunks = [];
      }
    }
    selectedChunkIndices.clear();
    manualSelectedIndex = null;
    render();
  }
}

async function scanRecoveryBooks(): Promise<void> {
  try {
    // Registry-based scan: default output folder + every book recorded in
    // the persistent registry (including user-chosen custom paths).
    recoveryBooks = await invoke<BookErrorSummary[]>("scan_recovery_all");
  } catch (e) {
    console.warn("scan_recovery_all failed:", e);
    recoveryBooks = [];
  }
}

async function loadFailedChunks(bookDir: string, chapter: string): Promise<void> {
  try {
    failedChunks = await invoke<FailedChunkInfo[]>("get_failed_chunks", { bookDir, chapter });
  } catch (e) {
    console.warn("get_failed_chunks failed:", e);
    failedChunks = [];
  }
}
