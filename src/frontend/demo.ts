import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { escapeHtml, ts, appendLog, setLog, collectParamExtras } from "./helpers";
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
      <textarea class="text-input" id="demo-status" rows="3" readonly placeholder="Status"></textarea>
      <audio id="demo-audio" controls style="${state.demoAudioPath ? "width:100%; margin-top:8px;" : "display:none; width:100%; margin-top:8px;"}"${state.demoAudioPath ? ` src="${convertFileSrc(state.demoAudioPath)}"` : ""}></audio>
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
      <audio id="test-audio" controls style="${state.testAudioPath ? "width:100%; margin-top:8px;" : "display:none; width:100%; margin-top:8px;"}"${state.testAudioPath ? ` src="${convertFileSrc(state.testAudioPath)}"` : ""}></audio>
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
      const audio = document.getElementById("demo-audio") as HTMLAudioElement | null;
      const outDir = state.demoOutputPath ?? "Demo_Outputs";
      const out = `${outDir}/demo_${Date.now()}.wav`;
      setLog("demo-status", `[INFO] Synthesizing with ${state.selectedEngineId}...\n`);

      // Engine params: same shared resolver used by Test and Generate, so
      // the values the user set in Configuration are the ones sent.
      const extra = collectParamExtras();

      try {
        const resultPath = await invoke<string>("synthesize_demo", {
          engineId: state.selectedEngineId,
          text,
          voice: state.selectedVoiceId || null,
          language: state.selectedLanguage || null,
          speed: state.speed,
          outputWav: out,
          extra,
          referenceAudio:
            state.selectedEngineId.startsWith("VoxCPM2") && state.voxMode === "design"
              ? null
              : state.referenceWavPath,
          maxChars: state.chunkMaxCharsByLang[state.selectedLanguage] ?? state.chunkMaxChars,
          maxWords: state.chunkStrategy === "Character Limit" ? 999999 : state.chunkMaxWords,
        });
        setLog("demo-status", `[INFO] Saved to ${resultPath}\n`);
        if (audio) {
          state.demoAudioPath = resultPath;
          audio.src = convertFileSrc(resultPath);
          audio.style.display = "block";
          audio.load();
        }
      } catch (e) {
        setLog("demo-status", `[ERROR] ${e}\n`);
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
      if (testAudio) { testAudio.style.display = "none"; testAudio.src = ""; state.testAudioPath = null; }
      setLog("test-status", `[${ts()}] [INFO] Resolving test EPUB...\n`);

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

        appendLog("test-status", `[${ts()}] [INFO] Test EPUB: ${testEpubPath}\n`);
        appendLog("test-status", `[${ts()}] [INFO] Output dir: ${outputDir}\n`);
        appendLog("test-status", `[${ts()}] [INFO] Engine: ${state.selectedEngineId}\n`);
        appendLog("test-status", `[${ts()}] [INFO] --- starting test generation ---\n`);

        // Engine params: shared resolver (Configuration values included).
        const extra = collectParamExtras();

        const maxCharsForLang =
          state.chunkMaxCharsByLang[state.selectedLanguage] ?? state.chunkMaxChars;
        const effectiveMaxWords =
          state.chunkStrategy === "Character Limit" ? 999999 : state.chunkMaxWords;

        const t0 = Date.now();

        let unlistenProgress: (() => void) | null = null;
        let unlistenComplete: (() => void) | null = null;

        try {
          unlistenProgress = await listen<string>("generation-progress", (e) => {
            appendLog("test-status", `[${ts()}] ${e.payload}\n`);
          });
          unlistenComplete = await listen("generation-complete", () => {
            const secs = ((Date.now() - t0) / 1000).toFixed(1);
            appendLog("test-status", `[${ts()}] [INFO] Test generation finished in ${secs}s\n`);
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
            onlyChapters: null,
            referenceAudio:
              state.selectedEngineId.startsWith("VoxCPM2") && state.voxMode === "design"
                ? null
                : state.referenceWavPath,
          });

          try {
            const mp3s = await invoke<string[]>("list_mp3s_in_dir", { dir: outputDir });
            if (mp3s.length > 0 && testAudio) {
              state.testAudioPath = mp3s[0];
              testAudio.src = convertFileSrc(mp3s[0]);
              testAudio.style.display = "block";
              testAudio.load();
              appendLog("test-status", `[${ts()}] [INFO] Playing: ${mp3s[0]}\n`);
            }
          } catch (e) {
            appendLog("test-status", `[${ts()}] [WARN] Could not load audio player: ${e}\n`);
          }
        } finally {
          if (unlistenProgress) unlistenProgress();
          if (unlistenComplete) unlistenComplete();
        }
      } catch (e) {
        appendLog("test-status", `[${ts()}] [ERROR] ${e}\n`);
      } finally {
        testBtn.disabled = false;
      }
    });
  }
}