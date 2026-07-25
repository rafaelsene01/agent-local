import { useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { Cpu, Download, Play, Square } from "lucide-react";
import { useConnectionsStore } from "../../store/connectionsStore";

export function EmbeddedRuntimeCard() {
  const { t } = useTranslation();
  const {
    embeddedStatus,
    embeddedProgress,
    isSettingUpEmbedded,
    error,
    loadEmbeddedStatus,
    setupEmbeddedRuntime,
    startEmbeddedRuntime,
    stopEmbeddedRuntime,
    downloadEmbeddedModel,
  } = useConnectionsStore();
  const [customUrl, setCustomUrl] = useState("");

  useEffect(() => {
    loadEmbeddedStatus();
  }, [loadEmbeddedStatus]);

  if (!embeddedStatus) return null;

  const stage = embeddedStatus.stage;
  const progress = embeddedProgress?.progress ?? null;
  const percent =
    progress?.total_bytes && progress.downloaded_bytes
      ? Math.min(100, Math.round((progress.downloaded_bytes / progress.total_bytes) * 100))
      : null;

  function handleCustomDownload(e: FormEvent) {
    e.preventDefault();
    if (!customUrl.trim()) return;
    downloadEmbeddedModel(customUrl.trim());
    setCustomUrl("");
  }

  return (
    <div className="rounded-md border border-[var(--border-color)] px-3 py-3">
      <div className="flex items-start justify-between gap-2">
        <div>
          <p className="text-sm font-medium">{t("connections.embedded.title")}</p>
          <p className="text-xs text-[var(--text-secondary)]">
            {t("connections.embedded.description")}
          </p>
        </div>
        {embeddedStatus.release_tag && (
          <span className="shrink-0 rounded-full bg-[var(--bg-elevated)] px-2 py-0.5 text-xs text-[var(--text-secondary)]">
            {t("connections.embedded.release", { tag: embeddedStatus.release_tag })}
          </span>
        )}
      </div>

      {stage === "unsupported" ? (
        <p className="mt-3 text-xs text-amber-500">{t("connections.embedded.unsupported")}</p>
      ) : (
        <>
          {embeddedStatus.backend && (
            <p className="mt-2 flex items-center gap-1.5 text-xs text-[var(--text-secondary)]">
              <Cpu size={12} />
              {embeddedStatus.backend === "vulkan"
                ? t("connections.embedded.backendVulkan")
                : t("connections.embedded.backendCpu")}
            </p>
          )}
          {/* EMBED-11: falling back to CPU is stated, never disguised as GPU. */}
          {embeddedStatus.backend === "cpu" && (
            <p className="mt-1 text-xs text-amber-500">{t("connections.embedded.cpuFallback")}</p>
          )}

          {isSettingUpEmbedded && (
            <div className="mt-3">
              <p className="text-xs text-[var(--text-secondary)]">
                {embeddedProgress?.stage === "downloading_model"
                  ? t("connections.embedded.downloadingModel")
                  : t("connections.embedded.downloadingBinary")}
              </p>
              <div className="mt-1 h-1.5 w-full overflow-hidden rounded-full bg-[var(--bg-elevated)]">
                <div
                  className="h-full bg-[var(--accent)] transition-all"
                  style={{ width: `${percent ?? 0}%` }}
                />
              </div>
              {embeddedProgress?.message && (
                <p className="mt-1 text-xs text-[var(--text-secondary)]">
                  {embeddedProgress.message}
                </p>
              )}
            </div>
          )}

          {!isSettingUpEmbedded && stage === "not_installed" && (
            <div className="mt-3">
              <p className="text-xs text-[var(--text-secondary)]">
                {t("connections.embedded.notInstalled")}
              </p>
              <button
                onClick={() => setupEmbeddedRuntime()}
                className="mt-2 flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)]"
              >
                <Download size={14} />
                {error ? t("connections.embedded.retry") : t("connections.embedded.install")}
              </button>
            </div>
          )}

          {!isSettingUpEmbedded && (stage === "ready" || stage === "running") && (
            <div className="mt-3 flex items-center gap-2">
              <span className="text-xs text-[var(--text-secondary)]">
                {stage === "running"
                  ? t("connections.embedded.running", { port: embeddedStatus.port })
                  : t("connections.embedded.ready")}
              </span>
              <button
                onClick={() => (stage === "running" ? stopEmbeddedRuntime() : startEmbeddedRuntime())}
                className="ml-auto flex items-center gap-1.5 rounded-md border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium hover:bg-[var(--bg-elevated)]"
              >
                {stage === "running" ? <Square size={12} /> : <Play size={12} />}
                {stage === "running"
                  ? t("connections.embedded.stop")
                  : t("connections.embedded.start")}
              </button>
            </div>
          )}

          {(stage === "ready" || stage === "running") && (
            <form onSubmit={handleCustomDownload} className="mt-3 flex gap-2">
              <input
                type="text"
                value={customUrl}
                onChange={(e) => setCustomUrl(e.target.value)}
                placeholder={t("connections.embedded.customModelPlaceholder")}
                title={t("connections.embedded.customModel")}
                className="flex-1 rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-2 py-1.5 text-xs"
              />
              <button
                type="submit"
                disabled={!customUrl.trim()}
                className="rounded-md border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium hover:bg-[var(--bg-elevated)] disabled:opacity-50"
              >
                {t("connections.embedded.customModelDownload")}
              </button>
            </form>
          )}

          {error && <p className="mt-2 text-xs text-red-500">{error}</p>}
        </>
      )}
    </div>
  );
}
