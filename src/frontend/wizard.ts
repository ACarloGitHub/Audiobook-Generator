import { invoke } from "@tauri-apps/api/core";
import { escapeHtml, bytesToGB } from "./helpers";

export interface HardwareInfo {
  os: string;
  arch: string;
  cpu_cores: number;
  ram_total_gb: number;
  ram_free_gb: number;
  gpu_vendor: string | null;
  gpu_model: string | null;
  gpu_vram_bytes: number | null;
  gpu_driver_version: string | null;
  recommended_backend: string;
}

export interface DependencyStatus {
  ffmpeg_installed: boolean;
  ffmpeg_path: string | null;
  llama_server_installed: boolean;
  llama_server_path: string | null;
  ort_installed: boolean;
  cudnn_installed: boolean;
}

export interface WizardStep {
  id: string;
  title: string;
  description: string;
  completed: boolean;
}

let currentStep = 0;
let steps: WizardStep[] = [];
let hardware: HardwareInfo | null = null;
let deps: DependencyStatus | null = null;

export function renderWizard(): string {
  return `
    <div class="wizard-overlay" id="wizard-overlay">
      <div class="wizard-card">
        <div class="wizard-header">
          <h1>Audiobook Generator — First Run Setup</h1>
          <p class="wizard-subtitle">Step ${currentStep + 1} of ${steps.length}: ${steps.length > 0 ? escapeHtml(steps[currentStep]?.title ?? "") : "Loading..."}</p>
        </div>
        <div class="wizard-body" id="wizard-body">
          ${renderStepContent()}
        </div>
        <div class="wizard-footer">
          <button class="btn-secondary" id="wizard-back" ${currentStep === 0 ? "disabled" : ""}>Back</button>
          <button class="btn-primary" id="wizard-next">${currentStep === steps.length - 1 ? "Finish" : "Next"}</button>
          <button class="btn-secondary" id="wizard-skip">Skip wizard</button>
        </div>
      </div>
    </div>
  `;
}

function renderStepContent(): string {
  if (steps.length === 0) return "<p>Loading...</p>";
  const step = steps[currentStep];
  switch (step.id) {
    case "welcome": return renderWelcome();
    case "hardware": return renderHardware();
    case "ffmpeg": return renderFfmpeg();
    case "llama_server": return renderLlamaServer();
    case "ort": return renderOrt();
    case "done": return renderDone();
    default: return `<p>${escapeHtml(step.description)}</p>`;
  }
}

function renderWelcome(): string {
  return `
    <p>Audiobook Generator converts EPUB books to audiobooks using local TTS (text-to-speech) models.</p>
    <p>Before you can use it, a few system components need to be installed:</p>
    <ul>
      <li><strong>FFmpeg</strong> — merges audio chunks into MP3</li>
      <li><strong>llama-server</strong> — runs GGUF models (Qwen3-TTS, OuteTTS, etc.)</li>
      <li><strong>ONNX Runtime + cuDNN</strong> — runs Kokoro in-process</li>
    </ul>
    <p>Models are downloaded separately from the <strong>Models</strong> panel after setup.</p>
  `;
}

function renderHardware(): string {
  if (!hardware) return "<p>Detecting hardware...</p>";
  const gpu = hardware.gpu_model
    ? `${hardware.gpu_vendor ?? ""} ${hardware.gpu_model} (${bytesToGB(hardware.gpu_vram_bytes ?? 0)} GB VRAM)`
    : "No GPU detected";
  return `
    <table class="wizard-table">
      <tr><td><strong>OS</strong></td><td>${escapeHtml(hardware.os)}</td></tr>
      <tr><td><strong>Architecture</strong></td><td>${escapeHtml(hardware.arch)}</td></tr>
      <tr><td><strong>CPU cores</strong></td><td>${hardware.cpu_cores}</td></tr>
      <tr><td><strong>RAM</strong></td><td>${hardware.ram_total_gb.toFixed(1)} GB total / ${hardware.ram_free_gb.toFixed(1)} GB free</td></tr>
      <tr><td><strong>GPU</strong></td><td>${escapeHtml(gpu)}</td></tr>
      ${hardware.gpu_driver_version ? `<tr><td><strong>Driver</strong></td><td>${escapeHtml(hardware.gpu_driver_version)}</td></tr>` : ""}
      <tr><td><strong>Recommended backend</strong></td><td>${escapeHtml(hardware.recommended_backend)}</td></tr>
    </table>
  `;
}

function renderFfmpeg(): string {
  const installed = deps?.ffmpeg_installed ?? false;
  const path = deps?.ffmpeg_path ?? null;
  return `
    <h3>FFmpeg</h3>
    <p>FFmpeg merges audio chunks into MP3 files. It is required for all engines.</p>
    <p class="field-help">Status: ${installed ? "✅ installed" : "❌ not found"}${path ? ` at <code>${escapeHtml(path)}</code>` : ""}</p>
    ${!installed ? `
      <button class="btn-primary" id="wizard-download-ffmpeg">Download FFmpeg</button>
      <p class="field-help" id="ffmpeg-log"></p>
    ` : ""}
    <details class="accordion">
      <summary>Manual installation</summary>
      <p class="field-help">
        Windows: <code>choco install ffmpeg</code> or download from <code>https://ffmpeg.org/download.html</code><br/>
        macOS: <code>brew install ffmpeg</code><br/>
        Linux: <code>sudo apt install ffmpeg</code> or <code>sudo dnf install ffmpeg</code>
      </p>
    </details>
  `;
}

function renderLlamaServer(): string {
  const installed = deps?.llama_server_installed ?? false;
  const path = deps?.llama_server_path ?? null;
  return `
    <h3>llama-server</h3>
    <p>llama-server is the inference engine for GGUF models (Qwen3-TTS, OuteTTS, NeuTTS Air, VibeVoice). It is <strong>not</strong> needed for Kokoro (which uses ONNX Runtime).</p>
    <p class="field-help">Status: ${installed ? "✅ installed" : "❌ not found"}${path ? ` at <code>${escapeHtml(path)}</code>` : ""}</p>
    ${!installed ? `
      <button class="btn-primary" id="wizard-download-llama">Download llama-server</button>
      <p class="field-help" id="llama-log"></p>
    ` : ""}
    <details class="accordion">
      <summary>Manual installation</summary>
      <p class="field-help">
        Download from <code>https://github.com/ggergan/llama.cpp/releases</code>. Place the binary in your PATH or in the app's bin/ directory.
      </p>
    </details>
  `;
}

function renderOrt(): string {
  const ortOk = deps?.ort_installed ?? false;
  const cudnnOk = deps?.cudnn_installed ?? false;
  return `
    <h3>ONNX Runtime + cuDNN</h3>
    <p>ONNX Runtime is required for Kokoro (in-process synthesis). cuDNN accelerates inference on NVIDIA GPUs.</p>
    <p class="field-help">ONNX Runtime model: ${ortOk ? "✅ found" : "❌ not found (download Kokoro from the Models panel first)"}</p>
    <p class="field-help">cuDNN 9: ${cudnnOk ? "✅ found" : "❌ not found (CPU-only mode will be used)"}</p>
    <details class="accordion">
      <summary>Manual cuDNN installation</summary>
      <p class="field-help">
        Windows: Download cuDNN 9 from <code>https://developer.nvidia.com/cudnn</code> and place the DLLs in <code>C:\\Windows\\System32\\</code> or next to the app executable.<br/>
        Linux: Install via package manager or download from NVIDIA.
      </p>
    </details>
  `;
}

function renderDone(): string {
  return `
    <h3>Setup Complete!</h3>
    <p>All system dependencies are in place. You can now:</p>
    <ol>
      <li>Go to the <strong>Models</strong> panel to download TTS models (Kokoro, Qwen3-TTS, etc.)</li>
      <li>Go to <strong>Configuration</strong> to select an engine and voice</li>
      <li>Go to <strong>EPUB & Options</strong> to load a book</li>
      <li>Go to <strong>Generate</strong> to create your audiobook</li>
    </ol>
  `;
}

export async function initWizard(): Promise<boolean> {
  const done = await invoke<boolean>("is_wizard_done");
  if (done) return false;
  steps = await invoke<WizardStep[]>("get_wizard_steps");
  hardware = await invoke<HardwareInfo>("detect_hardware");
  deps = await invoke<DependencyStatus>("check_dependencies");
  return true;
}

export function attachWizardListeners(rerender: () => void, closeWizard: () => void): void {
  const backBtn = document.getElementById("wizard-back");
  const nextBtn = document.getElementById("wizard-next");
  const skipBtn = document.getElementById("wizard-skip");

  if (backBtn) {
    backBtn.addEventListener("click", () => {
      if (currentStep > 0) {
        currentStep--;
        rerender();
      }
    });
  }

  if (nextBtn) {
    nextBtn.addEventListener("click", async () => {
      if (currentStep < steps.length - 1) {
        currentStep++;
        if (steps.length > 0) steps[currentStep] = { ...steps[currentStep], completed: true };
        deps = await invoke<DependencyStatus>("check_dependencies");
        rerender();
      } else {
        await invoke("mark_wizard_done");
        closeWizard();
      }
    });
  }

  if (skipBtn) {
    skipBtn.addEventListener("click", async () => {
      await invoke("mark_wizard_done");
      closeWizard();
    });
  }

  const downloadFfmpegBtn = document.getElementById("wizard-download-ffmpeg");
  if (downloadFfmpegBtn) {
    downloadFfmpegBtn.addEventListener("click", async () => {
      const log = document.getElementById("ffmpeg-log");
      if (log) log.textContent = "Downloading FFmpeg...";
      try {
        const result = await invoke<string>("download_ffmpeg");
        if (log) log.textContent = result;
      } catch (e) {
        if (log) log.textContent = `Error: ${e}`;
      }
    });
  }

  const downloadLlamaBtn = document.getElementById("wizard-download-llama");
  if (downloadLlamaBtn) {
    downloadLlamaBtn.addEventListener("click", async () => {
      const log = document.getElementById("llama-log");
      if (log) log.textContent = "Downloading llama-server...";
      try {
        const result = await invoke<string>("download_llama_server");
        if (log) log.textContent = result;
      } catch (e) {
        if (log) log.textContent = `Error: ${e}`;
      }
    });
  }
}