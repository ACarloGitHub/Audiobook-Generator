import { invoke } from "@tauri-apps/api/core";
import { escapeHtml } from "./helpers";
import type { BookErrorSummary, FailedChunkInfo } from "./types";

let recoveryBooks: BookErrorSummary[] = [];
let recoverySelectedBook: BookErrorSummary | null = null;
let recoverySelectedChapter: string | null = null;

export function renderRecovery(): string {
  const bookOpts = recoveryBooks
    .map(
      (b) =>
        `<option value="${escapeHtml(b.book_title)}" ${recoverySelectedBook?.book_title === b.book_title ? "selected" : ""}>${escapeHtml(b.book_title)} (${b.chapters_with_errors.length} chapter(s))</option>`,
    )
    .join("");
  const chapterOpts = recoverySelectedBook
    ? recoverySelectedBook.chapters_with_errors
        .map(
          (c) =>
            `<option value="${escapeHtml(c.title)}" ${recoverySelectedChapter === c.title ? "selected" : ""}>${escapeHtml(c.title)} (${c.failed_chunks}/${c.total_chunks} failed)</option>`,
        )
        .join("")
    : "";

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
          <button class="btn-secondary btn-large" id="recovery-refresh-btn">🔄 Refresh</button>
        </div>
      </div>

      <h3>Failed Chunks</h3>
      <textarea class="text-input log-area" id="failed-chunks" rows="10" readonly placeholder="Pick a book and chapter to see the failed chunks for that chapter."></textarea>

      <div class="btn-row btn-row-large">
        <button class="btn-primary" id="retry-btn" disabled>🔁 Retry Synthesis</button>
        <button class="btn-secondary" id="merge-btn" disabled>🔗 Merge All Chunks</button>
        <button class="btn-stop" id="recovery-stop-btn" disabled>🛑 Stop</button>
      </div>

      <details class="accordion">
        <summary>✏️ Manual Editing</summary>
        <p class="field-help">Per-chunk manual override and split (TBD).</p>
      </details>
    </div>
  `;
}

export function attachRecoveryListeners(render: () => void): void {
  const recoveryRefreshBtn = document.getElementById("recovery-refresh-btn");
  if (recoveryRefreshBtn) {
    recoveryRefreshBtn.addEventListener("click", async () => {
      await scanRecoveryBooks();
      render();
    });
  }
  const recoveryBookSelect = document.getElementById("recovery-book-select") as HTMLSelectElement | null;
  if (recoveryBookSelect) {
    recoveryBookSelect.addEventListener("change", async () => {
      const title = recoveryBookSelect.value;
      recoverySelectedBook = recoveryBooks.find((b) => b.book_title === title) ?? null;
      recoverySelectedChapter = null;
      const failedTextarea = document.getElementById("failed-chunks") as HTMLTextAreaElement | null;
      if (failedTextarea) failedTextarea.value = "";
      render();
    });
  }
  const recoveryChapterSelect = document.getElementById("recovery-chapter-select") as HTMLSelectElement | null;
  if (recoveryChapterSelect) {
    recoveryChapterSelect.addEventListener("change", async () => {
      recoverySelectedChapter = recoveryChapterSelect.value || null;
      if (recoverySelectedBook && recoverySelectedChapter) {
        await loadFailedChunks(recoverySelectedBook.book_dir, recoverySelectedChapter);
        render();
      }
    });
  }
}

async function scanRecoveryBooks(): Promise<void> {
  try {
    recoveryBooks = await invoke<BookErrorSummary[]>("scan_recovery_books", {
      rootDir: "Generated_Audiobooks",
    });
  } catch (e) {
    console.warn("scan_recovery_books failed:", e);
    recoveryBooks = [];
  }
}

async function loadFailedChunks(bookDir: string, chapter: string): Promise<void> {
  try {
    const failed = await invoke<FailedChunkInfo[]>("get_failed_chunks", { bookDir, chapter });
    const textarea = document.getElementById("failed-chunks") as HTMLTextAreaElement | null;
    if (!textarea) return;
    textarea.value = failed
      .map(
        (f) =>
          `chunk ${f.chunk_index + 1}: "${f.text.length > 200 ? f.text.slice(0, 200) + "..." : f.text}"\n  -> ${f.error}`,
      )
      .join("\n\n");
    const retryBtn = document.getElementById("retry-btn") as HTMLButtonElement | null;
    const mergeBtn = document.getElementById("merge-btn") as HTMLButtonElement | null;
    if (retryBtn) retryBtn.disabled = failed.length === 0;
    if (mergeBtn) mergeBtn.disabled = failed.length === 0;
  } catch (e) {
    console.warn("get_failed_chunks failed:", e);
  }
}