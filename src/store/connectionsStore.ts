import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { connectionsApi } from "../lib/connectionsApi";
import type {
  ActivePair,
  ConfigApplied,
  Connection,
  DownloadableModel,
  EmbeddedRuntimeStatus,
  EmbeddedSetupProgressEvent,
  InstalledModel,
  ModelDownloadProgressEvent,
  PullProgress,
} from "../types";

const NO_ACTIVE_PAIR: ActivePair = { connection: null, model: null };

function progressKey(connectionId: string, identifier: string) {
  return `${connectionId}:${identifier}`;
}

interface ConnectionsState {
  connections: Connection[];
  downloadableModels: DownloadableModel[];
  ramDetectedGb: number | null;
  installedModelsByConnection: Record<string, InstalledModel[]>;
  activePair: ActivePair;
  downloadProgress: Record<string, PullProgress>;
  isLoading: boolean;
  error: string | null;

  loadConnections: () => Promise<void>;
  addConnection: (provider: string, baseUrl: string) => Promise<void>;
  setActiveConnection: (id: string) => Promise<void>;
  clearActiveConnection: () => Promise<void>;
  refreshConnectionStatus: (id: string) => Promise<void>;

  loadDownloadableModels: () => Promise<void>;
  loadInstalledModels: (connectionId: string) => Promise<void>;
  loadAvailableInstalledModels: () => Promise<void>;
  pullModel: (connectionId: string, identifier: string) => Promise<void>;
  embeddedStatus: EmbeddedRuntimeStatus | null;
  embeddedProgress: EmbeddedSetupProgressEvent | null;
  isSettingUpEmbedded: boolean;
  loadEmbeddedStatus: () => Promise<void>;
  setupEmbeddedRuntime: () => Promise<void>;
  startEmbeddedRuntime: () => Promise<void>;
  stopEmbeddedRuntime: () => Promise<void>;
  downloadEmbeddedModel: (url: string) => Promise<void>;

  loadActivePair: () => Promise<void>;
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
  activePair: NO_ACTIVE_PAIR,
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

  // Activating a connection can drop an active model that belonged to
  // another one (ACTIVE-06), so the pair is always re-read from the backend
  // instead of guessed here.
  setActiveConnection: async (id) => {
    try {
      await connectionsApi.setActiveConnection(id);
      await Promise.all([get().loadConnections(), get().loadActivePair()]);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  clearActiveConnection: async () => {
    try {
      await connectionsApi.clearActiveConnection();
      await Promise.all([get().loadConnections(), get().loadActivePair()]);
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

  // ACTIVE-08: models of every reachable connection are inspectable, not
  // just the active one's, so the user can compare before switching.
  loadAvailableInstalledModels: async () => {
    const available = get().connections.filter((c) => c.status === "available");
    await Promise.all(available.map((c) => get().loadInstalledModels(c.id)));
  },

  pullModel: async (connectionId, identifier) => {
    try {
      await connectionsApi.pullModel(connectionId, identifier);
      await get().loadInstalledModels(connectionId);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  embeddedStatus: null,
  embeddedProgress: null,
  isSettingUpEmbedded: false,

  loadEmbeddedStatus: async () => {
    try {
      const embeddedStatus = await connectionsApi.embeddedRuntimeStatus();
      set({ embeddedStatus });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  // The whole setup (binary + ~2.4GB model) is one long call; progress
  // arrives through the `embedded-setup-progress` listener below, not here.
  setupEmbeddedRuntime: async () => {
    set({ isSettingUpEmbedded: true, error: null });
    try {
      const embeddedStatus = await connectionsApi.setupEmbeddedRuntime();
      set({ embeddedStatus });
    } catch (err) {
      set({ error: String(err) });
    } finally {
      set({ isSettingUpEmbedded: false });
    }
  },

  startEmbeddedRuntime: async () => {
    try {
      const embeddedStatus = await connectionsApi.startEmbeddedRuntime();
      set({ embeddedStatus });
      await get().loadConnections();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  stopEmbeddedRuntime: async () => {
    try {
      await connectionsApi.stopEmbeddedRuntime();
      await Promise.all([get().loadEmbeddedStatus(), get().loadConnections()]);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  downloadEmbeddedModel: async (url) => {
    try {
      await connectionsApi.downloadEmbeddedModel(url);
      await get().loadAvailableInstalledModels();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadActivePair: async () => {
    try {
      const activePair = await connectionsApi.getActivePair();
      set({ activePair });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  // Picking a model also activates its connection in the backend, so the
  // connection list has to be re-read alongside the pair.
  setActiveModel: async (connectionId, modelName) => {
    try {
      await connectionsApi.setActiveModel(connectionId, modelName);
      await Promise.all([get().loadConnections(), get().loadActivePair()]);
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

// The embedded sidecar takes seconds to load its model, so the status read at
// boot is stale by the time it answers.
listen("connections-changed", () => {
  const store = useConnectionsStore.getState();
  store.loadConnections();
  store.loadEmbeddedStatus();
});

listen<EmbeddedSetupProgressEvent>("embedded-setup-progress", (event) => {
  useConnectionsStore.setState({ embeddedProgress: event.payload });
});

listen<ModelDownloadProgressEvent>("model-download-progress", (event) => {
  const { connection_id, identifier, progress } = event.payload;
  useConnectionsStore.setState((state) => ({
    downloadProgress: {
      ...state.downloadProgress,
      [progressKey(connection_id, identifier)]: progress,
    },
  }));
});
