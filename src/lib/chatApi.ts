import { invoke } from "@tauri-apps/api/core";
import type { Chat, Message } from "../types";

export const chatApi = {
  createChat: (title?: string) => invoke<Chat>("create_chat", { title }),
  listChats: () => invoke<Chat[]>("list_chats"),
  renameChat: (id: string, title: string) => invoke<Chat>("rename_chat", { id, title }),
  deleteChat: (id: string) => invoke<void>("delete_chat", { id }),
  listMessages: (chatId: string) => invoke<Message[]>("list_messages", { chatId }),
};
