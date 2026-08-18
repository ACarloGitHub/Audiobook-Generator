// History panel (A7): lists every generation job from the persistent
// history (jobs_history.json) with its status, and lets the user open it —
// resuming the chapters not yet converted, or restarting from scratch.

import { invoke } from "@tauri-apps/api/core";
import { escapeHtml } from "./helpers";
import type { HistoryJobView } from "./types";

export async function loadHistory(): Promise<HistoryJobView[]> {
  try {
    return await invoke<HistoryJobView[]>("history_list");
  } catch (e) {
    console.error("[history] list failed:", e);
    return [];
  }
}

export function statusLabel(job: HistoryJobView): { text: string; color: string } {
  if (!job.dir_exists) return { text: "Folder missing", color: "#e06c6c" };
  if (job.status === "completed") return { text: "Completed", color: "#4caf50" };
  return { text: "In progress", color: "#e6a23c" };
}

export function chapterSummary(job: HistoryJobView): string {
  const total = job.total_count > 0 ? job.total_count : "?";
  return `${job.converted_count}/${total}`;
}

export function formatDate(ts: string | null): string {
  if (!ts) return "";
  const n = Number(ts);
  if (!Number.isFinite(n) || n <= 0) return "";
  return new Date(n * 1000).toLocaleDateString();
}

/** Shared table rows for the History panel and the AuraWrite panel. */
export function renderJobRows(jobs: HistoryJobView[]): string {
  if (jobs.length === 0) {
    return `<tr><td colspan="6" class="field-help">No jobs yet. Generate an audiobook to see it here.</td></tr>`;
  }
  return jobs
    .map((job) => {
      const st = statusLabel(job);
      const safeTitle = escapeHtml(job.title);
      return `<tr>
        <td>
          <span class="history-title" title="${safeTitle}">${safeTitle}</span>
        </td>
        <td><span style="color:${st.color};font-weight:600;">${escapeHtml(st.text)}</span></td>
        <td>${escapeHtml(chapterSummary(job))}</td>
        <td>${job.engine_id ? `<span class="history-engine" title="${escapeHtml(job.engine_id)}">${escapeHtml(job.engine_id)}</span>` : "—"}</td>
        <td>${escapeHtml(formatDate(job.updated_at))}</td>
        <td>
          <button class="btn-secondary btn-small" data-open-job-dir="${escapeHtml(job.book_dir)}">Open</button>
        </td>
      </tr>`;
    })
    .join("");
}

/**
 * Column widths are set by the fixed CSS percentages in styles.css, which
 * are identical for History and the AuraWrite panel. No runtime adjustment.
 */

export function renderHistory(jobs: HistoryJobView[], loaded: boolean): string {
  if (!loaded) {
    return `<div class="card"><p class="field-help">Loading jobs…</p></div>`;
  }
  return `<div class="card">
    <h2>Job history</h2>
    <p class="field-help">
      Every conversion job is kept here, even after the output folder was
      deleted. Open a job to resume the chapters not yet converted with the
      saved settings, or to restart it from scratch.
    </p>
    <table class="history-table">
      <thead>
        <tr>
          <th>Title</th>
          <th>Status</th>
          <th>Chapters</th>
          <th>Engine</th>
          <th>Last updated</th>
          <th></th>
        </tr>
      </thead>
      <tbody>${renderJobRows(jobs)}</tbody>
    </table>
    <div class="btn-row">
      <button class="btn-secondary" id="history-refresh">🔄 Refresh</button>
    </div>
  </div>`;
}

export function attachHistoryListeners(): void {
  document.getElementById("history-refresh")?.addEventListener("click", () => {
    window.dispatchEvent(new Event("history:refresh-requested"));
  });
  document.querySelectorAll("[data-open-job-dir]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const dir = btn.getAttribute("data-open-job-dir");
      if (!dir) return;
      window.dispatchEvent(
        new CustomEvent("history:open-job", { detail: { dir } })
      );
    });
  });
}

/**
 * English dialog shown when a job's output folder no longer exists.
 * Resolves to "continue" | "restart" | null (cancel).
 */
export function confirmMissingFolder(): Promise<"continue" | "restart" | null> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.55);display:flex;align-items:center;justify-content:center;z-index:1000;";
    overlay.innerHTML = `
      <div class="card" style="max-width:480px;width:90%;">
        <h2>Output folder missing</h2>
        <p>The output folder is no longer present at its designated path. The conversion work had already been started. You may have transferred the audio files to another device. If you want to continue the work from where it left off, press "Continue". If you want to start it over from scratch, press "Restart".</p>
        <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:12px;">
          <button class="btn-secondary" data-mf="cancel">Cancel</button>
          <button class="btn-secondary" data-mf="restart">Restart</button>
          <button class="btn-primary" data-mf="continue">Continue</button>
        </div>
      </div>`;
    document.body.appendChild(overlay);
    const done = (v: "continue" | "restart" | null): void => {
      overlay.remove();
      resolve(v);
    };
    overlay.querySelector('[data-mf="continue"]')?.addEventListener("click", () => done("continue"));
    overlay.querySelector('[data-mf="restart"]')?.addEventListener("click", () => done("restart"));
    overlay.querySelector('[data-mf="cancel"]')?.addEventListener("click", () => done(null));
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) done(null);
    });
  });
}
