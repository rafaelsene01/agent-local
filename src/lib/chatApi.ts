import { invoke } from "@tauri-apps/api/core";
import type { Chat, Message } from "../types";

export const chatApi = {
  createChat: (title?: string) => invoke<Chat>("create_chat", { title }),
  listChats: () => invoke<Chat[]>("list_chats"),
  renameChat: (id: string, title: string) => invoke<Chat>("rename_chat", { id, title }),
  deleteChat: (id: string) => invoke<void>("delete_chat", { id }),
  listMessages: (chatId: string) => invoke<Message[]>("list_messages", { chatId }),

  /** Resolves with the user message id; the answer arrives as
   *  `chat-stream-chunk` events (AD-018). */
  sendMessage: (chatId: string, content: string, attachmentPaths: string[]) =>
    invoke<string>("send_message", { chatId, content, attachmentPaths }),
  cancelGeneration: (chatId: string) => invoke<void>("cancel_generation", { chatId }),
  setChatUseGlobalRag: (chatId: string, enabled: boolean) =>
    invoke<void>("set_chat_use_global_rag", { chatId, enabled }),
};
