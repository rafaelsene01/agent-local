import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { chatApi } from "../lib/chatApi";
import i18n from "../i18n";
import type { Chat, ChatStreamChunk, Message } from "../types";

interface ChatState {
  chats: Chat[];
  activeChatId: string | null;
  messages: Message[];
  /** Text accumulated from stream events, not yet persisted as a Message. */
  streamingContent: string;
  isGenerating: boolean;
  isLoading: boolean;
  error: string | null;

  loadChats: () => Promise<void>;
  createChat: () => Promise<void>;
  selectChat: (id: string) => Promise<void>;
  renameChat: (id: string, title: string) => Promise<void>;
  deleteChat: (id: string) => Promise<void>;
  sendMessage: (content: string, attachmentPaths: string[]) => Promise<void>;
  cancelGeneration: () => Promise<void>;
}

export const useChatStore = create<ChatState>((set, get) => ({
  chats: [],
  activeChatId: null,
  messages: [],
  streamingContent: "",
  isGenerating: false,
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
      set({ activeChatId: chat.id, messages: [], streamingContent: "" });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  selectChat: async (id: string) => {
    set({ activeChatId: id, error: null, streamingContent: "" });
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

  // The command resolves only when generation ends; tokens arrive meanwhile
  // through the listener below, so the message list is reloaded at the end to
  // pick up what the backend persisted.
  sendMessage: async (content, attachmentPaths) => {
    const chatId = get().activeChatId;
    if (!chatId) return;
    set({ isGenerating: true, error: null, streamingContent: "" });
    try {
      await chatApi.sendMessage(chatId, content, attachmentPaths);
    } catch (err) {
      set({ error: String(err) });
    } finally {
      set({ isGenerating: false, streamingContent: "" });
      const messages = await chatApi.listMessages(chatId);
      set({ messages });
      await get().loadChats();
    }
  },

  cancelGeneration: async () => {
    const chatId = get().activeChatId;
    if (!chatId) return;
    try {
      await chatApi.cancelGeneration(chatId);
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));

listen<ChatStreamChunk>("chat-stream-chunk", (event) => {
  const { chat_id, delta, done, error } = event.payload;
  const state = useChatStore.getState();
  // Chunks for a chat the user has navigated away from are dropped rather
  // than appended to whatever is on screen now.
  if (state.activeChatId !== chat_id) return;

  if (error) {
    useChatStore.setState({ error, isGenerating: false });
    return;
  }
  if (done) {
    useChatStore.setState({ isGenerating: false });
    return;
  }
  useChatStore.setState({ streamingContent: state.streamingContent + delta });
});
