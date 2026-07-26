import { invoke } from "@tauri-apps/api/core";
import type { DocumentRecord, ImportResult } from "../types";

export const documentsApi = {
  importDocuments: (paths: string[]) => invoke<ImportResult>("import_documents", { paths }),
  listDocuments: () => invoke<DocumentRecord[]>("list_documents"),
  deleteDocument: (id: string) => invoke<void>("delete_document", { id }),
};
