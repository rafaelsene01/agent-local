export interface Chat {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  /** Whether this chat also searches the global knowledge base (CHAT-14). */
  use_global_rag: boolean;
}

/** Terminal states: `injected_whole` (small file put in the prompt verbatim),
 *  `ready` (indexed for retrieval) and `error`. */
export type ChatAttachmentStatus = "queued" | "injected_whole" | "ready" | "error";

export interface ChatAttachment {
  id: string;
  filename: string;
  status: ChatAttachmentStatus;
  error_message: string | null;
  created_at: string;
}

export interface Message {
  id: string;
  chat_id: string;
  role: "user" | "assistant" | "system";
  content: string;
  created_at: string;
}

export interface ChatStreamChunk {
  chat_id: string;
  message_id: string;
  delta: string;
  done: boolean;
  error: string | null;
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

export interface RejectedImport {
  path: string;
  reason: string;
}

/** A selection can be partly valid: the good files are imported and the bad
 *  ones come back named, instead of the whole batch failing. */
export interface ImportResult {
  imported: DocumentRecord[];
  rejected: RejectedImport[];
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
  /** Exact download size, known for the embedded runtime's GGUF files. */
  download_bytes: number | null;
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

/** What the provider says about a model's context window. Both are null when
 *  the provider can't report it (a plain OpenAI-compatible server). */
export interface ModelLimits {
  /** The window the model was trained for — the ceiling for the config field. */
  max_context: number | null;
  /** What the runtime has allocated right now; can be smaller than the max. */
  current_context: number | null;
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
