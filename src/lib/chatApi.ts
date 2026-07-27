// SPEC: chat-messaging (CHAT-01, CHAT-04, CHAT-14), app-shell (SHELL-03, SHELL-06, SHELL-07),
//       conversation-memory (MEM-14, MEM-17)

import { invoke } from "@tauri-apps/api/core";
import type { Chat, ChatAttachment, Message } from "../types";

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
  setChatUseMemory: (chatId: string, enabled: boolean) =>
    invoke<void>("set_chat_use_memory", { chatId, enabled }),

  /** Resolves with the number of turns indexed — 0 means the conversation had
   *  no complete exchange yet, which is a normal answer, not a failure. */
  indexChatHistory: (chatId: string) => invoke<number>("index_chat_history", { chatId }),
  listChatAttachments: (chatId: string) =>
    invoke<ChatAttachment[]>("list_chat_attachments", { chatId }),
};
