import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { MessageSquarePlus } from "lucide-react";
import { useChatStore } from "../../store/chatStore";
import { chatApi } from "../../lib/chatApi";
import { MessageInput } from "./MessageInput";

export function ChatPanel() {
  const { t } = useTranslation();
  const { activeChatId, chats, messages, streamingContent, isGenerating, error, createChat } =
    useChatStore();
  const activeChat = chats.find((c) => c.id === activeChatId);
  const [useGlobalDocs, setUseGlobalDocs] = useState(true);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length, streamingContent]);

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

  function handleToggleGlobalRag(enabled: boolean) {
    setUseGlobalDocs(enabled);
    if (activeChatId) {
      chatApi.setChatUseGlobalRag(activeChatId, enabled);
    }
  }

  return (
    <div className="flex flex-1 flex-col bg-[var(--bg-app)] text-[var(--text-primary)]">
      <div className="flex items-center justify-between gap-3 border-b border-[var(--border-color)] px-6 py-4">
        <h1 className="truncate text-base font-semibold">{activeChat.title}</h1>
        <label className="flex shrink-0 items-center gap-1.5 text-xs text-[var(--text-secondary)]">
          <input
            type="checkbox"
            checked={useGlobalDocs}
            onChange={(e) => handleToggleGlobalRag(e.target.checked)}
          />
          {t("chatPanel.useGlobalDocs")}
        </label>
      </div>

      <div className="flex-1 overflow-y-auto px-6 py-4">
        {messages.length === 0 && !streamingContent ? (
          <p className="text-sm text-[var(--text-secondary)]">{t("chatPanel.noMessages")}</p>
        ) : (
          <ul className="space-y-4">
            {messages.map((m) => (
              <li key={m.id} className="text-sm">
                <p className="mb-0.5 text-xs font-semibold uppercase tracking-wide text-[var(--text-secondary)]">
                  {m.role}
                </p>
                <p className="whitespace-pre-wrap">{m.content}</p>
              </li>
            ))}

            {/* The answer being streamed isn't persisted yet, so it lives
                outside the messages list until the backend saves it. */}
            {streamingContent && (
              <li className="text-sm">
                <p className="mb-0.5 text-xs font-semibold uppercase tracking-wide text-[var(--text-secondary)]">
                  assistant
                </p>
                <p className="whitespace-pre-wrap">{streamingContent}</p>
              </li>
            )}
          </ul>
        )}

        {isGenerating && !streamingContent && (
          <p className="mt-3 text-xs text-[var(--text-secondary)]">{t("chatPanel.generating")}</p>
        )}
        {error && <p className="mt-3 text-xs text-red-500">{error}</p>}
        <div ref={bottomRef} />
      </div>

      <MessageInput />
    </div>
  );
}
