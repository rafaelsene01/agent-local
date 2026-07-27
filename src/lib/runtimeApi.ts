// SPEC: self-contained-runtime (SELF-01)

import { invoke } from "@tauri-apps/api/core";
import type {
  ActiveModel,
  DownloadableModelsResponse,
  InstalledModel,
  ModelLimits,
  RuntimeStatus,
} from "../types";

/** One runtime, so nothing here takes a connection to disambiguate. Every
 *  function below maps to a command registered in `lib.rs`; adding one without
 *  its Rust side fails at runtime, not at build time (`invoke` takes a
 *  string), which is why the two lists are kept in the same order. */
export const runtimeApi = {
  prepareRuntime: () => invoke<RuntimeStatus>("prepare_runtime"),
  startRuntime: () => invoke<RuntimeStatus>("start_runtime"),
  stopRuntime: () => invoke<void>("stop_runtime"),
  runtimeStatus: () => invoke<RuntimeStatus>("runtime_status"),

  listDownloadableModels: () => invoke<DownloadableModelsResponse>("list_downloadable_models"),
  listInstalledModels: () => invoke<InstalledModel[]>("list_installed_models"),
  downloadModel: (url: string) => invoke<void>("download_model", { url }),

  getActiveModel: () => invoke<ActiveModel | null>("get_active_model"),
  setActiveModel: (modelName: string) => invoke<void>("set_active_model", { modelName }),
  modelLimits: (model: string) => invoke<ModelLimits>("model_limits", { model }),
  configureModel: (contextLength: number | null, gpuOffload: string | null) =>
    invoke<void>("configure_model", { contextLength, gpuOffload }),
};
