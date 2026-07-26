import { useTranslation } from "react-i18next";
import { AlertTriangle, Download, X } from "lucide-react";
import { useUpdateStore } from "../../store/updateStore";
import { useChatStore } from "../../store/chatStore";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

/**
 * Non-blocking strip above the active panel. It never covers the app: the user
 * can keep chatting and deal with the update later, which is the whole point of
 * not using a modal.
 */
export function UpdateBanner() {
  const { t } = useTranslation();
  const { available, dismissed, installing, progress, error, install, skip, dismiss } =
    useUpdateStore();
  const generatingChatId = useChatStore((s) => s.generatingChatId);

  if (!available || dismissed) return null;

  const percent =
    progress && progress.total ? Math.min(100, (progress.downloaded / progress.total) * 100) : null;

  async function handleInstall() {
    // Installing restarts the app; losing an answer mid-generation without
    // warning would be a nasty surprise.
    if (generatingChatId && !window.confirm(t("update.confirmWhileGenerating"))) return;
    await install();
  }

  return (
    <div className="border-b border-[var(--border-color)] bg-[var(--bg-elevated)] px-6 py-3 text-[var(--text-primary)]">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <Download size={16} className="shrink-0 text-[var(--accent)]" />

        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium">
            {t("update.availableTitle", { version: available.version })}
          </p>
          <p className="text-xs text-[var(--text-secondary)]">
            {t("update.currentVersion", { version: available.current_version })}
          </p>
          {available.notes && (
            <p className="mt-1 line-clamp-3 whitespace-pre-line text-xs text-[var(--text-secondary)]">
              {available.notes}
            </p>
          )}
        </div>

        {error ? (
          <div className="flex items-center gap-2">
            <span className="flex items-center gap-1.5 text-xs text-[var(--danger,#f87171)]">
              <AlertTriangle size={14} />
              {error}
            </span>
            <button
              onClick={handleInstall}
              className="rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)]"
            >
              {t("update.retry")}
            </button>
          </div>
        ) : installing ? (
          <div className="w-56 shrink-0">
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-[var(--bg-app)]">
              <div
                className="h-full rounded-full bg-[var(--accent)] transition-all"
                style={{ width: percent === null ? "100%" : `${percent}%` }}
              />
            </div>
            <p className="mt-1 text-xs text-[var(--text-secondary)]">
              {progress
                ? t("update.downloading", {
                    downloaded: formatBytes(progress.downloaded),
                    total: progress.total ? formatBytes(progress.total) : "—",
                  })
                : t("update.preparing")}
            </p>
          </div>
        ) : (
          <div className="flex shrink-0 items-center gap-2">
            <button
              onClick={handleInstall}
              className="rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)]"
            >
              {t("update.install")}
            </button>
            <button
              onClick={dismiss}
              className="rounded-md border border-[var(--border-color)] px-3 py-1.5 text-sm hover:bg-[var(--bg-app)]"
            >
              {t("update.later")}
            </button>
            <button
              onClick={skip}
              className="rounded-md px-2 py-1.5 text-sm text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
              title={t("update.skipTitle", { version: available.version })}
            >
              {t("update.skip")}
            </button>
            <button
              onClick={dismiss}
              className="rounded-md p-1.5 text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
              aria-label={t("update.later")}
            >
              <X size={14} />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
