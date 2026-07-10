import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { escapeHtml, ts } from "./helpers";
import { state } from "./state";
import type { EngineStatus } from "./types";

export function renderDemo(_status: EngineStatus): string {
  return `
    <div class="card">
      <h2>Demo synthesis</h2>
      <label class="field-label">Text</label>
      <textarea class="text-input" id="demo-text" rows="3" placeholder="Type a sentence to synthesize..."></textarea>
      <div class="btn-row">
        <button class="btn-secondary" id="demo-pick-output-btn">Choose output path...</button>
        <span class="field-help" id="demo-output-path">${state.demoOutputPath ? escapeHtml(state.demoOutputPath) : "Default: <output_dir>/demo_<timestamp>.wav"}</span>
      </div>
      <button class="btn-secondary btn-large" id="demo-generate-btn" disabled>Generate Demo</button>
      <label class="field-label">Status</label>
      <textarea class="text-input" id="demo-status" rows="1" readonly placeholder="Status"></textarea>
      <audio id="demo-audio" controls style="display:none; width:100%; margin-top:8px;"></audio>
    </div>

    <div class="card">
      <h2>Test file generation</h2>
      <p class="field-help">Runs bundled mini-EPUBs end-to-end through the same pipeline as a real book.</p>
      <div class="btn-row">
        <button class="btn-secondary" id="test-pick-output-btn">Choose output path...</button>
        <span class="field-help" id="test-output-path">${state.testOutputPath ? escapeHtml(state.testOutputPath) : "Default: app data folder"}</span>
      </div>
      <button class="btn-secondary btn-large" id="test-file-btn">Run Test File Generation</button>
      <label class="field-label">Test Status</label>
      <textarea class="text-input log-area" id="test-status" rows="8" readonly placeholder="No test run yet."></textarea>
      <audio id="test-audio" controls style="display:none; width:100%; margin-top:8px;"></audio>
    </div>
  `;
}

export function attachDemoListeners(): void {
  const demoOutputBtn = document.getElementById("demo-pick-output-btn");
  if (demoOutputBtn) {
    demoOutputBtn.addEventListener("click", async () => {
      try {
        const path = await open({ multiple: false, directory: true });
        if (typeof path === "string") {
          state.demoOutputPath = path;
          const pathEl = document.getElementById("demo-output-path");
          if (pathEl) pathEl.textContent = path;
        }
      } catch (e) {
        console.warn("dialog open failed:", e);
      }
    });
  }

  const demoGenBtn = document.getElementById("demo-generate-btn") as HTMLButtonElement | null;
  if (demoGenBtn) {
    demoGenBtn.disabled = false;
    demoGenBtn.addEventListener("click", async () => {
      const text = (document.getElementById("demo-text") as HTMLTextAreaElement | null)?.value ?? "";
      if (!text.trim()) {
        alert("Type some text first.");
        return;
      }
      const status = document.getElementById("demo-status") as HTMLTextAreaElement | null;
      const audio = document.getElementById("demo-audio") as HTMLAudioElement | null;
      const outDir = state.demoOutputPath ?? "Demo_Outputs";
      const out = `${outDir}/demo_${Date.now()}.wav`;
      if (status) status.value = `[INFO] Synthesizing with ${state.selectedEngineId}...\n`;

      // Build extra params for Qwen3-TTS — read from state (persists across panels)
      // with DOM element as fallback for live edits
      const extra: Record<string, string> = {};
      const instructEl = document.getElementById("qwen-instruct-input") as HTMLInputElement | HTMLTextAreaElement | null;
      const instructVal = instructEl?.value?.trim() || state.qwenInstruct?.trim();
      if (instructVal) {
        extra["instruct"] = instructVal;
      }
      const refTextEl = document.getElementById("qwen-ref-text") as HTMLTextAreaElement | null;
      const refTextVal = refTextEl?.value?.trim() || state.referenceTranscript?.trim();
      if (refTextVal) {
        extra["ref_text"] = refTextVal;
      }
      // Advanced params (DOM fallback to state defaults via configuration values)
      const tempEl = document.getElementById("qwen-temp") as HTMLInputElement | null;
      if (tempEl && tempEl.value) extra["temp"] = tempEl.value;
      const topKEl = document.getElementById("qwen-top-k") as HTMLInputElement | null;
      if (topKEl && topKEl.value) extra["top_k"] = topKEl.value;
      const topPEl = document.getElementById("qwen-top-p") as HTMLInputElement | null;
      if (topPEl && topPEl.value) extra["top_p"] = topPEl.value;
      const repPenEl = document.getElementById("qwen-rep-pen") as HTMLInputElement | null;
      if (repPenEl && repPenEl.value) extra["rep_pen"] = repPenEl.value;
      const maxNewEl = document.getElementById("qwen-max-new") as HTMLInputElement | null;
      if (maxNewEl && maxNewEl.value) extra["max_new"] = maxNewEl.value;
      const seedEl = document.getElementById("qwen-seed") as HTMLInputElement | null;
      if (seedEl && seedEl.value) extra["seed"] = seedEl.value;

      // OuteTTS params (different param names than Qwen)
      const outeTemp = document.getElementById("oute-temperature") as HTMLInputElement | null;
      if (outeTemp && outeTemp.value) extra["temperature"] = outeTemp.value;
      const outeTopK = document.getElementById("oute-top-k") as HTMLInputElement | null;
      if (outeTopK && outeTopK.value) extra["top_k"] = outeTopK.value;
      const outeTopP = document.getElementById("oute-top-p") as HTMLInputElement | null;
      if (outeTopP && outeTopP.value) extra["top_p"] = outeTopP.value;
      const outeMinP = document.getElementById("oute-min-p") as HTMLInputElement | null;
      if (outeMinP && outeMinP.value) extra["min_p"] = outeMinP.value;
      const outeRepPen = document.getElementById("oute-rep-pen") as HTMLInputElement | null;
      if (outeRepPen && outeRepPen.value) extra["repetition_penalty"] = outeRepPen.value;
      const outeMaxTokens = document.getElementById("oute-max-tokens") as HTMLInputElement | null;
      if (outeMaxTokens && outeMaxTokens.value) extra["max_tokens"] = outeMaxTokens.value;

      if (state.selectedEngineId.startsWith("OuteTTS")) {
        if (state.outeSpeakerJsonPath) {
          extra["speaker_json"] = state.outeSpeakerJsonPath;
        } else if (state.selectedVoiceId) {
          extra["speaker"] = state.selectedVoiceId;
        }
        const ctxSize = state.engineGeneration["ctx_size"]?.default;
        if (ctxSize !== undefined && ctxSize !== null) {
          extra["ctx_size"] = String(ctxSize);
        }
      }

      try {
        const resultPath = await invoke<string>("synthesize_demo", {
          engineId: state.selectedEngineId,
          text,
          voice: state.selectedVoiceId || null,
          language: state.selectedLanguage || null,
          speed: state.speed,
          outputWav: out,
          extra,
          referenceAudio: state.referenceWavPath,
        });
        if (status) status.value = `[INFO] Saved to ${resultPath}\n`;
        if (audio) {
          audio.src = convertFileSrc(resultPath);
          audio.style.display = "block";
          audio.load();
        }
      } catch (e) {
        if (status) status.value = `[ERROR] ${e}\n`;
      }
    });
  }

  const testPickBtn = document.getElementById("test-pick-output-btn");
  if (testPickBtn) {
    testPickBtn.addEventListener("click", async () => {
      try {
        const path = await open({ multiple: false, directory: true });
        if (typeof path === "string") {
          state.testOutputPath = path;
          const pathEl = document.getElementById("test-output-path");
          if (pathEl) pathEl.textContent = path;
        }
      } catch (e) {
        console.warn("dialog open failed:", e);
      }
    });
  }

  const testBtn = document.getElementById("test-file-btn") as HTMLButtonElement | null;
  const testStatus = document.getElementById("test-status") as HTMLTextAreaElement | null;
  const testAudio = document.getElementById("test-audio") as HTMLAudioElement | null;

  if (testBtn && testStatus) {
    testBtn.addEventListener("click", async () => {
      testBtn.disabled = true;
      if (testAudio) { testAudio.style.display = "none"; testAudio.src = ""; }
      testStatus.value = `[${ts()}] [INFO] Resolving test EPUB...\n`;

      try {
        const langForTest = state.selectedLanguage || "en";
        const testEpubPath = await invoke<string>("get_test_epub", {
          language: langForTest,
        });

        const langMap: Record<string, string> = {
          italian: "it", english: "en", spanish: "es", french: "fr",
          german: "de", portuguese: "pt", japanese: "ja", russian: "ru",
          chinese: "cn", korean: "ko", auto: "en",
        };
        const langSuffix = langMap[langForTest.toLowerCase()] ?? langForTest.slice(0, 2).toLowerCase();
        const title = `TEST_${langSuffix}_${state.selectedEngineId}`;
        const safeTitle = title.replace(/[^a-zA-Z0-9-_ ]/g, "_");
        const outputDir = state.testOutputPath
          ? `${state.testOutputPath}/${safeTitle}`
          : `Generated_Audiobooks/${safeTitle}`;

        testStatus.value += `[${ts()}] [INFO] Test EPUB: ${testEpubPath}\n`;
        testStatus.value += `[${ts()}] [INFO] Output dir: ${outputDir}\n`;
        testStatus.value += `[${ts()}] [INFO] Engine: ${state.selectedEngineId}\n`;
        testStatus.value += `[${ts()}] [INFO] --- starting test generation ---\n`;

        const extra: Record<string, string> = {};
        const instructEl = document.getElementById("qwen-instruct-input") as HTMLInputElement | HTMLTextAreaElement | null;
        const instructVal = instructEl?.value?.trim() || state.qwenInstruct?.trim();
        if (instructVal) {
          extra["instruct"] = instructVal;
        }
        const refTextEl = document.getElementById("qwen-ref-text") as HTMLTextAreaElement | null;
        const refTextVal = refTextEl?.value?.trim() || state.referenceTranscript?.trim();
        if (refTextVal) {
          extra["ref_text"] = refTextVal;
        }
        const tempEl = document.getElementById("qwen-temp") as HTMLInputElement | null;
        if (tempEl && tempEl.value) extra["temp"] = tempEl.value;
        const topKEl = document.getElementById("qwen-top-k") as HTMLInputElement | null;
        if (topKEl && topKEl.value) extra["top_k"] = topKEl.value;
        const topPEl = document.getElementById("qwen-top-p") as HTMLInputElement | null;
        if (topPEl && topPEl.value) extra["top_p"] = topPEl.value;
        const repPenEl = document.getElementById("qwen-rep-pen") as HTMLInputElement | null;
        if (repPenEl && repPenEl.value) extra["rep_pen"] = repPenEl.value;
        const maxNewEl = document.getElementById("qwen-max-new") as HTMLInputElement | null;
        if (maxNewEl && maxNewEl.value) extra["max_new"] = maxNewEl.value;
        const seedEl = document.getElementById("qwen-seed") as HTMLInputElement | null;
        if (seedEl && seedEl.value) extra["seed"] = seedEl.value;

        const outeTemp = document.getElementById("oute-temperature") as HTMLInputElement | null;
        if (outeTemp && outeTemp.value) extra["temperature"] = outeTemp.value;
        const outeTopK = document.getElementById("oute-top-k") as HTMLInputElement | null;
        if (outeTopK && outeTopK.value) extra["top_k"] = outeTopK.value;
        const outeTopP = document.getElementById("oute-top-p") as HTMLInputElement | null;
        if (outeTopP && outeTopP.value) extra["top_p"] = outeTopP.value;
        const outeMinP = document.getElementById("oute-min-p") as HTMLInputElement | null;
        if (outeMinP && outeMinP.value) extra["min_p"] = outeMinP.value;
        const outeRepPen = document.getElementById("oute-rep-pen") as HTMLInputElement | null;
        if (outeRepPen && outeRepPen.value) extra["repetition_penalty"] = outeRepPen.value;
        const outeMaxTokens = document.getElementById("oute-max-tokens") as HTMLInputElement | null;
        if (outeMaxTokens && outeMaxTokens.value) extra["max_tokens"] = outeMaxTokens.value;

        if (state.selectedEngineId.startsWith("OuteTTS")) {
          if (state.outeSpeakerJsonPath) {
            extra["speaker_json"] = state.outeSpeakerJsonPath;
          } else if (state.selectedVoiceId) {
            extra["speaker"] = state.selectedVoiceId;
          }
        }

        const maxCharsForLang =
          state.chunkMaxCharsByLang[state.selectedLanguage] ?? state.chunkMaxChars;
        const effectiveMaxWords =
          state.chunkStrategy === "Character Limit" ? 999999 : state.chunkMaxWords;

        const t0 = Date.now();

        let unlistenProgress: (() => void) | null = null;
        let unlistenComplete: (() => void) | null = null;

        try {
          unlistenProgress = await listen<string>("generation-progress", (e) => {
            testStatus.value += `[${ts()}] ${e.payload}\n`;
            testStatus.scrollTop = testStatus.scrollHeight;
          });
          unlistenComplete = await listen("generation-complete", () => {
            const secs = ((Date.now() - t0) / 1000).toFixed(1);
            testStatus.value += `[${ts()}] [INFO] Test generation finished in ${secs}s\n`;
            testStatus.scrollTop = testStatus.scrollHeight;
          });

          await invoke("start_generation", {
            engineId: state.selectedEngineId,
            voice: state.selectedVoiceId || null,
            language: state.selectedLanguage || null,
            speed: state.speed,
            epubPath: testEpubPath,
            outputDir,
            maxWords: effectiveMaxWords,
            maxChars: maxCharsForLang,
            extra,
            referenceAudio: state.referenceWavPath,
          });

          try {
            const mp3s = await invoke<string[]>("list_mp3s_in_dir", { dir: outputDir });
            if (mp3s.length > 0 && testAudio) {
              testAudio.src = convertFileSrc(mp3s[0]);
              testAudio.style.display = "block";
              testAudio.load();
              testStatus.value += `[${ts()}] [INFO] Playing: ${mp3s[0]}\n`;
            }
          } catch (e) {
            testStatus.value += `[${ts()}] [WARN] Could not load audio player: ${e}\n`;
          }
        } finally {
          if (unlistenProgress) unlistenProgress();
          if (unlistenComplete) unlistenComplete();
        }
      } catch (e) {
        testStatus.value += `[${ts()}] [ERROR] ${e}\n`;
      } finally {
        testBtn.disabled = false;
      }
    });
  }
}