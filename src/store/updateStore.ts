import { create } from "zustand";
import { updateApi } from "../lib/updateApi";
import type { UpdateInfo, UpdateProgress, UpdateSettings } from "../types";

/** Enough for the window to be usable before a network call goes out. */
const BOOT_CHECK_DELAY_MS = 5000;

interface UpdateState {
  available: UpdateInfo | null;
  settings: UpdateSettings | null;
  progress: UpdateProgress | null;
  installing: boolean;
  /** Result of an explicit "check now"; the boot check never sets this. */
  checking: boolean;
  checkedManually: boolean;
  error: string | null;
  dismissed: boolean;

  init: () => Promise<() => void>;
  checkOnBoot: () => void;
  checkNow: () => Promise<void>;
  install: () => Promise<void>;
  skip: () => Promise<void>;
  dismiss: () => void;
  setAutoCheck: (enabled: boolean) => Promise<void>;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  available: null,
  settings: null,
  progress: null,
  installing: false,
  checking: false,
  checkedManually: false,
  error: null,
  dismissed: false,

  init: async () => {
    const unlisten = await updateApi.onProgress((progress) => set({ progress }));
    try {
      set({ settings: await updateApi.getSettings() });
    } catch {
      // Settings are cosmetic here; a failure must not stop the app from
      // rendering.
    }
    return unlisten;
  },

  checkOnBoot: () => {
    window.setTimeout(async () => {
      const { settings } = get();
      if (!settings?.auto_check) return;
      try {
        const available = await updateApi.check();
        if (available) set({ available, dismissed: false });
      } catch {
        // Silent by design: being offline is the normal state for this app, and
        // an error toast on every launch would be noise. "Check now" is loud.
      }
    }, BOOT_CHECK_DELAY_MS);
  },

  checkNow: async () => {
    set({ checking: true, error: null, checkedManually: false });
    try {
      const available = await updateApi.check();
      set({ available, dismissed: false, checkedManually: true });
    } catch (err) {
      set({ error: String(err) });
    } finally {
      set({ checking: false });
    }
  },

  install: async () => {
    set({ installing: true, error: null, progress: null });
    try {
      // Never resolves on success — the backend restarts or exits.
      await updateApi.install();
    } catch (err) {
      set({ error: String(err), installing: false, progress: null });
    }
  },

  skip: async () => {
    const version = get().available?.version;
    if (!version) return;
    try {
      await updateApi.skipVersion(version);
      set({ available: null, settings: await updateApi.getSettings() });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  dismiss: () => set({ dismissed: true }),

  setAutoCheck: async (enabled) => {
    await updateApi.setAutoCheck(enabled);
    set({ settings: await updateApi.getSettings() });
  },
}));
