import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";
import { DocumentStatusBadge } from "./DocumentStatusBadge";
import type { DocumentRecord } from "../../types";

interface Props {
  document: DocumentRecord;
  onRemove: () => void;
}

function formatSize(bytes: number) {
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
  if (bytes >= 1e3) return `${Math.round(bytes / 1e3)} KB`;
  return `${bytes} B`;
}

export function DocumentRow({ document, onRemove }: Props) {
  const { t } = useTranslation();

  return (
    <div className="rounded-md border border-[var(--border-color)] px-3 py-2">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate text-sm font-medium">{document.filename}</p>
          <p className="text-xs text-[var(--text-secondary)]">{formatSize(document.size_bytes)}</p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <DocumentStatusBadge status={document.status} />
          <button
            onClick={onRemove}
            className="rounded-md p-1.5 text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-primary)]"
            title={t("documents.remove")}
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      {document.status === "error" && (
        <p className="mt-1 text-xs text-red-500">
          {document.error_message} {t("documents.retryHint")}
        </p>
      )}
    </div>
  );
}
