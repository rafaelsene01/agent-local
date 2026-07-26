import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UpdateInfo, UpdateProgress, UpdateSettings } from "../types";

/** Matches `portable::UPDATE_PROGRESS_EVENT`. */
const PROGRESS_EVENT = "update-download-progress";

export const updateApi = {
  check: () => invoke<UpdateInfo | null>("check_for_update"),

  /**
   * Resolves only on failure. On success the backend either restarts the app
   * (installed) or spawns the new executable and exits (portable), so the
   * promise never settles — the caller must not wait for it to show progress.
   */
  install: () => invoke<void>("install_update"),

  skipVersion: (version: string) => invoke<void>("skip_update_version", { version }),
  getSettings: () => invoke<UpdateSettings>("get_update_settings"),
  setAutoCheck: (enabled: boolean) => invoke<void>("set_auto_update_check", { enabled }),

  onProgress: (callback: (progress: UpdateProgress) => void) =>
    listen<UpdateProgress>(PROGRESS_EVENT, (event) => callback(event.payload)),
};
