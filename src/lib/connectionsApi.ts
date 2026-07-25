import { invoke } from "@tauri-apps/api/core";
import type {
  ActiveModel,
  ConfigApplied,
  Connection,
  ConnectionStatus,
  DownloadableModelsResponse,
  InstalledModel,
} from "../types";

export const connectionsApi = {
  listConnections: () => invoke<Connection[]>("list_connections"),
  addConnection: (provider: string, baseUrl: string) =>
    invoke<Connection>("add_connection", { provider, baseUrl }),
  toggleConnection: (id: string, enabled: boolean) =>
    invoke<void>("toggle_connection", { id, enabled }),
  refreshConnectionStatus: (id: string) =>
    invoke<ConnectionStatus>("refresh_connection_status", { id }),

  listDownloadableModels: () => invoke<DownloadableModelsResponse>("list_downloadable_models"),
  listInstalledModels: (connectionId: string) =>
    invoke<InstalledModel[]>("list_installed_models", { connectionId }),
  pullModel: (connectionId: string, identifier: string) =>
    invoke<void>("pull_model", { connectionId, identifier }),
  setActiveModel: (connectionId: string, modelName: string) =>
    invoke<void>("set_active_model", { connectionId, modelName }),
  getActiveModel: () => invoke<ActiveModel | null>("get_active_model"),
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
