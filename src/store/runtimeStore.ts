// SPEC: self-contained-runtime (SELF-01)

import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { runtimeApi } from "../lib/runtimeApi";
import type {
  ActiveModel,
  DownloadableModel,
  InstalledModel,
  ModelDownloadProgressEvent,
  PullProgress,
  RuntimeProgressEvent,
  RuntimeStatus,
} from "../types";

interface RuntimeState {
  status: RuntimeStatus | null;
  progress: RuntimeProgressEvent | null;
  isPreparing: boolean;

  installedModels: InstalledModel[];
  downloadableModels: DownloadableModel[];
  ramDetectedGb: number | null;
  /** Keyed by the `.gguf` URL, which is what the backend echoes back. */
  downloadProgress: Record<string, PullProgress>;

  activeModel: ActiveModel | null;
  error: string | null;

  loadStatus: () => Promise<void>;
  prepareRuntime: () => Promise<void>;
  startRuntime: () => Promise<void>;
  stopRuntime: () => Promise<void>;

  loadInstalledModels: () => Promise<void>;
  loadDownloadableModels: () => Promise<void>;
  downloadModel: (url: string) => Promise<void>;

  loadActiveModel: () => Promise<void>;
  setActiveModel: (modelName: string) => Promise<void>;
  configureModel: (contextLength: number | null, gpuOffload: string | null) => Promise<void>;
}

export const useRuntimeStore = create<RuntimeState>((set, get) => ({
  status: null,
  progress: null,
  isPreparing: false,
  installedModels: [],
  downloadableModels: [],
  ramDetectedGb: null,
  downloadProgress: {},
  activeModel: null,
  error: null,

  loadStatus: async () => {
    try {
      const status = await runtimeApi.runtimeStatus();
      set({ status });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  // Preparing no longer downloads a model, so this is short. The progress
  // listener below still exists because picking the backend can involve
  // fetching the engine until the components ship in the installer.
  prepareRuntime: async () => {
    set({ isPreparing: true, error: null });
    try {
      const status = await runtimeApi.prepareRuntime();
      set({ status });
    } catch (err) {
      set({ error: String(err) });
    } finally {
      set({ isPreparing: false });
    }
  },

  startRuntime: async () => {
    try {
      const status = await runtimeApi.startRuntime();
      set({ status, error: null });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  stopRuntime: async () => {
    try {
      await runtimeApi.stopRuntime();
      await get().loadStatus();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadInstalledModels: async () => {
    try {
      set({ installedModels: await runtimeApi.listInstalledModels() });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadDownloadableModels: async () => {
    try {
      const { ram_detected_gb, models } = await runtimeApi.listDownloadableModels();
      set({ ramDetectedGb: ram_detected_gb, downloadableModels: models });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  downloadModel: async (url) => {
    try {
      await runtimeApi.downloadModel(url);
      await get().loadInstalledModels();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadActiveModel: async () => {
    try {
      set({ activeModel: await runtimeApi.getActiveModel() });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  // Choosing the first model also starts the sidecar in the backend, so the
  // runtime status is re-read alongside the choice rather than assumed.
  setActiveModel: async (modelName) => {
    try {
      await runtimeApi.setActiveModel(modelName);
      set({ error: null });
    } catch (err) {
      set({ error: String(err) });
    }
    await Promise.all([get().loadActiveModel(), get().loadStatus()]);
  },

  configureModel: async (contextLength, gpuOffload) => {
    // Context and GPU offload are start-up flags: applying them restarts the
    // sidecar, so both the active model and the status can change here.
    await runtimeApi.configureModel(contextLength, gpuOffload);
    await Promise.all([get().loadActiveModel(), get().loadStatus()]);
  },
}));

// The sidecar takes seconds to load its model, so the status read at boot is
// stale by the time it answers. The backend emits this once it is really up.
listen("runtime-changed", () => {
  const store = useRuntimeStore.getState();
  store.loadStatus();
  store.loadActiveModel();
});

listen<RuntimeProgressEvent>("runtime-progress", (event) => {
  useRuntimeStore.setState({ progress: event.payload });
});

listen<ModelDownloadProgressEvent>("model-download-progress", (event) => {
  const { identifier, progress } = event.payload;
  useRuntimeStore.setState((state) => ({
    downloadProgress: { ...state.downloadProgress, [identifier]: progress },
  }));
});
