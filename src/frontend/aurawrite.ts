// AuraWrite panel (twin integration).
//
// The "AuraWrite" sidebar panel: shows an informative message when AuraWrite
// is not reachable, otherwise lists the ebooks from AuraWrite's unified
// catalog. Also handles the handover flow: reads the proposal AuraWrite
// wrote, asks the user to accept it, and reports busy state.

import { invoke } from "@tauri-apps/api/core";
import { escapeHtml } from "./helpers";

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

export function renderAurawrite(aurawrite: AurawriteState, loaded: boolean): string {
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
  const rows = aurawrite.books.length
    ? aurawrite.books
        .map((b) => {
          if (b.section === "reader") {
            return `<li>
              <strong>${escapeHtml(b.name)}</strong>
              <button class="btn-secondary btn-small" data-open-book-path="${escapeHtml(b.path)}">Open</button>
              <span class="field-help">(Reader)</span>
            </li>`;
          }
          return `<li>
            <strong>${escapeHtml(b.name)}</strong>
            <span class="field-help">(Editor — export it first with the 🎧 button in AuraWrite)</span>
          </li>`;
        })
        .join("")
    : `<p class="field-help">No ebooks published by AuraWrite yet. Export an ebook from AuraWrite to see it here.</p>`;
  return `<div class="card">
    <h2>AuraWrite ebooks</h2>
    <p class="field-help">Books exported from AuraWrite. When one is sent, accept the request to load it.</p>
    <ul>${rows}</ul>
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
