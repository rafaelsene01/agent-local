import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, Upload } from "lucide-react";
import { useUiStore } from "../../store/uiStore";
import { useDocumentsStore } from "../../store/documentsStore";
import { DocumentRow } from "./DocumentRow";

export function DocumentsPanel() {
  const { t } = useTranslation();
  const setActiveView = useUiStore((s) => s.setActiveView);
  const {
    documents,
    rejected,
    isImporting,
    error,
    loadDocuments,
    importDocuments,
    deleteDocument,
  } = useDocumentsStore();

  useEffect(() => {
    loadDocuments();
  }, [loadDocuments]);

  async function handleImport() {
    const selected = await open({
      multiple: true,
      title: t("documents.fileDialogTitle"),
      filters: [{ name: t("documents.supportedFormats"), extensions: ["pdf", "docx", "txt", "md"] }],
    });
    if (!selected) return;
    await importDocuments(Array.isArray(selected) ? selected : [selected]);
  }

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
        <h1 className="text-base font-semibold">{t("documents.title")}</h1>
      </div>

      <div className="mx-auto w-full max-w-2xl px-6 py-6">
        <p className="text-sm text-[var(--text-secondary)]">{t("documents.panelDescription")}</p>

        <button
          onClick={handleImport}
          disabled={isImporting}
          className="mt-4 flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          <Upload size={14} />
          {isImporting ? t("documents.importing") : t("documents.import")}
        </button>
        <p className="mt-1 text-xs text-[var(--text-secondary)]">
          {t("documents.supportedFormats")}
        </p>

        {error && <p className="mt-3 text-xs text-red-500">{error}</p>}

        {rejected.map((item) => (
          <p key={item.path} className="mt-2 text-xs text-amber-500">
            {t("documents.rejected", {
              name: item.path.split(/[\\/]/).pop() ?? item.path,
              reason: item.reason,
            })}
          </p>
        ))}

        <div className="mt-6 space-y-2">
          {documents.length === 0 ? (
            <p className="text-sm text-[var(--text-secondary)]">{t("documents.empty")}</p>
          ) : (
            documents.map((doc) => (
              <DocumentRow
                key={doc.id}
                document={doc}
                onRemove={() => deleteDocument(doc.id)}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
