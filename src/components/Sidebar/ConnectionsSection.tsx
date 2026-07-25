import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Plug } from "lucide-react";
import { useUiStore } from "../../store/uiStore";
import { useConnectionsStore } from "../../store/connectionsStore";

export function ConnectionsSection() {
  const { t } = useTranslation();
  const { activeView, setActiveView } = useUiStore();
  const { connections, loadConnections } = useConnectionsStore();
  const isActive = activeView === "connections";

  useEffect(() => {
    loadConnections();
  }, [loadConnections]);

  const hasAvailableEnabled = connections.some((c) => c.enabled && c.status === "available");
  const hasEnabled = connections.some((c) => c.enabled);
  const statusKey = hasAvailableEnabled ? "available" : hasEnabled ? "unavailable" : "none";
  const dotColor = hasAvailableEnabled
    ? "bg-green-500"
    : hasEnabled
      ? "bg-red-500"
      : "bg-[var(--text-secondary)]";

  return (
    <div className="border-t border-[var(--border-color)] px-2 py-2">
      <button
        onClick={() => setActiveView("connections")}
        className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm ${
          isActive
            ? "bg-[var(--bg-elevated)] text-[var(--text-primary)]"
            : "text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)]/60 hover:text-[var(--text-primary)]"
        }`}
      >
        <Plug size={14} />
        <span className="flex-1 text-left">{t("sidebar.connections")}</span>
        <span
          className={`h-2 w-2 rounded-full ${dotColor}`}
          title={t(`connections.status.${statusKey}`)}
        />
      </button>
    </div>
  );
}
