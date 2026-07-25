import { useTranslation } from "react-i18next";
import { Plug, Circle } from "lucide-react";

export function ConnectionsSection() {
  const { t } = useTranslation();

  return (
    <div className="border-t border-[var(--border-color)] px-3 py-3">
      <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-[var(--text-secondary)]">
        <Plug size={14} />
        {t("connections.title")}
      </div>
      <div className="mt-1.5 flex items-center gap-2 text-xs text-[var(--text-secondary)]">
        <Circle size={8} className="fill-[var(--text-secondary)] text-[var(--text-secondary)]" />
        {t("connections.placeholder")}
      </div>
    </div>
  );
}
