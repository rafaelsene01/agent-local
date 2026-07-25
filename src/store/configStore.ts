import { create } from "zustand";
import { configApi } from "../lib/configApi";
import { applyLanguage } from "../i18n";
import { applyTheme } from "../lib/theme";
import type { AppConfig } from "../types";

interface ConfigState {
  config: AppConfig | null;
  status: "loading" | "needs-onboarding" | "ready";
  error: string | null;

  loadConfig: () => Promise<void>;
  completeOnboarding: (basePath: string, theme: string, language: string) => Promise<void>;
  setTheme: (theme: string) => Promise<void>;
  setLanguage: (language: string) => Promise<void>;
  setBasePath: (basePath: string) => Promise<void>;
}

export const useConfigStore = create<ConfigState>((set) => ({
  config: null,
  status: "loading",
  error: null,

  loadConfig: async () => {
    try {
      const config = await configApi.getConfig();
      if (config && config.onboarding_completed) {
        applyTheme(config.theme);
        applyLanguage(config.language);
        set({ config, status: "ready" });
      } else {
        set({ config: null, status: "needs-onboarding" });
      }
    } catch (err) {
      set({ error: String(err), status: "needs-onboarding" });
    }
  },

  completeOnboarding: async (basePath, theme, language) => {
    const config = await configApi.completeOnboarding(basePath, theme, language);
    applyTheme(config.theme);
    applyLanguage(config.language);
    set({ config, status: "ready" });
  },

  setTheme: async (theme) => {
    applyTheme(theme);
    const config = await configApi.updateTheme(theme);
    set({ config });
  },

  setLanguage: async (language) => {
    applyLanguage(language);
    const config = await configApi.updateLanguage(language);
    set({ config });
  },

  setBasePath: async (basePath) => {
    const config = await configApi.updateBasePath(basePath);
    set({ config });
  },
}));
