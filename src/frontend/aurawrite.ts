// AuraWrite panel (twin integration).
//
// The "AuraWrite" sidebar panel: shows an informative message when AuraWrite
// is not reachable, otherwise lists the ebooks from AuraWrite's unified
// catalog. Also handles the handover flow: reads the proposal AuraWrite
// wrote, asks the user to accept it, and reports busy state.

import { invoke } from "@tauri-apps/api/core";
import { escapeHtml } from "./helpers";
import { renderJobRows, statusLabel, chapterSummary, formatDate } from "./history";
import type { HistoryJobView } from "./types";

export interface AurawriteProposal {
  input: string;
}

export interface CatalogBook {
  id: string;
  name: string;
  section: string;
  path: string;
}

export interface AurawriteState {
  found: boolean;
  books: CatalogBook[];
}

const INFO_KEY = "abg-aurawrite-info-seen";

/** Fetch whether AuraWrite is reachable and its unified ebook catalog. */
export async function checkAurawriteState(): Promise<AurawriteState> {
  let found = false;
  try {
    const check = await invoke<{ found_aurawrite: boolean; has_proposal: boolean }>(
      "aurawrite_check"
    );
    found = check.found_aurawrite;
  } catch (e) {
    console.error("[aurawrite] check failed:", e);
  }
  let books: CatalogBook[] = [];
  try {
    const catalog = await invoke<{ books: CatalogBook[] } | null>("aurawrite_catalog");
    books = catalog?.books ?? [];
  } catch (e) {
    console.error("[aurawrite] catalog failed:", e);
  }
  return { found, books };
}

/** Take (read + consume) the pending proposal, if any. */
export async function takeProposal(): Promise<AurawriteProposal | null> {
  try {
    return await invoke<AurawriteProposal | null>("aurawrite_take_proposal");
  } catch (e) {
    console.error("[aurawrite] take proposal failed:", e);
    return null;
  }
}

export function renderAurawrite(
  aurawrite: AurawriteState,
  jobs: HistoryJobView[],
  loaded: boolean,
): string {
  if (!loaded) {
    return `<div class="card"><p class="field-help">Checking for AuraWrite…</p></div>`;
  }
  if (!aurawrite.found) {
    if (localStorage.getItem(INFO_KEY)) {
      return `<div class="card">
        <h2>AuraWrite</h2>
        <p class="field-help">AuraWrite is not reachable on this computer.</p>
        <button class="btn-secondary" id="aurawrite-refresh">🔄 Refresh</button>
      </div>`;
    }
    return `<div class="card">
      <h2>AuraWrite</h2>
      <p>AuraWrite is a desktop writing app (with EPUB reading and editing) that can send its ebooks here to be turned into audiobooks. To use this panel, AuraWrite must be installed and used on this computer.</p>
      <p>When you export an ebook from AuraWrite, Audiobook Generator opens with a request to accept it: you then choose the engine, the chapters and where to save the audiobook.</p>
      <p>Download AuraWrite at <a href="https://github.com/ACarloGitHub/AuraWrite/releases" target="_blank" rel="noopener">github.com/ACarloGitHub/AuraWrite/releases</a>.</p>
      <label class="checkbox-row">
        <input type="checkbox" id="aurawrite-dont-show" />
        <span>Don't show again</span>
      </label>
      <button class="btn-secondary" id="aurawrite-refresh">🔄 Refresh</button>
    </div>`;
  }

  // One table like the History panel: every catalog book has a row with the
  // status of its job next to it. Books without a job show "Not started"
  // (Reader) or the Editor note. Orphan AuraWrite jobs (books no longer in
  // the catalog) are listed afterwards with the same rows as History.
  const jobByBookId = new Map(
    jobs.filter((j) => j.aurawrite_book_id).map((j) => [j.aurawrite_book_id!, j]),
  );
  const rows = aurawrite.books
    .map((b) => {
      const job = jobByBookId.get(b.id);
      if (job) {
        const st = statusLabel(job);
        return `<tr>
          <td><span class="history-title" title="${escapeHtml(b.name)}">${escapeHtml(b.name)}</span></td>
          <td><span style="color:${st.color};font-weight:600;">${escapeHtml(st.text)}</span></td>
          <td>${escapeHtml(chapterSummary(job))}</td>
          <td>${job.engine_id ? `<span class="history-engine" title="${escapeHtml(job.engine_id)}">${escapeHtml(job.engine_id)}</span>` : "—"}</td>
          <td>${escapeHtml(formatDate(job.updated_at))}</td>
          <td><button class="btn-secondary btn-small" data-open-job-dir="${escapeHtml(job.book_dir)}">Open</button></td>
        </tr>`;
      }
      if (b.section === "reader") {
        return `<tr>
          <td><span class="history-title" title="${escapeHtml(b.name)}">${escapeHtml(b.name)}</span></td>
          <td><span style="color:var(--text-dim);">Not started</span></td>
          <td>—</td>
          <td>—</td>
          <td>—</td>
          <td><button class="btn-secondary btn-small" data-open-book-path="${escapeHtml(b.path)}">Open</button></td>
        </tr>`;
      }
      return `<tr>
        <td><span class="history-title" title="${escapeHtml(b.name)}">${escapeHtml(b.name)}</span></td>
        <td><span class="history-note" title="Export it first with the 🎧 button in AuraWrite">Export it first with the 🎧 button in AuraWrite</span></td>
        <td>—</td>
        <td>—</td>
        <td>—</td>
        <td></td>
      </tr>`;
    })
    .join("");

  const orphanIds = new Set(aurawrite.books.map((b) => b.id));
  const orphanJobs = jobs.filter((j) => j.aurawrite_book_id && !orphanIds.has(j.aurawrite_book_id));
  const orphanRows = renderJobRows(orphanJobs);

  const tableBody = rows + orphanRows;
  const empty = !tableBody
    ? `<p class="field-help">No ebooks published by AuraWrite yet. Export an ebook from AuraWrite to see it here.</p>`
    : "";

  return `<div class="card">
    <h2>AuraWrite ebooks</h2>
    <p class="field-help">Books exported from AuraWrite, with the status of their conversion jobs.</p>
    <table class="history-table">
      <thead>
        <tr>
          <th>Title</th><th>Status</th><th>Chapters</th><th>Engine</th><th>Last updated</th><th></th>
        </tr>
      </thead>
      <tbody>${tableBody}</tbody>
    </table>
    ${empty}
    <button class="btn-secondary" id="aurawrite-refresh">🔄 Refresh</button>
  </div>`;
}

export function attachAurawriteListeners(): void {
  document.getElementById("aurawrite-refresh")?.addEventListener("click", () => {
    window.dispatchEvent(new Event("aurawrite:refresh-requested"));
  });
  document.querySelectorAll("[data-open-book-path]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const path = btn.getAttribute("data-open-book-path");
      if (!path) return;
      window.dispatchEvent(
        new CustomEvent("aurawrite:open-catalog-book", { detail: { path } })
      );
    });
  });
  const dontShow = document.getElementById("aurawrite-dont-show") as HTMLInputElement | null;
  dontShow?.addEventListener("change", () => {
    localStorage.setItem(INFO_KEY, "1");
  });
}

/** English confirmation dialog for an incoming proposal. Resolves accept/reject. */
export function confirmProposal(proposal: AurawriteProposal): Promise<boolean> {
  const name = proposal.input.split(/[\\/]/).pop() ?? proposal.input;
  return showDialog({
    title: "Request from AuraWrite",
    body: `AuraWrite has proposed this ebook: <strong>${escapeHtml(name)}</strong>.<br/><br/>Accept it to load the book here, ready to choose the engine, the chapters and the output location.`,
    confirmLabel: "Accept",
    cancelLabel: "Reject",
  });
}

/** Warning shown when a proposal arrives while a generation is running. */
export function showBusyWarning(): void {
  void showDialog({
    title: "A generation is already running",
    body: "The book proposed by AuraWrite cannot be loaded until the current generation finishes. Please export it again from AuraWrite once this work is done.",
    confirmLabel: "OK",
    cancelLabel: null,
  });
}

function showDialog(opts: {
  title: string;
  body: string;
  confirmLabel: string;
  cancelLabel: string | null;
}): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.55);display:flex;align-items:center;justify-content:center;z-index:1000;";
    overlay.innerHTML = `
      <div class="card" style="max-width:460px;width:90%;">
        <h2>${escapeHtml(opts.title)}</h2>
        <p>${opts.body}</p>
        <div style="display:flex;justify-content:flex-end;gap:8px;">
          ${opts.cancelLabel ? `<button class="btn-secondary" data-dialog="cancel">${escapeHtml(opts.cancelLabel)}</button>` : ""}
          <button class="btn-primary" data-dialog="confirm">${escapeHtml(opts.confirmLabel)}</button>
        </div>
      </div>`;
    document.body.appendChild(overlay);
    const done = (ok: boolean): void => {
      overlay.remove();
      resolve(ok);
    };
    overlay.querySelector('[data-dialog="confirm"]')?.addEventListener("click", () => done(true));
    overlay.querySelector('[data-dialog="cancel"]')?.addEventListener("click", () => done(false));
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay && opts.cancelLabel) done(false);
    });
  });
}
