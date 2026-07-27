import { create } from "zustand";

// SPEC: self-contained-runtime (SELF-01)

export type ActiveView = "chat" | "settings" | "runtime" | "documents";

interface UiState {
  activeView: ActiveView;
  setActiveView: (view: ActiveView) => void;
}

export const useUiStore = create<UiState>((set) => ({
  activeView: "chat",
  setActiveView: (view) => set({ activeView: view }),
}));
