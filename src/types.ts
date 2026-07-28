// SPEC: app-shell (SHELL-04), chat-messaging (CHAT-06, CHAT-14),
//       self-contained-runtime (SELF-01), conversation-memory (MEM-14, MEM-18)

export interface Chat {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  /** Whether this chat also searches the global knowledge base (CHAT-14). */
  use_global_rag: boolean;
  /** Whether completed turns are remembered and recalled later (MEM-14). */
  use_memory: boolean;
}

/** Progress of an on-demand history indexing run (MEM-18). */
export interface MemoryBackfillProgress {
  chat_id: string;
  done: number;
  total: number;
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

/** Mirrors `ChatRetrievalWarning` in chat_commands.rs. The answer still came;
 *  it just came without the knowledge base. */
export interface ChatRetrievalWarning {
  chat_id: string;
  reason: string;
}

export interface AppConfig {
  base_path: string;
  theme: string;
  language: string;
  onboarding_completed: boolean;
  auto_update_check: boolean;
  skipped_version: string | null;
}

/** Mirrors `StorageStatus` in config.rs. `configured && !ready` is the folder
 *  that vanished between sessions: the wizard reopens with a warning naming
 *  `base_path`, instead of the app booting with every command broken. */
export interface StorageStatus {
  configured: boolean;
  ready: boolean;
  base_path: string;
}

/** Mirrors `InstallFlavor` in update/mod.rs. Decided by a marker file next to
 *  the executable, never by inspecting the install path. */
export type InstallFlavor = "installed" | "portable";

export interface UpdateInfo {
  version: string;
  current_version: string;
  notes: string | null;
  pub_date: string | null;
  flavor: InstallFlavor;
}

export interface UpdateSettings {
  current_version: string;
  auto_check: boolean;
  flavor: InstallFlavor;
  skipped_version: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
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

export interface InstalledModel {
  name: string;
  size_bytes: number | null;
}

export interface DownloadableModel {
  id: string;
  display_name: string;
  /** The direct `.gguf` URL — there is no registry to pull by name (SELF-02). */
  pull_identifier: string;
  params_billions: number;
  default_quant: string;
  estimated_ram_gb: number;
  /** Exact download size, checked against the server when the entry was added. */
  download_bytes: number | null;
  fits_ram: boolean;
}

export interface DownloadableModelsResponse {
  ram_detected_gb: number | null;
  models: DownloadableModel[];
}

// Mirrors `providers::PullStatus`. "verifying" was Ollama's checksum phase and
// was removed on the Rust side (C-11); a value that the backend can no longer
// send has no place here either.
export type PullStatus = "downloading" | "success" | "error";

export interface PullProgress {
  status: PullStatus;
  downloaded_bytes: number | null;
  total_bytes: number | null;
  message: string | null;
}

/** Keyed by the URL that was asked for, so the card that started a download is
 *  the one that shows its bar. */
export interface ModelDownloadProgressEvent {
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

/** Mirrors `store::ActiveModel` in runtime/store.rs. `null` from the backend
 *  covers both "never chose one" and "the file is gone" — the two cases the
 *  user fixes the same way. */
export interface ActiveModel {
  name: string;
  path: string;
  context_length: number | null;
  gpu_layers: number | null;
}

/** Mirrors `RuntimeStage` in runtime_commands.rs — the mapping is manual on
 *  purpose (no codegen in this project, see C-03).
 *
 *  `no_model` is a first-class state, not an error: preparing the runtime
 *  downloads nothing, so a fresh install is prepared and modelless. */
export type RuntimeStage =
  | "unsupported"
  | "not_prepared"
  | "preparing"
  | "no_model"
  | "ready"
  | "running";

export interface RuntimeStatus {
  stage: RuntimeStage;
  release_tag: string | null;
  backend: "vulkan" | "cpu" | null;
  port: number | null;
  model_name: string | null;
  message: string | null;
}

export interface RuntimeProgressEvent {
  stage: RuntimeStage;
  progress: PullProgress | null;
  message: string | null;
}
