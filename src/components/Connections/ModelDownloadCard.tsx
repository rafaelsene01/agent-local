import { useTranslation } from "react-i18next";
import { Download } from "lucide-react";
import type { DownloadableModel, PullProgress } from "../../types";

interface Props {
  model: DownloadableModel;
  progress?: PullProgress;
  disabled: boolean;
  disabledReason?: string;
  onPull: () => void;
}

export function ModelDownloadCard({ model, progress, disabled, disabledReason, onPull }: Props) {
  const { t } = useTranslation();
  const isDownloading = progress && progress.status !== "success" && progress.status !== "error";
  const percent =
    progress?.total_bytes && progress.downloaded_bytes
      ? Math.min(100, Math.round((progress.downloaded_bytes / progress.total_bytes) * 100))
      : null;

  return (
    <div className="rounded-md border border-[var(--border-color)] px-3 py-2">
      <div className="flex items-center justify-between gap-2">
        <div>
          <p className="text-sm font-medium">{model.display_name}</p>
          <p className="text-xs text-[var(--text-secondary)]">
            {model.params_billions}B · {model.default_quant} · ~{model.estimated_ram_gb.toFixed(1)} GB
          </p>
        </div>
        {!model.fits_ram && (
          <span className="shrink-0 rounded-full bg-amber-500/20 px-2 py-0.5 text-xs text-amber-500">
            {t("connections.notRecommended")}
          </span>
        )}
      </div>

      {isDownloading ? (
        <div className="mt-2">
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-[var(--bg-elevated)]">
            <div
              className="h-full bg-[var(--accent)] transition-all"
              style={{ width: `${percent ?? 0}%` }}
            />
          </div>
          <p className="mt-1 text-xs text-[var(--text-secondary)]">{progress?.message ?? progress?.status}</p>
        </div>
      ) : (
        <button
          onClick={onPull}
          disabled={disabled}
          title={disabledReason}
          className="mt-2 flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          <Download size={14} />
          {progress?.status === "success" ? t("connections.downloaded") : t("connections.download")}
        </button>
      )}
      {progress?.status === "error" && (
        <p className="mt-1 text-xs text-red-500">{progress.message ?? t("connections.downloadError")}</p>
      )}
    </div>
  );
}
