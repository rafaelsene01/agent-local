import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { FileText } from "lucide-react";
import { useUiStore } from "../../store/uiStore";
import { useDocumentsStore } from "../../store/documentsStore";

export function DocumentsSection() {
  const { t } = useTranslation();
  const { activeView, setActiveView } = useUiStore();
  const { documents, loadDocuments } = useDocumentsStore();
  const isActive = activeView === "documents";

  useEffect(() => {
    loadDocuments();
  }, [loadDocuments]);

  // Only `ready` documents answer questions, so that is the count worth
  // showing next to the section.
  const readyCount = documents.filter((d) => d.status === "ready").length;

  return (
    <div className="border-t border-[var(--border-color)] px-2 py-2">
      <button
        onClick={() => setActiveView("documents")}
        className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm ${
          isActive
            ? "bg-[var(--bg-elevated)] text-[var(--text-primary)]"
            : "text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)]/60 hover:text-[var(--text-primary)]"
        }`}
      >
        <FileText size={14} />
        <span className="flex-1 text-left">{t("sidebar.documents")}</span>
        {readyCount > 0 && (
          <span className="rounded-full bg-[var(--bg-elevated)] px-1.5 text-xs text-[var(--text-secondary)]">
            {readyCount}
          </span>
        )}
      </button>
    </div>
  );
}
