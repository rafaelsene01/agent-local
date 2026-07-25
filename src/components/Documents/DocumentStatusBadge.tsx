import { useTranslation } from "react-i18next";
import type { DocumentStatus } from "../../types";

const STATUS_STYLE: Record<DocumentStatus, string> = {
  queued: "bg-[var(--bg-elevated)] text-[var(--text-secondary)]",
  parsing: "bg-amber-500/20 text-amber-500",
  chunking: "bg-amber-500/20 text-amber-500",
  embedding: "bg-amber-500/20 text-amber-500",
  ready: "bg-green-500/20 text-green-500",
  error: "bg-red-500/20 text-red-500",
};

const STATUS_LABEL_KEY: Record<DocumentStatus, string> = {
  queued: "documents.statusQueued",
  parsing: "documents.statusParsing",
  chunking: "documents.statusChunking",
  embedding: "documents.statusEmbedding",
  ready: "documents.statusReady",
  error: "documents.statusError",
};

export function DocumentStatusBadge({ status }: { status: DocumentStatus }) {
  const { t } = useTranslation();
  const isProcessing = status !== "ready" && status !== "error";

  return (
    <span
      className={`inline-flex shrink-0 items-center gap-1.5 rounded-full px-2 py-0.5 text-xs ${STATUS_STYLE[status]}`}
    >
      {isProcessing && <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-current" />}
      {t(STATUS_LABEL_KEY[status])}
    </span>
  );
}
