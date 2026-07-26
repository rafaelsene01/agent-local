import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { chatApi } from "../lib/chatApi";
import i18n from "../i18n";
import type {
  Chat,
  ChatAttachment,
  ChatRetrievalWarning,
  ChatStreamChunk,
  Message,
} from "../types";

interface ChatState {
  chats: Chat[];
  activeChatId: string | null;
  messages: Message[];
  attachments: ChatAttachment[];
  /** Text accumulated from stream events, not yet persisted as a Message. */
  streamingContent: string;
  /** Which chat the accumulated text belongs to — generation keeps running
   *  after the user switches away, so it can't be assumed to be the active one. */
  streamingChatId: string | null;
  generatingChatId: string | null;
  isLoading: boolean;
  error: string | null;
  /** Set when an answer was produced without the knowledge base because
   *  retrieval failed. Distinct from `error`: the message itself worked. */
  retrievalWarning: string | null;

  loadChats: () => Promise<void>;
  createChat: () => Promise<void>;
  selectChat: (id: string) => Promise<void>;
  renameChat: (id: string, title: string) => Promise<void>;
  deleteChat: (id: string) => Promise<void>;
  setUseGlobalRag: (id: string, enabled: boolean) => Promise<void>;
  sendMessage: (content: string, attachmentPaths: string[]) => Promise<void>;
  cancelGeneration: () => Promise<void>;
  dismissRetrievalWarning: () => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  chats: [],
  activeChatId: null,
  messages: [],
  attachments: [],
  streamingContent: "",
  streamingChatId: null,
  generatingChatId: null,
  isLoading: false,
  error: null,
  retrievalWarning: null,

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
      set({ activeChatId: chat.id, messages: [], attachments: [] });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  // Streaming state is left alone: coming back to a chat that is still
  // generating must show the answer it accumulated meanwhile.
  selectChat: async (id: string) => {
    set({ activeChatId: id, error: null, retrievalWarning: null });
    try {
      const [messages, attachments] = await Promise.all([
        chatApi.listMessages(id),
        chatApi.listChatAttachments(id),
      ]);
      set({ messages, attachments });
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
        set({ activeChatId: null, messages: [], attachments: [] });
      }
    } catch (err) {
      set({ error: String(err) });
    }
  },

  // The checkbox reflects `chats[].use_global_rag`, so the local list is
  // updated together with the database (CHAT-14).
  setUseGlobalRag: async (id, enabled) => {
    const previous = get().chats;
    set({
      chats: previous.map((c) => (c.id === id ? { ...c, use_global_rag: enabled } : c)),
    });
    try {
      await chatApi.setChatUseGlobalRag(id, enabled);
    } catch (err) {
      set({ error: String(err), chats: previous });
    }
  },

  // The command resolves only when generation ends; tokens arrive meanwhile
  // through the listener below. Everything here is scoped to `chatId` because
  // the user may be looking at another chat by the time it finishes.
  sendMessage: async (content, attachmentPaths) => {
    const chatId = get().activeChatId;
    if (!chatId) return;
    // Shown immediately: the backend only persists it as part of a call that
    // lasts as long as the answer, and until then the user would see their own
    // message vanish.
    const pending: Message = {
      id: `pending-${Date.now()}`,
      chat_id: chatId,
      role: "user",
      content,
      created_at: new Date().toISOString(),
    };
    set({
      messages: [...get().messages, pending],
      generatingChatId: chatId,
      streamingChatId: chatId,
      streamingContent: "",
      error: null,
      retrievalWarning: null,
    });
    try {
      await chatApi.sendMessage(chatId, content, attachmentPaths);
    } catch (err) {
      if (get().activeChatId === chatId) set({ error: String(err) });
    } finally {
      if (get().generatingChatId === chatId) set({ generatingChatId: null });
      if (get().streamingChatId === chatId) {
        set({ streamingChatId: null, streamingContent: "" });
      }
      // Reloading another chat's messages into the view would replace what the
      // user is reading with a different conversation.
      if (get().activeChatId === chatId) {
        const [messages, attachments] = await Promise.all([
          chatApi.listMessages(chatId),
          chatApi.listChatAttachments(chatId),
        ]);
        set({ messages, attachments });
      }
      await get().loadChats();
    }
  },

  cancelGeneration: async () => {
    const chatId = get().generatingChatId;
    if (!chatId) return;
    try {
      await chatApi.cancelGeneration(chatId);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  dismissRetrievalWarning: () => set({ retrievalWarning: null }),
}));

// Retrieval failing is not the message failing: the answer is on its way, it
// just has no documents behind it. Reported separately so the user can tell
// "the knowledge base broke" from "the model ignored my document".
listen<ChatRetrievalWarning>("chat-retrieval-warning", (event) => {
  const { chat_id, reason } = event.payload;
  if (useChatStore.getState().activeChatId !== chat_id) return;
  useChatStore.setState({ retrievalWarning: reason });
});

listen<ChatStreamChunk>("chat-stream-chunk", (event) => {
  const { chat_id, delta, done, error } = event.payload;
  const state = useChatStore.getState();
  // Chunks are accumulated even for a chat the user navigated away from —
  // the generation keeps running and the text must be there on return.
  if (state.streamingChatId !== chat_id) return;

  if (error) {
    useChatStore.setState({
      generatingChatId: null,
      ...(state.activeChatId === chat_id ? { error } : {}),
    });
    return;
  }
  if (done) {
    useChatStore.setState({ generatingChatId: null });
    return;
  }
  useChatStore.setState({ streamingContent: state.streamingContent + delta });
});
