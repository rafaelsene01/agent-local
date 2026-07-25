import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, MessageSquare, Pencil, Trash2, Check, X } from "lucide-react";
import { useChatStore } from "../../store/chatStore";
import { useUiStore } from "../../store/uiStore";

export function ChatList() {
  const { t } = useTranslation();
  const { chats, activeChatId, loadChats, createChat, selectChat, renameChat, deleteChat } =
    useChatStore();
  const setActiveView = useUiStore((s) => s.setActiveView);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");

  useEffect(() => {
    loadChats();
  }, [loadChats]);

  function handleCreateChat() {
    setActiveView("chat");
    createChat();
  }

  function handleSelectChat(id: string) {
    setActiveView("chat");
    selectChat(id);
  }

  function startRename(id: string, currentTitle: string) {
    setEditingId(id);
    setEditValue(currentTitle);
  }

  function confirmRename() {
    if (editingId) {
      renameChat(editingId, editValue);
    }
    setEditingId(null);
  }

  return (
    <div className="flex flex-1 min-h-0 flex-col">
      <div className="flex items-center justify-between px-3 py-2">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-[var(--text-secondary)]">
          {t("sidebar.chats")}
        </h2>
        <button
          onClick={handleCreateChat}
          className="rounded-md p-1.5 text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-primary)]"
          title={t("chats.newChat")}
        >
          <Plus size={16} />
        </button>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto px-2 pb-2">
        {chats.length === 0 && (
          <div className="mt-4 px-2 text-center text-sm text-[var(--text-secondary)]">
            {t("chats.empty")}
            <button
              onClick={handleCreateChat}
              className="mt-2 block w-full rounded-md bg-[var(--bg-elevated)] px-3 py-1.5 text-[var(--text-primary)] hover:opacity-90"
            >
              {t("chats.createFirst")}
            </button>
          </div>
        )}

        <ul className="space-y-0.5">
          {chats.map((chat) => (
            <li key={chat.id}>
              <div
                className={`group flex items-center gap-2 rounded-md px-2 py-1.5 text-sm cursor-pointer ${
                  activeChatId === chat.id
                    ? "bg-[var(--bg-elevated)] text-[var(--text-primary)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)]/60"
                }`}
                onClick={() => editingId !== chat.id && handleSelectChat(chat.id)}
              >
                <MessageSquare size={14} className="shrink-0 text-[var(--text-secondary)]" />

                {editingId === chat.id ? (
                  <input
                    autoFocus
                    value={editValue}
                    onChange={(e) => setEditValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") confirmRename();
                      if (e.key === "Escape") setEditingId(null);
                    }}
                    className="min-w-0 flex-1 rounded bg-[var(--bg-app)] px-1 py-0.5 text-sm text-[var(--text-primary)] outline-none"
                  />
                ) : (
                  <span className="min-w-0 flex-1 truncate">{chat.title}</span>
                )}

                <div className="flex shrink-0 items-center gap-1 opacity-0 group-hover:opacity-100">
                  {editingId === chat.id ? (
                    <>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          confirmRename();
                        }}
                        className="rounded p-1 hover:bg-[var(--bg-app)]"
                        title={t("chats.save")}
                      >
                        <Check size={13} />
                      </button>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setEditingId(null);
                        }}
                        className="rounded p-1 hover:bg-[var(--bg-app)]"
                        title={t("chats.cancel")}
                      >
                        <X size={13} />
                      </button>
                    </>
                  ) : (
                    <>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          startRename(chat.id, chat.title);
                        }}
                        className="rounded p-1 hover:bg-[var(--bg-app)]"
                        title={t("chats.rename")}
                      >
                        <Pencil size={13} />
                      </button>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          deleteChat(chat.id);
                        }}
                        className="rounded p-1 hover:bg-[var(--bg-app)]"
                        title={t("chats.delete")}
                      >
                        <Trash2 size={13} />
                      </button>
                    </>
                  )}
                </div>
              </div>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
