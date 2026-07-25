import { useTranslation } from "react-i18next";
import { Settings } from "lucide-react";
import { useUiStore } from "../../store/uiStore";

export function SettingsSection() {
  const { t } = useTranslation();
  const { activeView, setActiveView } = useUiStore();
  const isActive = activeView === "settings";

  return (
    <div className="border-t border-[var(--border-color)] px-2 py-2">
      <button
        onClick={() => setActiveView("settings")}
        className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm ${
          isActive
            ? "bg-[var(--bg-elevated)] text-[var(--text-primary)]"
            : "text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)]/60 hover:text-[var(--text-primary)]"
        }`}
      >
        <Settings size={14} />
        {t("sidebar.settings")}
      </button>
    </div>
  );
}
