import { invoke } from "@tauri-apps/api/core";
import type { AppConfig } from "../types";

export const configApi = {
  getConfig: () => invoke<AppConfig | null>("get_app_config"),
  getDefaultBasePath: () => invoke<string>("get_default_base_path"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  completeOnboarding: (base_path: string, theme: string, language: string) =>
    invoke<AppConfig>("complete_onboarding", { basePath: base_path, theme, language }),
  updateTheme: (theme: string) => invoke<AppConfig>("update_theme", { theme }),
  updateLanguage: (language: string) => invoke<AppConfig>("update_language", { language }),
  updateBasePath: (newBasePath: string) =>
    invoke<AppConfig>("update_base_path", { newBasePath }),
};
