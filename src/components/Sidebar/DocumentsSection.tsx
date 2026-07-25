import { useTranslation } from "react-i18next";
import { FileText } from "lucide-react";

export function DocumentsSection() {
  const { t } = useTranslation();

  return (
    <div className="border-t border-[var(--border-color)] px-3 py-3">
      <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-[var(--text-secondary)]">
        <FileText size={14} />
        {t("documents.title")}
      </div>
      <p className="mt-1.5 text-xs text-[var(--text-secondary)]">{t("documents.placeholder")}</p>
    </div>
  );
}
