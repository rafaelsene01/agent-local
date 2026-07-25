import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { Download } from "lucide-react";
import { useConnectionsStore } from "../../store/connectionsStore";
import { ModelDownloadCard } from "./ModelDownloadCard";

export function ModelsList() {
  const { t } = useTranslation();
  const {
    connections,
    installedModelsByConnection,
    downloadableModels,
    ramDetectedGb,
    activeModel,
    downloadProgress,
    loadInstalledModels,
    loadDownloadableModels,
    loadActiveModel,
    setActiveModel,
    pullModel,
  } = useConnectionsStore();

  const [showAll, setShowAll] = useState(false);
  const [manualConnectionId, setManualConnectionId] = useState("");
  const [manualIdentifier, setManualIdentifier] = useState("");

  const enabledConnections = useMemo(() => connections.filter((c) => c.enabled), [connections]);

  useEffect(() => {
    loadDownloadableModels();
    loadActiveModel();
  }, [loadDownloadableModels, loadActiveModel]);

  useEffect(() => {
    enabledConnections.forEach((c) => loadInstalledModels(c.id));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabledConnections.map((c) => c.id).join(",")]);

  useEffect(() => {
    if (!manualConnectionId && enabledConnections.length > 0) {
      setManualConnectionId(enabledConnections[0].id);
    }
  }, [enabledConnections, manualConnectionId]);

  const visibleDownloadable = downloadableModels.filter((m) => showAll || m.fits_ram);

  function handleManualPull(e: FormEvent) {
    e.preventDefault();
    if (!manualConnectionId || !manualIdentifier.trim()) return;
    pullModel(manualConnectionId, manualIdentifier.trim());
    setManualIdentifier("");
  }

  return (
    <div className="space-y-8">
      <section>
        <h3 className="text-sm font-medium">{t("connections.installedModels")}</h3>
        {enabledConnections.length === 0 && (
          <p className="mt-2 text-sm text-[var(--text-secondary)]">{t("connections.noEnabledConnections")}</p>
        )}
        <div className="mt-2 space-y-4">
          {enabledConnections.map((conn) => {
            const models = installedModelsByConnection[conn.id] ?? [];
            return (
              <div key={conn.id}>
                <p className="text-xs font-medium uppercase tracking-wide text-[var(--text-secondary)]">
                  {conn.provider} · {conn.base_url}
                </p>
                {models.length === 0 ? (
                  <p className="mt-1 text-xs text-[var(--text-secondary)]">{t("connections.noInstalledModels")}</p>
                ) : (
                  <div className="mt-1 space-y-1">
                    {models.map((m) => {
                      const isActive =
                        activeModel?.connection_id === conn.id && activeModel?.model_name === m.name;
                      return (
                        <div
                          key={m.name}
                          className="flex items-center justify-between rounded-md border border-[var(--border-color)] px-3 py-1.5"
                        >
                          <span className="text-sm">{m.name}</span>
                          <button
                            onClick={() => setActiveModel(conn.id, m.name)}
                            className={`rounded-md px-2 py-1 text-xs font-medium ${
                              isActive
                                ? "bg-[var(--accent)] text-[var(--accent-fg)]"
                                : "border border-[var(--border-color)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                            }`}
                          >
                            {isActive ? t("connections.active") : t("connections.useModel")}
                          </button>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </section>

      <section>
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-medium">{t("connections.downloadModels")}</h3>
          <label className="flex items-center gap-1.5 text-xs text-[var(--text-secondary)]">
            <input type="checkbox" checked={showAll} onChange={(e) => setShowAll(e.target.checked)} />
            {t("connections.showAllModels")}
          </label>
        </div>
        {ramDetectedGb == null && <p className="mt-1 text-xs text-amber-500">{t("connections.ramUnknown")}</p>}
        <div className="mt-2 space-y-2">
          {visibleDownloadable.map((model) => {
            const targetConnection = enabledConnections.find((c) => c.provider === model.provider);
            const key = targetConnection ? `${targetConnection.id}:${model.pull_identifier}` : undefined;
            return (
              <ModelDownloadCard
                key={model.id}
                model={model}
                progress={key ? downloadProgress[key] : undefined}
                disabled={!targetConnection}
                disabledReason={!targetConnection ? t("connections.noConnectionForProvider") : undefined}
                onPull={() => targetConnection && pullModel(targetConnection.id, model.pull_identifier)}
              />
            );
          })}
        </div>
      </section>

      <section>
        <h3 className="text-sm font-medium">{t("connections.manualPull")}</h3>
        <form onSubmit={handleManualPull} className="mt-2 flex gap-2">
          <select
            value={manualConnectionId}
            onChange={(e) => setManualConnectionId(e.target.value)}
            className="rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-2 py-1.5 text-sm"
          >
            {enabledConnections.map((c) => (
              <option key={c.id} value={c.id}>
                {c.provider} · {c.base_url}
              </option>
            ))}
          </select>
          <input
            type="text"
            value={manualIdentifier}
            onChange={(e) => setManualIdentifier(e.target.value)}
            placeholder={t("connections.manualPullPlaceholder")}
            className="flex-1 rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-2 py-1.5 text-sm"
          />
          <button
            type="submit"
            disabled={!manualConnectionId || !manualIdentifier.trim()}
            className="flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            <Download size={14} />
            {t("connections.pull")}
          </button>
        </form>
      </section>
    </div>
  );
}
