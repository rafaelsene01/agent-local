import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { documentsApi } from "../lib/documentsApi";
import type {
  DocumentRecord,
  DocumentStatusEvent,
  RejectedImport,
} from "../types";

interface DocumentsState {
  documents: DocumentRecord[];
  /** Files refused by the last import, kept next to the ones that went in. */
  rejected: RejectedImport[];
  isLoading: boolean;
  isImporting: boolean;
  error: string | null;

  loadDocuments: () => Promise<void>;
  importDocuments: (paths: string[]) => Promise<void>;
  deleteDocument: (id: string) => Promise<void>;
}

export const useDocumentsStore = create<DocumentsState>((set, get) => ({
  documents: [],
  rejected: [],
  isLoading: false,
  isImporting: false,
  error: null,

  loadDocuments: async () => {
    set({ isLoading: true, error: null });
    try {
      const documents = await documentsApi.listDocuments();
      set({ documents, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  // The rows come back as `queued`; the pipeline reports every later step
  // through the `document-status` listener below.
  importDocuments: async (paths) => {
    set({ isImporting: true, error: null, rejected: [] });
    try {
      // A partly valid selection is normal: the accepted files are already
      // queued, the refused ones are reported by name (DOC-03).
      const { rejected } = await documentsApi.importDocuments(paths);
      set({ rejected });
      await get().loadDocuments();
    } catch (err) {
      set({ error: String(err) });
    } finally {
      set({ isImporting: false });
    }
  },

  deleteDocument: async (id) => {
    try {
      await documentsApi.deleteDocument(id);
      set({ documents: get().documents.filter((d) => d.id !== id) });
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));

listen<DocumentStatusEvent>("document-status", (event) => {
  const { id, status, error_message } = event.payload;
  useDocumentsStore.setState((state) => ({
    documents: state.documents.map((doc) =>
      doc.id === id ? { ...doc, status, error_message } : doc,
    ),
  }));
});
