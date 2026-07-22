export function $(sel: string): HTMLElement {
  const el = document.querySelector(sel);
  if (!el) throw new Error(`Missing element: ${sel}`);
  return el as HTMLElement;
}

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function ts(): string {
  const d = new Date();
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function bytesToGB(n: number | null): string {
  if (n === null || n === undefined) return "?";
  return (n / 1024 / 1024 / 1024).toFixed(2);
}

export function pickOutputDir(bookTitle: string): string {
  const safe = bookTitle.replace(/[^a-zA-Z0-9-_ ]/g, "_").trim() || "audiobook";
  return `Generated_Audiobooks/${safe}`;
}

export function hardwareLine(status: import("./types").EngineStatus): string {
  const hw = status.hardware;
  const gpu = hw.gpus[0];
  if (!gpu) return `${hw.os} · ${hw.arch} · no GPU detected`;
  return `${hw.os} · ${gpu.vendor} ${gpu.model} · ${bytesToGB(gpu.vram_bytes)} GB VRAM`;
}

// Log textareas are re-created on every full re-render (e.g. when the
// engine loads mid-generation). Always look the element up fresh: writing
// to a stale reference would silently swallow the log lines.
export function appendLog(id: string, line: string): void {
  const el = document.getElementById(id) as HTMLTextAreaElement | null;
  if (el) {
    el.value += line;
    el.scrollTop = el.scrollHeight;
  }
}

export function setLog(id: string, text: string): void {
  const el = document.getElementById(id) as HTMLTextAreaElement | null;
  if (el) {
    el.value = text;
    el.scrollTop = el.scrollHeight;
  }
}