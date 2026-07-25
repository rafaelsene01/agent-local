import { create } from "zustand";

export type ActiveView = "chat" | "settings";

interface UiState {
  activeView: ActiveView;
  setActiveView: (view: ActiveView) => void;
}

export const useUiStore = create<UiState>((set) => ({
  activeView: "chat",
  setActiveView: (view) => set({ activeView: view }),
}));
