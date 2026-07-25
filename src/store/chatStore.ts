import { create } from "zustand";
import { chatApi } from "../lib/chatApi";
import i18n from "../i18n";
import type { Chat, Message } from "../types";

interface ChatState {
  chats: Chat[];
  activeChatId: string | null;
  messages: Message[];
  isLoading: boolean;
  error: string | null;

  loadChats: () => Promise<void>;
  createChat: () => Promise<void>;
  selectChat: (id: string) => Promise<void>;
  renameChat: (id: string, title: string) => Promise<void>;
  deleteChat: (id: string) => Promise<void>;
}

export const useChatStore = create<ChatState>((set, get) => ({
  chats: [],
  activeChatId: null,
  messages: [],
  isLoading: false,
  error: null,

  loadChats: async () => {
    set({ isLoading: true, error: null });
    try {
      const chats = await chatApi.listChats();
      set({ chats, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  createChat: async () => {
    try {
      const chat = await chatApi.createChat(i18n.t("chats.defaultTitle"));
      await get().loadChats();
      set({ activeChatId: chat.id, messages: [] });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  selectChat: async (id: string) => {
    set({ activeChatId: id, error: null });
    try {
      const messages = await chatApi.listMessages(id);
      set({ messages });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  renameChat: async (id: string, title: string) => {
    try {
      await chatApi.renameChat(id, title);
      await get().loadChats();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  deleteChat: async (id: string) => {
    try {
      await chatApi.deleteChat(id);
      const wasActive = get().activeChatId === id;
      await get().loadChats();
      if (wasActive) {
        set({ activeChatId: null, messages: [] });
      }
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));
