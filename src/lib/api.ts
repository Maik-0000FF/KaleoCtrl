import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, KeywordConfig, SttStatus, AppMode } from "./types";

export async function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export async function updateConfig(config: AppConfig): Promise<void> {
  return invoke("update_config", { config });
}

export async function getKeywords(language?: string): Promise<KeywordConfig> {
  return invoke<KeywordConfig>("get_keywords", { language: language ?? null });
}

export async function saveKeywords(keywords: KeywordConfig): Promise<void> {
  return invoke("save_keywords", { keywords });
}

export async function getAvailableLanguages(): Promise<string[]> {
  return invoke<string[]>("get_available_languages");
}

export async function loadSttModel(modelPath: string): Promise<void> {
  return invoke("load_stt_model", { modelPath });
}

export async function unloadSttModel(): Promise<void> {
  return invoke("unload_stt_model");
}

export async function getSttStatus(): Promise<SttStatus> {
  return invoke<SttStatus>("get_stt_status");
}

export async function startListening(): Promise<void> {
  return invoke("start_listening");
}

export async function stopListening(): Promise<void> {
  return invoke("stop_listening");
}

export async function getListeningStatus(): Promise<boolean> {
  return invoke<boolean>("get_listening_status");
}

export async function getMode(): Promise<AppMode> {
  return invoke<AppMode>("get_mode");
}

export async function setMode(mode: AppMode): Promise<AppMode> {
  return invoke<AppMode>("set_mode", { mode });
}

export async function toggleOverlay(): Promise<void> {
  return invoke("toggle_overlay");
}
