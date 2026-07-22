import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { escapeHtml } from "./helpers";
import { state, SEPARATOR_OPTIONS } from "./state";
import type { BookInfo } from "./types";

export function renderEpub(): string {
  const wordGroupVisible = state.chunkStrategy === "Word Count Approx";
  const charGroupVisible = state.chunkStrategy === "Character Limit";
  const sepOptions = SEPARATOR_OPTIONS.map(
    (o) =>
      `<option value="${escapeHtml(o.value)}" ${o.value === state.selectedSeparator ? "selected" : ""}>${escapeHtml(o.label)}</option>`,
  ).join("");

  return `
    <div class="card">
      <h2>Upload EPUB</h2>
      <button class="btn-secondary btn-large" id="pick-epub-btn">Drop File Here / Click to Upload</button>
      <p class="field-help" id="epub-path">${state.epubPath ? escapeHtml(state.epubPath) : "No document selected (EPUB, TXT, Markdown, DOCX, JSON)."}</p>
    </div>

    <div class="card">
      <h2>Audiobook Title</h2>
      <textarea class="text-input" rows="1" id="audiobook-title" placeholder="Enter title or leave blank">${escapeHtml(state.audioBookTitle)}</textarea>
    </div>

    <div class="card">
      <h2>Text cleanup</h2>
      <label class="checkbox-row">
        <input type="checkbox" id="replace-guillemets" ${state.replaceGuillemets ? "checked" : ""} />
        <span>Replace Guillemets (« »)</span>
      </label>
      <label class="field-label">Sentence Separator</label>
      <select class="select" id="separator-select">${sepOptions}</select>
    </div>

    <div class="card">
      <h2>Chunking strategy</h2>
      <div class="radio-row">
        <label class="radio-pill">
          <input type="radio" name="chunk-strategy" value="Word Count Approx" ${state.chunkStrategy === "Word Count Approx" ? "checked" : ""} />
          <span>Word Count Approx</span>
        </label>
        <label class="radio-pill">
          <input type="radio" name="chunk-strategy" value="Character Limit" ${state.chunkStrategy === "Character Limit" ? "checked" : ""} />
          <span>Character Limit</span>
        </label>
      </div>

      <div class="group ${wordGroupVisible ? "" : "hidden"}" id="word-count-group">
        <label class="field-label">Min Words</label>
        <input type="number" class="num-input" id="min-words" min="10" step="10" value="${state.chunkMinWords}" />
        <label class="field-label">Max Words</label>
        <input type="number" class="num-input" id="max-words" min="50" step="10" value="${state.chunkMaxWords}" />
      </div>

      <div class="group ${charGroupVisible ? "" : "hidden"}" id="char-limit-group">
        <label class="field-label">Max Chars</label>
        <input type="number" class="num-input" id="max-chars" min="100" step="50" value="${state.chunkMaxChars}" />
        <p class="field-help">Max value auto-loaded from the selected engine on Configuration. Override manually if needed.</p>
      </div>
    </div>
  `;
}

export function attachEpubListeners(render: () => void, onEpubLoaded: (info: BookInfo) => void): void {
  const pickEpubBtn = document.getElementById("pick-epub-btn");
  if (pickEpubBtn) {
    pickEpubBtn.addEventListener("click", async () => {
      try {
        const path = await open({ multiple: false, filters: [{ name: "Book or text document", extensions: ["epub", "txt", "md", "docx", "json"] }] });
        if (typeof path === "string") {
          state.epubPath = path;
          const info = await invoke<BookInfo>("load_epub", { path });
          onEpubLoaded(info);
        }
      } catch (e) {
        console.warn("dialog open failed:", e);
      }
    });
  }

  const audiobookTitle = document.getElementById("audiobook-title") as HTMLTextAreaElement | null;
  if (audiobookTitle) {
    audiobookTitle.addEventListener("input", () => {
      state.audioBookTitle = audiobookTitle.value;
    });
  }

  const sepSelect = document.getElementById("separator-select") as HTMLSelectElement | null;
  if (sepSelect) {
    sepSelect.addEventListener("change", () => {
      state.selectedSeparator = sepSelect.value;
    });
  }

  const guillemetCb = document.getElementById("replace-guillemets") as HTMLInputElement | null;
  if (guillemetCb) {
    guillemetCb.addEventListener("change", () => {
      state.replaceGuillemets = guillemetCb.checked;
    });
  }

  for (const r of Array.from(document.querySelectorAll<HTMLInputElement>("input[name='chunk-strategy']"))) {
    r.addEventListener("change", () => {
      if (r.checked) {
        state.chunkStrategy = r.value as "Word Count Approx" | "Character Limit";
        render();
      }
    });
  }

  const minW = document.getElementById("min-words") as HTMLInputElement | null;
  if (minW) minW.addEventListener("input", () => (state.chunkMinWords = Number(minW.value)));
  const maxW = document.getElementById("max-words") as HTMLInputElement | null;
  if (maxW) maxW.addEventListener("input", () => (state.chunkMaxWords = Number(maxW.value)));
  const maxC = document.getElementById("max-chars") as HTMLInputElement | null;
  if (maxC) maxC.addEventListener("input", () => (state.chunkMaxChars = Number(maxC.value)));
}