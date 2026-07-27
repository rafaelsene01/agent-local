// SPEC: chat-messaging (CHAT-01, CHAT-04, CHAT-10, CHAT-14),
//       conversation-memory (MEM-14, MEM-17, MEM-18)

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, Brain, MessageSquarePlus, Paperclip } from "lucide-react";
import { useChatStore } from "../../store/chatStore";
import { MessageInput } from "./MessageInput";
import type { Message } from "../../types";

/// The side carries the role, the way every chat app does it: what the user
/// sent sits on the right in an accent bubble, the model answers on the left.
/// A `system` message (none are persisted today, but the schema allows it)
/// gets neither side — it is a note about the conversation, not a turn in it.
function MessageBubble({ role, content }: { role: Message["role"]; content: string }) {
  if (role === "system") {
    return (
      <li className="self-center px-4 text-center text-xs italic text-[var(--text-secondary)]">
        {content}
      </li>
    );
  }

  const isUser = role === "user";
  return (
    <li
      className={`flex ${isUser ? "justify-end" : "justify-start"}`}
      // Screen readers lose the layout, so the role stays available as text.
      aria-label={role}
    >
      <div
        className={`max-w-[80%] whitespace-pre-wrap rounded-2xl px-4 py-2.5 text-sm ${
          isUser
            ? "rounded-br-sm bg-[var(--accent)] text-[var(--accent-fg)]"
            : "rounded-bl-sm bg-[var(--bg-elevated)] text-[var(--text-primary)]"
        }`}
      >
        {content}
      </div>
    </li>
  );
}

export function ChatPanel() {
  const { t } = useTranslation();
  const {
    activeChatId,
    chats,
    messages,
    attachments,
    streamingContent,
    streamingChatId,
    generatingChatId,
    error,
    retrievalWarning,
    memoryIndexing,
    memoryIndexed,
    createChat,
    setUseGlobalRag,
    setUseMemory,
    indexHistory,
    dismissRetrievalWarning,
  } = useChatStore();
  const activeChat = chats.find((c) => c.id === activeChatId);
  // Only this chat's own generation shows up here; another chat streaming in
  // the background must not paint over the conversation on screen.
  const isGenerating = generatingChatId !== null && generatingChatId === activeChatId;
  const streaming = streamingChatId === activeChatId ? streamingContent : "";
  const failedAttachments = attachments.filter((a) => a.status === "error");
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length, streaming]);

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
      <div className="flex items-center justify-between gap-3 border-b border-[var(--border-color)] px-6 py-4">
        <h1 className="truncate text-base font-semibold">{activeChat.title}</h1>
        <div className="flex shrink-0 items-center gap-4 text-xs text-[var(--text-secondary)]">
          <label className="flex items-center gap-1.5">
            <input
              type="checkbox"
              checked={activeChat.use_global_rag}
              onChange={(e) => setUseGlobalRag(activeChat.id, e.target.checked)}
            />
            {t("chatPanel.useGlobalDocs")}
          </label>
          <label className="flex items-center gap-1.5">
            <input
              type="checkbox"
              checked={activeChat.use_memory}
              onChange={(e) => setUseMemory(activeChat.id, e.target.checked)}
            />
            {t("chatPanel.useMemory")}
          </label>
          {/* Only offered where it can do something: with the toggle off the
              command refuses, so showing the button would be a dead end. */}
          {activeChat.use_memory && (
            <button
              onClick={() => indexHistory(activeChat.id)}
              disabled={memoryIndexing !== null}
              className="flex items-center gap-1.5 rounded-md border border-[var(--border-color)] px-2 py-1 hover:bg-[var(--bg-elevated)] disabled:opacity-60"
            >
              <Brain size={12} />
              {memoryIndexing
                ? t("chatPanel.indexingHistory", {
                    done: memoryIndexing.done,
                    total: memoryIndexing.total,
                  })
                : t("chatPanel.indexHistory")}
            </button>
          )}
        </div>
      </div>

      {/* A run that indexed nothing has to say so: silence after a click is
          indistinguishable from a button that is broken (MEM-20). */}
      {memoryIndexed !== null && (
        <p className="border-b border-[var(--border-color)] px-6 py-2 text-xs text-[var(--text-secondary)]">
          {memoryIndexed > 0
            ? t("chatPanel.indexedTurns", { count: memoryIndexed })
            : t("chatPanel.nothingToIndex")}
        </p>
      )}

      {/* The files this chat can already answer about — the counterpart of the
          failure notice further down. */}
      {attachments.some((a) => a.status !== "error") && (
        <div className="flex flex-wrap items-center gap-2 border-b border-[var(--border-color)] px-6 py-2 text-xs text-[var(--text-secondary)]">
          <Paperclip size={12} />
          {attachments
            .filter((a) => a.status !== "error")
            .map((a) => (
              <span key={a.id} className="rounded-full bg-[var(--bg-elevated)] px-2 py-0.5">
                {a.filename}
              </span>
            ))}
        </div>
      )}

      {/* The same max-width as the bubbles keeps every notice aligned with the
          conversation instead of hugging the window edge. */}
      <div className="flex-1 overflow-y-auto px-6 py-4 [&>*:not(ul)]:mx-auto [&>*:not(ul)]:max-w-3xl">
        {messages.length === 0 && !streaming ? (
          <p className="text-sm text-[var(--text-secondary)]">{t("chatPanel.noMessages")}</p>
        ) : (
          <ul className="mx-auto flex max-w-3xl flex-col gap-3">
            {messages.map((m) => (
              <MessageBubble key={m.id} role={m.role} content={m.content} />
            ))}

            {/* The answer being streamed isn't persisted yet, so it lives
                outside the messages list until the backend saves it. */}
            {streaming && <MessageBubble role="assistant" content={streaming} />}
          </ul>
        )}

        {isGenerating && !streaming && (
          <p className="mt-3 text-xs text-[var(--text-secondary)]">{t("chatPanel.generating")}</p>
        )}

        {/* CHAT-10: a file that failed to process is said out loud here — the
            message was answered without it. */}
        {failedAttachments.map((attachment) => (
          <p
            key={attachment.id}
            className="mt-3 flex items-start gap-1.5 text-xs text-amber-500"
          >
            <AlertTriangle size={12} className="mt-0.5 shrink-0" />
            {t("chatPanel.attachmentFailed", {
              name: attachment.filename,
              reason: attachment.error_message ?? "",
            })}
          </p>
        ))}

        {/* Retrieval failing is silent from the outside: the answer arrives,
            just without the documents. Saying so is the difference between a
            bug the user can report and one they blame on the model. */}
        {retrievalWarning && (
          <p className="mt-3 flex items-start gap-1.5 text-xs text-amber-500">
            <AlertTriangle size={12} className="mt-0.5 shrink-0" />
            <span className="min-w-0">
              {t("chatPanel.retrievalFailed", { reason: retrievalWarning })}{" "}
              <button
                onClick={dismissRetrievalWarning}
                className="underline underline-offset-2 hover:text-[var(--text-primary)]"
              >
                {t("chatPanel.dismiss")}
              </button>
            </span>
          </p>
        )}

        {error && <p className="mt-3 text-xs text-red-500">{error}</p>}
        <div ref={bottomRef} />
      </div>

      <MessageInput />
    </div>
  );
}
