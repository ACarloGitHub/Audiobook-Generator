export type PanelId =
  | "configuration"
  | "epub"
  | "generate"
  | "recovery"
  | "demo"
  | "models";

export interface EngineInfo {
  id: string;
  display_name: string;
  format: "ONNX" | "GGUF" | "Safetensors";
  voice_cloning: boolean;
  hardware: string[];
  license: string;
  languages: string[];
  installed: boolean;
  size_mb: number;
  voices?: VoiceDescriptor[];
}

export interface EngineStatus {
  active_engine: string | null;
  active_model: string | null;
  vram_bytes: number | null;
  loaded_at: string | null;
  engines: EngineInfo[];
  hardware: HardwareSummary;
}

export interface HardwareSummary {
  os: string;
  arch: string;
  gpus: GpuInfo[];
}

export interface GpuInfo {
  vendor: string;
  model: string;
  vram_bytes: number;
  backend: string;
}

export interface VoiceDescriptor {
  id: string;
  display_name: string;
  language: string;
}

export interface EngineDefaults {
  engine_id: string;
  chunk_strategy: string;
  chunk_min_words: number | null;
  chunk_max_words: number | null;
  chunk_max_chars: number;
  chunk_max_chars_by_lang: Record<string, number>;
  separator: string;
  replace_guillemets: boolean;
  voice_cloning: boolean;
  needs_reference_transcript: boolean;
  supported_languages: string[];
  voices: VoiceDescriptor[];
}

export interface ChapterSummary {
  title: string;
  char_count: number;
}

export interface BookInfo {
  title: string;
  chapters: ChapterSummary[];
}

export interface BookErrorSummary {
  book_title: string;
  book_dir: string;
  chapters_with_errors: ChapterErrorSummary[];
}

export interface ChapterErrorSummary {
  title: string;
  failed_chunks: number;
  total_chunks: number;
}

export interface FailedChunkInfo {
  chapter: string;
  chunk_index: number;
  text: string;
  error: string;
}

export interface ModelListEntry {
  name: string;
  engine_id: string;
  format: string;
  license: string;
  size_mb: number;
  installed: boolean;
  essential_present: boolean;
  dest: string;
  supported: boolean;
  note: string | null;
}