// SPEC: self-contained-runtime (SELF-01)

import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Cpu, Download, Play, Square } from "lucide-react";
import { useRuntimeStore } from "../../store/runtimeStore";

export function RuntimeCard() {
  const { t } = useTranslation();
  const { status, progress, isPreparing, error, loadStatus, prepareRuntime, startRuntime, stopRuntime } =
    useRuntimeStore();

  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  if (!status) return null;

  const stage = status.stage;
  const bytes = progress?.progress ?? null;
  const percent =
    bytes?.total_bytes && bytes.downloaded_bytes
      ? Math.min(100, Math.round((bytes.downloaded_bytes / bytes.total_bytes) * 100))
      : null;

  return (
    <div className="rounded-md border border-[var(--border-color)] px-3 py-3">
      <div className="flex items-start justify-between gap-2">
        <div>
          <p className="text-sm font-medium">{t("runtime.engine.title")}</p>
          <p className="text-xs text-[var(--text-secondary)]">{t("runtime.engine.description")}</p>
        </div>
        {status.release_tag && (
          <span className="shrink-0 rounded-full bg-[var(--bg-elevated)] px-2 py-0.5 text-xs text-[var(--text-secondary)]">
            {t("runtime.engine.release", { tag: status.release_tag })}
          </span>
        )}
      </div>

      {stage === "unsupported" ? (
        <p className="mt-3 text-xs text-amber-500">{t("runtime.engine.unsupported")}</p>
      ) : (
        <>
          {status.backend && (
            <p className="mt-2 flex items-center gap-1.5 text-xs text-[var(--text-secondary)]">
              <Cpu size={12} />
              {status.backend === "vulkan"
                ? t("runtime.engine.backendVulkan")
                : t("runtime.engine.backendCpu")}
            </p>
          )}
          {/* EMBED-11: falling back to CPU is stated, never disguised as GPU. */}
          {status.backend === "cpu" && (
            <p className="mt-1 text-xs text-amber-500">{t("runtime.engine.cpuFallback")}</p>
          )}

          {isPreparing && (
            <div className="mt-3">
              <p className="text-xs text-[var(--text-secondary)]">{t("runtime.engine.preparing")}</p>
              <div className="mt-1 h-1.5 w-full overflow-hidden rounded-full bg-[var(--bg-elevated)]">
                <div
                  className="h-full bg-[var(--accent)] transition-all"
                  style={{ width: `${percent ?? 0}%` }}
                />
              </div>
              {progress?.message && (
                <p className="mt-1 text-xs text-[var(--text-secondary)]">{progress.message}</p>
              )}
            </div>
          )}

          {!isPreparing && stage === "not_prepared" && (
            <div className="mt-3">
              <p className="text-xs text-[var(--text-secondary)]">
                {t("runtime.engine.notPrepared")}
              </p>
              <button
                onClick={() => prepareRuntime()}
                className="mt-2 flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)]"
              >
                <Download size={14} />
                {error ? t("runtime.engine.retry") : t("runtime.engine.prepare")}
              </button>
            </div>
          )}

          {/* A prepared runtime with no model is the normal state of a fresh
              install, so it says what is missing instead of looking broken. */}
          {!isPreparing && stage === "no_model" && (
            <p className="mt-3 text-xs text-amber-500">{t("runtime.engine.noModel")}</p>
          )}

          {!isPreparing && (stage === "ready" || stage === "running") && (
            <div className="mt-3 flex items-center gap-2">
              <span className="text-xs text-[var(--text-secondary)]">
                {stage === "running"
                  ? t("runtime.engine.running", { port: status.port })
                  : t("runtime.engine.ready")}
              </span>
              <button
                onClick={() => (stage === "running" ? stopRuntime() : startRuntime())}
                className="ml-auto flex items-center gap-1.5 rounded-md border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium hover:bg-[var(--bg-elevated)]"
              >
                {stage === "running" ? <Square size={12} /> : <Play size={12} />}
                {stage === "running" ? t("runtime.engine.stop") : t("runtime.engine.start")}
              </button>
            </div>
          )}

          {status.model_name && (
            <p className="mt-2 text-xs text-[var(--text-secondary)]">
              {t("runtime.engine.activeModel", { name: status.model_name })}
            </p>
          )}

          {status.message && <p className="mt-2 text-xs text-amber-500">{status.message}</p>}
          {error && <p className="mt-2 text-xs text-red-500">{error}</p>}
        </>
      )}
    </div>
  );
}
