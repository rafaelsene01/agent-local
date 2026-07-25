import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { connectionsApi } from "../lib/connectionsApi";
import type {
  ActiveModel,
  ConfigApplied,
  Connection,
  DownloadableModel,
  InstalledModel,
  ModelDownloadProgressEvent,
  PullProgress,
} from "../types";

function progressKey(connectionId: string, identifier: string) {
  return `${connectionId}:${identifier}`;
}

interface ConnectionsState {
  connections: Connection[];
  downloadableModels: DownloadableModel[];
  ramDetectedGb: number | null;
  installedModelsByConnection: Record<string, InstalledModel[]>;
  activeModel: ActiveModel | null;
  downloadProgress: Record<string, PullProgress>;
  isLoading: boolean;
  error: string | null;

  loadConnections: () => Promise<void>;
  addConnection: (provider: string, baseUrl: string) => Promise<void>;
  toggleConnection: (id: string, enabled: boolean) => Promise<void>;
  refreshConnectionStatus: (id: string) => Promise<void>;

  loadDownloadableModels: () => Promise<void>;
  loadInstalledModels: (connectionId: string) => Promise<void>;
  pullModel: (connectionId: string, identifier: string) => Promise<void>;
  loadActiveModel: () => Promise<void>;
  setActiveModel: (connectionId: string, modelName: string) => Promise<void>;
  configureModel: (
    connectionId: string,
    modelName: string,
    contextLength: number | null,
    gpuOffload: string | null,
  ) => Promise<ConfigApplied>;
}

export const useConnectionsStore = create<ConnectionsState>((set, get) => ({
  connections: [],
  downloadableModels: [],
  ramDetectedGb: null,
  installedModelsByConnection: {},
  activeModel: null,
  downloadProgress: {},
  isLoading: false,
  error: null,

  loadConnections: async () => {
    set({ isLoading: true, error: null });
    try {
      const connections = await connectionsApi.listConnections();
      set({ connections, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  addConnection: async (provider, baseUrl) => {
    try {
      await connectionsApi.addConnection(provider, baseUrl);
      await get().loadConnections();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  toggleConnection: async (id, enabled) => {
    try {
      await connectionsApi.toggleConnection(id, enabled);
      await get().loadConnections();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  refreshConnectionStatus: async (id) => {
    try {
      const status = await connectionsApi.refreshConnectionStatus(id);
      set({
        connections: get().connections.map((c) => (c.id === id ? { ...c, status } : c)),
      });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadDownloadableModels: async () => {
    try {
      const { ram_detected_gb, models } = await connectionsApi.listDownloadableModels();
      set({ ramDetectedGb: ram_detected_gb, downloadableModels: models });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadInstalledModels: async (connectionId) => {
    try {
      const models = await connectionsApi.listInstalledModels(connectionId);
      set({
        installedModelsByConnection: {
          ...get().installedModelsByConnection,
          [connectionId]: models,
        },
      });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  pullModel: async (connectionId, identifier) => {
    try {
      await connectionsApi.pullModel(connectionId, identifier);
      await get().loadInstalledModels(connectionId);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadActiveModel: async () => {
    try {
      const activeModel = await connectionsApi.getActiveModel();
      set({ activeModel });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  setActiveModel: async (connectionId, modelName) => {
    try {
      await connectionsApi.setActiveModel(connectionId, modelName);
      await get().loadActiveModel();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  configureModel: async (connectionId, modelName, contextLength, gpuOffload) => {
    const applied = await connectionsApi.configureModel(
      connectionId,
      modelName,
      contextLength,
      gpuOffload,
    );
    return applied;
  },
}));

listen<ModelDownloadProgressEvent>("model-download-progress", (event) => {
  const { connection_id, identifier, progress } = event.payload;
  useConnectionsStore.setState((state) => ({
    downloadProgress: {
      ...state.downloadProgress,
      [progressKey(connection_id, identifier)]: progress,
    },
  }));
});
