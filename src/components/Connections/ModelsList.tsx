import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { Download, Settings2 } from "lucide-react";
import { useConnectionsStore } from "../../store/connectionsStore";
import { ModelConfigForm } from "./ModelConfigForm";
import { ModelDownloadCard } from "./ModelDownloadCard";

export function ModelsList() {
  const { t } = useTranslation();
  const {
    connections,
    installedModelsByConnection,
    downloadableModels,
    ramDetectedGb,
    activePair,
    downloadProgress,
    loadAvailableInstalledModels,
    loadDownloadableModels,
    loadActivePair,
    setActiveModel,
    pullModel,
  } = useConnectionsStore();

  const [showAll, setShowAll] = useState(false);
  const [manualConnectionId, setManualConnectionId] = useState("");
  const [manualIdentifier, setManualIdentifier] = useState("");
  const [configuringKey, setConfiguringKey] = useState<string | null>(null);

  // ACTIVE-08: any reachable connection is inspectable, active or not, so the
  // user can look at another runtime's models before switching to it.
  const availableConnections = useMemo(
    () => connections.filter((c) => c.status === "available"),
    [connections],
  );
  const unavailableConnections = useMemo(
    () => connections.filter((c) => c.status !== "available"),
    [connections],
  );

  useEffect(() => {
    loadDownloadableModels();
    loadActivePair();
  }, [loadDownloadableModels, loadActivePair]);

  useEffect(() => {
    loadAvailableInstalledModels();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [availableConnections.map((c) => c.id).join(",")]);

  useEffect(() => {
    if (!manualConnectionId && availableConnections.length > 0) {
      setManualConnectionId(availableConnections[0].id);
    }
  }, [availableConnections, manualConnectionId]);

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
        {availableConnections.length === 0 && (
          <p className="mt-2 text-sm text-[var(--text-secondary)]">
            {t("connections.noAvailableConnections")}
          </p>
        )}
        <div className="mt-2 space-y-4">
          {availableConnections.map((conn) => {
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
                        activePair.model?.connection_id === conn.id &&
                        activePair.model?.model_name === m.name;
                      const key = `${conn.id}:${m.name}`;
                      const isConfiguring = configuringKey === key;
                      return (
                        <div key={m.name}>
                          <div className="flex items-center justify-between rounded-md border border-[var(--border-color)] px-3 py-1.5">
                            <span className="text-sm">{m.name}</span>
                            <div className="flex items-center gap-2">
                              <button
                                onClick={() => setConfiguringKey(isConfiguring ? null : key)}
                                className="rounded-md p-1 text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-primary)]"
                                title={t("connections.configureModel", { model: m.name })}
                              >
                                <Settings2 size={14} />
                              </button>
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
                          </div>
                          {isConfiguring && (
                            <div className="mt-1">
                              <ModelConfigForm
                                connectionId={conn.id}
                                modelName={m.name}
                                onClose={() => setConfiguringKey(null)}
                              />
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}

          {unavailableConnections.map((conn) => (
            <div key={conn.id}>
              <p className="text-xs font-medium uppercase tracking-wide text-[var(--text-secondary)]">
                {conn.provider} · {conn.base_url}
              </p>
              <p className="mt-1 text-xs text-[var(--text-secondary)]">
                {t("connections.connectionUnavailable")}
              </p>
            </div>
          ))}
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
            const targetConnection = availableConnections.find((c) => c.provider === model.provider);
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
            {availableConnections.map((c) => (
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
