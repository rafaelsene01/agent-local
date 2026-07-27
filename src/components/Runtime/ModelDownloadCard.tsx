// SPEC: self-contained-runtime (SELF-01)

import { useTranslation } from "react-i18next";
import { Download } from "lucide-react";
import type { DownloadableModel, PullProgress } from "../../types";

interface Props {
  model: DownloadableModel;
  progress?: PullProgress;
  onDownload: () => void;
}

export function ModelDownloadCard({ model, progress, onDownload }: Props) {
  const { t } = useTranslation();
  const isDownloading = progress && progress.status !== "success" && progress.status !== "error";
  const percent =
    progress?.total_bytes && progress.downloaded_bytes
      ? Math.min(100, Math.round((progress.downloaded_bytes / progress.total_bytes) * 100))
      : null;

  return (
    <div className="rounded-md border border-[var(--border-color)] px-3 py-2">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate text-sm font-medium">{model.display_name}</p>
          <p className="text-xs text-[var(--text-secondary)]">
            {model.params_billions}B · {model.default_quant} ·{" "}
            {t("runtime.ramEstimate", { gb: model.estimated_ram_gb.toFixed(1) })}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {!model.fits_ram && (
            <span className="rounded-full bg-amber-500/20 px-2 py-0.5 text-xs text-amber-500">
              {t("runtime.notRecommended")}
            </span>
          )}
          {/* The exact download size is known for every entry: it was read
              from the server's content-length when the catalog was written. */}
          <span className="text-xs text-[var(--text-secondary)]">
            {model.download_bytes
              ? `${(model.download_bytes / 1e9).toFixed(1)} GB`
              : `~${model.estimated_ram_gb.toFixed(1)} GB`}
          </span>
        </div>
      </div>

      {isDownloading ? (
        <div className="mt-2">
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-[var(--bg-elevated)]">
            <div
              className="h-full bg-[var(--accent)] transition-all"
              style={{ width: `${percent ?? 0}%` }}
            />
          </div>
          <p className="mt-1 text-xs text-[var(--text-secondary)]">
            {progress?.message ?? progress?.status}
          </p>
        </div>
      ) : (
        <div className="mt-2">
          <button
            onClick={onDownload}
            className="flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)]"
          >
            <Download size={14} />
            {progress?.status === "success" ? t("runtime.downloaded") : t("runtime.download")}
          </button>
        </div>
      )}
      {progress?.status === "error" && (
        <p className="mt-1 text-xs text-red-500">{progress.message ?? t("runtime.downloadError")}</p>
      )}
    </div>
  );
}
