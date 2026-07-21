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
  qwentts_installed: boolean;
  qwentts_path: string | null;
  voxcpm2_installed: boolean;
  voxcpm2_path: string | null;
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
    case "components": return renderComponents();
    case "done": return renderDone();
    default: return `<p>${escapeHtml(step.description)}</p>`;
  }
}

function renderWelcome(): string {
  return `
    <p>Audiobook Generator converts EPUB books to audiobooks using local TTS (text-to-speech) models.</p>
    <p>Everything needed to run is <strong>bundled in the installer</strong> — no extra downloads, works offline:</p>
    <ul>
      <li><strong>FFmpeg</strong> — merges audio chunks into MP3</li>
      <li><strong>llama-server</strong> — runs GGUF models (OuteTTS)</li>
      <li><strong>qwen-tts</strong> — the Qwen3-TTS engine binary</li>
      <li><strong>voxcpm2-cli</strong> — the VoxCPM2 engine binary</li>
      <li><strong>ONNX Runtime</strong> — built into the app (OuteTTS DAC decoder)</li>
    </ul>
    <p>Only the TTS model weights are downloaded separately, from the <strong>Models</strong> panel, after setup.</p>
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

function componentRow(label: string, installed: boolean, path: string | null): string {
  return `
    <tr>
      <td><strong>${label}</strong></td>
      <td>${installed ? "✅ bundled" : "❌ not found"}</td>
      <td>${path ? `<code>${escapeHtml(path)}</code>` : ""}</td>
    </tr>
  `;
}

function renderComponents(): string {
  const allOk =
    (deps?.ffmpeg_installed ?? false) &&
    (deps?.llama_server_installed ?? false) &&
    (deps?.qwentts_installed ?? false);
  return `
    <h3>Bundled Components</h3>
    <p>These engine binaries ship inside the installer and should be present out of the box.</p>
    <table class="wizard-table">
      ${componentRow("FFmpeg", deps?.ffmpeg_installed ?? false, deps?.ffmpeg_path ?? null)}
      ${componentRow("llama-server", deps?.llama_server_installed ?? false, deps?.llama_server_path ?? null)}
      ${componentRow("qwen-tts", deps?.qwentts_installed ?? false, deps?.qwentts_path ?? null)}
      ${componentRow("voxcpm2-cli", deps?.voxcpm2_installed ?? false, deps?.voxcpm2_path ?? null)}
      <tr>
        <td><strong>ONNX Runtime</strong></td>
        <td>${deps?.ort_installed ? "✅ built-in" : "❌ not available"}</td>
        <td></td>
      </tr>
      <tr>
        <td><strong>cuDNN 9</strong></td>
        <td>not required</td>
        <td></td>
      </tr>
    </table>
    ${!allOk ? `<p class="field-help">Some components are missing. If you installed via the official installer, try reinstalling. In a development checkout, place the binaries under <code>resources/</code> in the working directory.</p>` : ""}
  `;
}

function renderDone(): string {
  return `
    <h3>Setup Complete!</h3>
    <p>All components are in place. You can now:</p>
    <ol>
      <li>Go to the <strong>Models</strong> panel to download TTS models (Qwen3-TTS, OuteTTS, VoxCPM2)</li>
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
}
