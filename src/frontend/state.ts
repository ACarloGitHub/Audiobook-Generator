import type { PanelId, GenerationParam } from "./types";

export interface AppState {
  currentPanel: PanelId;
  selectedEngineId: string;
  selectedLanguage: string;
  selectedVoiceId: string;
  selectedSeparator: string;
  replaceGuillemets: boolean;
  chunkStrategy: "Word Count Approx" | "Character Limit";
  chunkMinWords: number;
  chunkMaxWords: number;
  chunkMaxChars: number;
  chunkMaxCharsByLang: Record<string, number>;
  speed: number;
  referenceWavPath: string | null;
  referenceTranscript: string;
  epubPath: string | null;
  audioBookTitle: string;
  deleteIntermediateChunks: boolean;
  selectedChapters: Set<string>;
  demoOutputPath: string | null;
  testOutputPath: string | null;
  generateOutputPath: string | null;
  // Last generated audio, so the players survive panel switches.
  demoAudioPath: string | null;
  testAudioPath: string | null;
  engineVoices: import("./types").VoiceDescriptor[];
  engineSupportedLanguages: string[];
  engineVoiceCloning: boolean;
  qwenInstruct: string;
  outeSpeakerJsonPath: string | null;
  voxMode: "design" | "clone" | "ultimate";
  voxVoiceDescription: string;
  engineGeneration: Record<string, GenerationParam>;
  // User-edited advanced parameter values, keyed by input id
  // (e.g. "vox-temperature"). Persisted across panel switches; without
  // this, edits were stored on the DOM element and lost on re-render.
  engineParamOverrides: Record<string, string>;
}

export const state: AppState = {
  currentPanel: "generate",
  selectedEngineId: "Qwen3-TTS-12Hz-0.6B-CustomVoice",
  selectedLanguage: "Italian",
  selectedVoiceId: "Serena",
  selectedSeparator: ".",
  replaceGuillemets: false,
  chunkStrategy: "Character Limit",
  chunkMinWords: 100,
  chunkMaxWords: 500,
  chunkMaxChars: 800,
  chunkMaxCharsByLang: {},
  speed: 1.0,
  referenceWavPath: null,
  referenceTranscript: "",
  epubPath: null,
  audioBookTitle: "",
  deleteIntermediateChunks: false,
  selectedChapters: new Set(),
  demoOutputPath: null,
  testOutputPath: null,
  generateOutputPath: null,
  demoAudioPath: null,
  testAudioPath: null,
  engineVoices: [],
  engineSupportedLanguages: [],
  engineVoiceCloning: false,
  qwenInstruct: "",
  outeSpeakerJsonPath: null,
  voxMode: "design",
  voxVoiceDescription: "A calm middle-aged male narrator with a deep voice",
  engineGeneration: {},
  engineParamOverrides: {},
};

export const SEPARATOR_OPTIONS = [
  { value: ".", label: "Standard Period (.)" },
  { value: "|", label: "Pipe (|)" },
  { value: ";", label: "Semicolon (;)" },
  { value: "<sil>", label: "Silence Tag (<sil>)" },
  { value: "[PAUSE]", label: "Pause Tag ([PAUSE])" },
  { value: "_", label: "Underscore (_)" },
];

export const PANEL_TITLES: Record<PanelId, string> = {
  configuration: "Configuration",
  epub: "EPUB & Options",
  generate: "Generate",
  recovery: "Error Recovery",
  demo: "Demo & Test",
  models: "Models",
  agents: "AI Agents",
};

export const NAV_ITEMS: { id: PanelId; label: string }[] = [
  { id: "configuration", label: "Configuration" },
  { id: "epub", label: "EPUB & Options" },
  { id: "generate", label: "Generate" },
  { id: "recovery", label: "Error Recovery" },
  { id: "demo", label: "Demo & Test" },
  { id: "models", label: "Models" },
  { id: "agents", label: "AI Agents" },
];