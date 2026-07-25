import { useEffect, useState, type FormEvent, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { Paperclip, Send, Square } from "lucide-react";
import { useChatStore } from "../../store/chatStore";
import { useConnectionsStore } from "../../store/connectionsStore";

export function MessageInput() {
  const { t } = useTranslation();
  const { isGenerating, sendMessage, cancelGeneration } = useChatStore();
  const { activePair, loadActivePair } = useConnectionsStore();
  const [text, setText] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);

  useEffect(() => {
    loadActivePair();
  }, [loadActivePair]);

  // CHAT-02: without an active pair there is nowhere to send, so the input is
  // blocked here instead of failing after the user typed a message.
  const canSend = Boolean(activePair.connection && activePair.model);

  async function handleAttach() {
    const selected = await open({ multiple: true, title: t("chatPanel.fileDialogTitle") });
    if (!selected) return;
    setAttachments(Array.isArray(selected) ? selected : [selected]);
  }

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    const content = text.trim();
    if (!content || !canSend || isGenerating) return;
    setText("");
    const files = attachments;
    setAttachments([]);
    sendMessage(content, files);
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e as unknown as FormEvent);
    }
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="border-t border-[var(--border-color)] px-6 py-3"
    >
      {!canSend && (
        <p className="mb-2 text-xs text-amber-500">{t("chatPanel.noActiveModel")}</p>
      )}

      {attachments.length > 0 && (
        <div className="mb-2 flex items-center gap-2 text-xs text-[var(--text-secondary)]">
          <span>{t("chatPanel.attachmentsSelected", { count: attachments.length })}</span>
          <button
            type="button"
            onClick={() => setAttachments([])}
            className="underline hover:text-[var(--text-primary)]"
          >
            {t("chatPanel.clearAttachments")}
          </button>
        </div>
      )}

      <div className="flex items-end gap-2">
        <button
          type="button"
          onClick={handleAttach}
          disabled={!canSend}
          title={t("chatPanel.attach")}
          className="rounded-md p-2 text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-primary)] disabled:opacity-50"
        >
          <Paperclip size={16} />
        </button>

        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          rows={1}
          disabled={!canSend}
          placeholder={t("chatPanel.placeholder")}
          className="max-h-40 flex-1 resize-y rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-3 py-2 text-sm outline-none disabled:opacity-50"
        />

        {isGenerating ? (
          <button
            type="button"
            onClick={() => cancelGeneration()}
            className="flex items-center gap-1.5 rounded-md border border-[var(--border-color)] px-3 py-2 text-sm font-medium hover:bg-[var(--bg-elevated)]"
          >
            <Square size={14} />
            {t("chatPanel.cancel")}
          </button>
        ) : (
          <button
            type="submit"
            disabled={!canSend || !text.trim()}
            className="flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-2 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            <Send size={14} />
            {t("chatPanel.send")}
          </button>
        )}
      </div>
    </form>
  );
}
