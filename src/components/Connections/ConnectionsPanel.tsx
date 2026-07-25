import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft } from "lucide-react";
import { useUiStore } from "../../store/uiStore";
import { ConnectionsList } from "./ConnectionsList";
import { ModelsList } from "./ModelsList";

type Tab = "connections" | "models";

export function ConnectionsPanel() {
  const { t } = useTranslation();
  const setActiveView = useUiStore((s) => s.setActiveView);
  const [tab, setTab] = useState<Tab>("connections");

  return (
    <div className="flex flex-1 flex-col overflow-y-auto bg-[var(--bg-app)] text-[var(--text-primary)]">
      <div className="flex items-center gap-3 border-b border-[var(--border-color)] px-6 py-4">
        <button
          onClick={() => setActiveView("chat")}
          className="rounded-md p-1.5 text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-primary)]"
          title={t("settings.back")}
        >
          <ArrowLeft size={18} />
        </button>
        <h1 className="text-base font-semibold">{t("connections.title")}</h1>
      </div>

      <div className="flex gap-1 border-b border-[var(--border-color)] px-6 pt-3">
        {(["connections", "models"] as const).map((tabOption) => (
          <button
            key={tabOption}
            onClick={() => setTab(tabOption)}
            className={`rounded-t-md px-3 py-2 text-sm font-medium ${
              tab === tabOption
                ? "border-b-2 border-[var(--accent)] text-[var(--text-primary)]"
                : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
            }`}
          >
            {t(tabOption === "connections" ? "connections.tabConnections" : "connections.tabModels")}
          </button>
        ))}
      </div>

      <div className="mx-auto w-full max-w-2xl px-6 py-6">
        {tab === "connections" ? <ConnectionsList /> : <ModelsList />}
      </div>
    </div>
  );
}
