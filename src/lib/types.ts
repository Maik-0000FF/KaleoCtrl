export interface AppConfig {
  assistant_name: string;
  language: string;
  stt_model: string;
  default_mode: string;
}

export interface ModeInfo {
  name: string;
  description: string;
}

export interface KeywordConfig {
  language: string;
  modes: Record<string, ModeInfo>;
  mode_switch: string;
  wake_phrase: string;
  sleep_phrase: string;
  commands: Record<string, string>;
  dictation: Record<string, string>;
}

export interface SttStatus {
  engine: string;
  model_loaded: boolean;
  current_model: string | null;
}

export type AppMode = "desktop" | "dictation" | "terminal" | "sleep";

export interface TranscriptionResult {
  text: string;
  language: string | null;
  duration_ms: number;
}
