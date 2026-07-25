export interface Chat {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface Message {
  id: string;
  chat_id: string;
  role: "user" | "assistant" | "system";
  content: string;
  created_at: string;
}

export interface AppConfig {
  base_path: string;
  theme: string;
  language: string;
  onboarding_completed: boolean;
}

export type ConnectionProvider = "ollama" | "lmstudio" | "custom";
export type ConnectionStatus = "available" | "unavailable" | "unknown";

export interface Connection {
  id: string;
  provider: ConnectionProvider;
  base_url: string;
  enabled: boolean;
  status: ConnectionStatus;
}

export interface InstalledModel {
  name: string;
  size_bytes: number | null;
}

export interface DownloadableModel {
  id: string;
  display_name: string;
  provider: ConnectionProvider;
  pull_identifier: string;
  params_billions: number;
  default_quant: string;
  estimated_ram_gb: number;
  fits_ram: boolean;
}

export interface DownloadableModelsResponse {
  ram_detected_gb: number | null;
  models: DownloadableModel[];
}

export type PullStatus = "downloading" | "verifying" | "success" | "error";

export interface PullProgress {
  status: PullStatus;
  downloaded_bytes: number | null;
  total_bytes: number | null;
  message: string | null;
}

export interface ModelDownloadProgressEvent {
  connection_id: string;
  identifier: string;
  progress: PullProgress;
}

export interface ConfigApplied {
  context_length_applied: number | null;
  gpu_offload_applied: string | null;
  requires_reload: boolean;
  note: string | null;
}

export interface ActiveModel {
  connection_id: string;
  model_name: string;
  context_length: number | null;
  gpu_offload: string | null;
}
