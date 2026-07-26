import { invoke } from "@tauri-apps/api/core";
import type {
  ActivePair,
  ConfigApplied,
  Connection,
  ConnectionStatus,
  DownloadableModelsResponse,
  EmbeddedRuntimeStatus,
  InstalledModel,
  ModelLimits,
} from "../types";

export const connectionsApi = {
  listConnections: () => invoke<Connection[]>("list_connections"),
  addConnection: (provider: string, baseUrl: string) =>
    invoke<Connection>("add_connection", { provider, baseUrl }),
  setActiveConnection: (id: string) => invoke<void>("set_active_connection", { id }),
  clearActiveConnection: () => invoke<void>("clear_active_connection"),
  refreshConnectionStatus: (id: string) =>
    invoke<ConnectionStatus>("refresh_connection_status", { id }),

  listDownloadableModels: () => invoke<DownloadableModelsResponse>("list_downloadable_models"),
  listInstalledModels: (connectionId: string) =>
    invoke<InstalledModel[]>("list_installed_models", { connectionId }),
  pullModel: (connectionId: string, identifier: string) =>
    invoke<void>("pull_model", { connectionId, identifier }),
  setActiveModel: (connectionId: string, modelName: string) =>
    invoke<void>("set_active_model", { connectionId, modelName }),
  getActivePair: () => invoke<ActivePair>("get_active_pair"),
  setupEmbeddedRuntime: () => invoke<EmbeddedRuntimeStatus>("setup_embedded_runtime"),
  startEmbeddedRuntime: () => invoke<EmbeddedRuntimeStatus>("start_embedded_runtime"),
  stopEmbeddedRuntime: () => invoke<void>("stop_embedded_runtime"),
  embeddedRuntimeStatus: () => invoke<EmbeddedRuntimeStatus>("embedded_runtime_status"),
  downloadEmbeddedModel: (url: string) => invoke<void>("download_embedded_model", { url }),

  modelLimits: (connectionId: string, modelName: string) =>
    invoke<ModelLimits>("model_limits", { connectionId, modelName }),

  configureModel: (
    connectionId: string,
    modelName: string,
    contextLength: number | null,
    gpuOffload: string | null,
  ) =>
    invoke<ConfigApplied>("configure_model", {
      connectionId,
      modelName,
      contextLength,
      gpuOffload,
    }),
};
