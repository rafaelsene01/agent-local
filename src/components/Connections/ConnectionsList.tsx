import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw, Plus } from "lucide-react";
import { useConnectionsStore } from "../../store/connectionsStore";
import { EmbeddedRuntimeCard } from "./EmbeddedRuntimeCard";
import type { ConnectionProvider } from "../../types";

const STATUS_DOT: Record<string, string> = {
  available: "bg-green-500",
  unavailable: "bg-red-500",
  unknown: "bg-[var(--text-secondary)]",
};

export function ConnectionsList() {
  const { t } = useTranslation();
  const {
    connections,
    isLoading,
    loadConnections,
    setActiveConnection,
    clearActiveConnection,
    refreshConnectionStatus,
    addConnection,
    embeddedStatus,
  } = useConnectionsStore();
  const [provider, setProvider] = useState<ConnectionProvider>("custom");
  const [baseUrl, setBaseUrl] = useState("");
  const [isAdding, setIsAdding] = useState(false);

  async function handleAdd(e: FormEvent) {
    e.preventDefault();
    if (!baseUrl.trim()) return;
    setIsAdding(true);
    try {
      await addConnection(provider, baseUrl.trim());
      setBaseUrl("");
    } finally {
      setIsAdding(false);
    }
  }

  const activeConnection = connections.find((c) => c.is_active);

  return (
    <div className="space-y-6">
      <div className="space-y-2">
        {connections.length > 0 && !activeConnection && (
          <p className="rounded-md border border-dashed border-[var(--border-color)] px-3 py-2 text-xs text-[var(--text-secondary)]">
            {t("connections.noActiveConnection")}
          </p>
        )}
        {activeConnection && activeConnection.status === "unavailable" && (
          <p className="rounded-md border border-red-500/40 px-3 py-2 text-xs text-red-500">
            {t("connections.activeUnavailable")}
          </p>
        )}

        {connections.length === 0 && !isLoading && (
          <div className="rounded-md border border-dashed border-[var(--border-color)] px-4 py-6 text-center text-sm text-[var(--text-secondary)]">
            <p>{t("connections.empty")}</p>
            <button
              onClick={() => loadConnections()}
              className="mt-3 inline-flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)]"
            >
              <RefreshCw size={14} />
              {t("connections.retry")}
            </button>
          </div>
        )}

        {connections.map((conn) => (
          <div
            key={conn.id}
            className={`flex items-center justify-between rounded-md border px-3 py-2 ${
              conn.is_active
                ? "border-[var(--accent)] bg-[var(--bg-elevated)]"
                : "border-[var(--border-color)]"
            }`}
          >
            <div className="flex items-center gap-2">
              <span className={`h-2 w-2 rounded-full ${STATUS_DOT[conn.status]}`} />
              <div>
                <p className="text-sm font-medium capitalize">
                  {conn.provider === "embedded"
                    ? t("connections.providerEmbedded")
                    : conn.provider}
                </p>
                <p className="text-xs text-[var(--text-secondary)]">
                  {conn.provider === "embedded"
                    ? [
                        embeddedStatus?.release_tag &&
                          t("connections.embedded.release", { tag: embeddedStatus.release_tag }),
                        embeddedStatus?.backend === "vulkan"
                          ? t("connections.embedded.backendVulkan")
                          : embeddedStatus?.backend === "cpu"
                            ? t("connections.embedded.backendCpu")
                            : null,
                      ]
                        .filter(Boolean)
                        .join(" · ") || t("connections.embedded.description")
                    : conn.base_url}
                </p>
              </div>
            </div>
            <div className="flex items-center gap-3">
              <button
                onClick={() => refreshConnectionStatus(conn.id)}
                className="rounded-md p-1.5 text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-primary)]"
                title={t("connections.retry")}
              >
                <RefreshCw size={14} />
              </button>
              <label className="flex items-center gap-1.5 text-xs text-[var(--text-secondary)]">
                <input
                  type="radio"
                  name="active-connection"
                  checked={conn.is_active}
                  onChange={() => setActiveConnection(conn.id)}
                />
                {conn.is_active ? t("connections.active") : t("connections.useConnection")}
              </label>
            </div>
          </div>
        ))}

        {activeConnection && (
          <button
            onClick={() => clearActiveConnection()}
            className="text-xs text-[var(--text-secondary)] underline hover:text-[var(--text-primary)]"
          >
            {t("connections.clearActive")}
          </button>
        )}
      </div>

      <EmbeddedRuntimeCard />

      <form onSubmit={handleAdd} className="space-y-2 border-t border-[var(--border-color)] pt-4">
        <h3 className="text-sm font-medium">{t("connections.addManual")}</h3>
        <div className="flex gap-2">
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value as ConnectionProvider)}
            className="rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-2 py-1.5 text-sm"
          >
            <option value="custom">{t("connections.providerCustom")}</option>
            <option value="ollama">{t("connections.providerOllama")}</option>
            <option value="lmstudio">{t("connections.providerLmStudio")}</option>
          </select>
          <input
            type="text"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder={t("connections.baseUrlPlaceholder")}
            className="flex-1 rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-2 py-1.5 text-sm"
          />
          <button
            type="submit"
            disabled={isAdding || !baseUrl.trim()}
            className="flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            <Plus size={14} />
            {t("connections.add")}
          </button>
        </div>
      </form>
    </div>
  );
}
