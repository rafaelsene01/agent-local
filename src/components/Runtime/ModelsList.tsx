// SPEC: self-contained-runtime (SELF-01, SELF-07)

import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { Download, Settings2 } from "lucide-react";
import { useRuntimeStore } from "../../store/runtimeStore";
import { ModelConfigForm } from "./ModelConfigForm";
import { ModelDownloadCard } from "./ModelDownloadCard";

/** Read from the `.gguf` on disk, so it is always known — unlike the old
 *  `/v1/models` route, which only knew about the loaded model (AD-028). */
function formatSize(bytes: number | null) {
  if (!bytes) return "—";
  return `${(bytes / 1e9).toFixed(1)} GB`;
}

export function ModelsList() {
  const { t } = useTranslation();
  const {
    installedModels,
    downloadableModels,
    ramDetectedGb,
    activeModel,
    downloadProgress,
    error,
    loadInstalledModels,
    loadDownloadableModels,
    loadActiveModel,
    setActiveModel,
    downloadModel,
  } = useRuntimeStore();

  const [showAll, setShowAll] = useState(false);
  const [manualUrl, setManualUrl] = useState("");
  const [configuringModel, setConfiguringModel] = useState<string | null>(null);

  useEffect(() => {
    loadInstalledModels();
    loadDownloadableModels();
    loadActiveModel();
  }, [loadInstalledModels, loadDownloadableModels, loadActiveModel]);

  // What fits comes first: a list that opens on disabled-looking cards reads
  // as "nothing here works".
  const visibleDownloadable = useMemo(
    () => downloadableModels.filter((m) => showAll || m.fits_ram),
    [downloadableModels, showAll],
  );

  function handleManualDownload(e: FormEvent) {
    e.preventDefault();
    if (!manualUrl.trim()) return;
    downloadModel(manualUrl.trim());
    setManualUrl("");
  }

  return (
    <div className="space-y-8">
      <section>
        <h3 className="text-sm font-medium">{t("runtime.installedModels")}</h3>

        {installedModels.length === 0 && (
          <p className="mt-2 text-sm text-[var(--text-secondary)]">
            {t("runtime.noInstalledModels")}
          </p>
        )}

        <div className="mt-2 space-y-1">
          {installedModels.map((model) => {
            const isActive = activeModel?.name === model.name;
            const isConfiguring = configuringModel === model.name;
            return (
              <div key={model.name}>
                <div className="flex items-center justify-between gap-3 rounded-md border border-[var(--border-color)] px-3 py-1.5">
                  <span className="min-w-0 truncate text-sm" title={model.name}>
                    {model.name}
                  </span>
                  <div className="flex shrink-0 items-center gap-3">
                    <span className="text-xs text-[var(--text-secondary)]">
                      {formatSize(model.size_bytes)}
                    </span>
                    {/* Only the active model can be configured: context and
                        GPU offload are flags of the running process, not
                        per-file settings. */}
                    {isActive && (
                      <button
                        onClick={() => setConfiguringModel(isConfiguring ? null : model.name)}
                        className="rounded-md p-1 text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-primary)]"
                        title={t("runtime.configureModel", { model: model.name })}
                      >
                        <Settings2 size={14} />
                      </button>
                    )}
                    <button
                      onClick={() => setActiveModel(model.name)}
                      className={`rounded-md px-2 py-1 text-xs font-medium ${
                        isActive
                          ? "bg-[var(--accent)] text-[var(--accent-fg)]"
                          : "border border-[var(--border-color)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                      }`}
                    >
                      {isActive ? t("runtime.active") : t("runtime.useModel")}
                    </button>
                  </div>
                </div>
                {isConfiguring && (
                  <div className="mt-1">
                    <ModelConfigForm
                      modelName={model.name}
                      onClose={() => setConfiguringModel(null)}
                    />
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {error && <p className="mt-2 text-xs text-red-500">{error}</p>}
      </section>

      <section>
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-medium">{t("runtime.downloadModels")}</h3>
          <label className="flex items-center gap-1.5 text-xs text-[var(--text-secondary)]">
            <input
              type="checkbox"
              checked={showAll}
              onChange={(e) => setShowAll(e.target.checked)}
            />
            {t("runtime.showAllModels")}
          </label>
        </div>
        {ramDetectedGb == null && (
          <p className="mt-1 text-xs text-amber-500">{t("runtime.ramUnknown")}</p>
        )}
        <div className="mt-2 space-y-2">
          {visibleDownloadable.map((model) => (
            <ModelDownloadCard
              key={model.id}
              model={model}
              progress={downloadProgress[model.pull_identifier]}
              onDownload={() => downloadModel(model.pull_identifier)}
            />
          ))}
        </div>
      </section>

      <section>
        <h3 className="text-sm font-medium">{t("runtime.manualDownload")}</h3>
        <form onSubmit={handleManualDownload} className="mt-2 flex gap-2">
          <input
            type="text"
            value={manualUrl}
            onChange={(e) => setManualUrl(e.target.value)}
            placeholder={t("runtime.manualDownloadPlaceholder")}
            className="flex-1 rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-2 py-1.5 text-sm"
          />
          <button
            type="submit"
            disabled={!manualUrl.trim()}
            className="flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            <Download size={14} />
            {t("runtime.download")}
          </button>
        </form>
      </section>
    </div>
  );
}
