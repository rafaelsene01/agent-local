import { useTranslation } from "react-i18next";
import { MessageSquarePlus } from "lucide-react";
import { useChatStore } from "../../store/chatStore";

export function ChatPanel() {
  const { t } = useTranslation();
  const { activeChatId, chats, messages, createChat } = useChatStore();
  const activeChat = chats.find((c) => c.id === activeChatId);

  if (!activeChat) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 bg-[var(--bg-app)] text-[var(--text-secondary)]">
        <MessageSquarePlus size={40} className="text-[var(--text-secondary)]" />
        <p className="text-sm">{t("chatPanel.selectOrCreate")}</p>
        <button
          onClick={() => createChat()}
          className="rounded-md bg-[var(--accent)] px-4 py-2 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)]"
        >
          {t("chatPanel.newChat")}
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col bg-[var(--bg-app)] text-[var(--text-primary)]">
      <div className="border-b border-[var(--border-color)] px-6 py-4">
        <h1 className="text-base font-semibold">{activeChat.title}</h1>
      </div>

      <div className="flex-1 overflow-y-auto px-6 py-4">
        {messages.length === 0 ? (
          <p className="text-sm text-[var(--text-secondary)]">{t("chatPanel.noMessages")}</p>
        ) : (
          <ul className="space-y-3">
            {messages.map((m) => (
              <li key={m.id} className="text-sm">
                <span className="font-semibold">{m.role}:</span> {m.content}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
