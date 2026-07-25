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

/** Mirrors `DocumentStatus` in rag/pipeline.rs. Everything before `ready` is
 *  a processing step; only `ready` documents are searchable. */
export type DocumentStatus =
  | "queued"
  | "parsing"
  | "chunking"
  | "embedding"
  | "ready"
  | "error";

export interface DocumentRecord {
  id: string;
  filename: string;
  file_path: string;
  size_bytes: number;
  status: DocumentStatus;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export interface DocumentStatusEvent {
  id: string;
  status: DocumentStatus;
  error_message: string | null;
}

export type ConnectionProvider = "ollama" | "lmstudio" | "custom" | "embedded";
export type ConnectionStatus = "available" | "unavailable" | "unknown";

export interface Connection {
  id: string;
  provider: ConnectionProvider;
  base_url: string;
  is_active: boolean;
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

/** Mirrors `EmbeddedSetupStage` in embedded_commands.rs — the mapping is
 *  manual on purpose (no codegen in this project, see C-03). */
export type EmbeddedSetupStage =
  | "unsupported"
  | "not_installed"
  | "downloading_binary"
  | "downloading_model"
  | "ready"
  | "running"
  | "error";

export interface EmbeddedRuntimeStatus {
  stage: EmbeddedSetupStage;
  release_tag: string | null;
  backend: "vulkan" | "cpu" | null;
  port: number | null;
  model_name: string | null;
  message: string | null;
}

export interface EmbeddedSetupProgressEvent {
  stage: EmbeddedSetupStage;
  progress: PullProgress | null;
  message: string | null;
}

/** Who answers right now. `model` can be null while `connection` is set:
 *  a connection was activated but no model picked yet. */
export interface ActivePair {
  connection: Connection | null;
  model: ActiveModel | null;
}
