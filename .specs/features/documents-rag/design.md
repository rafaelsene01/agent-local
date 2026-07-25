# Base de Conhecimento & RAG Global Design

**Spec**: `.specs/features/documents-rag/spec.md`
**Status**: Draft

---

## Architecture Overview

Um pipeline `parse → chunk → embed → store` roda em background (thread separada, não bloqueia a UI) por documento importado, avançando um `status` em SQLite a cada etapa e emitindo eventos Tauri para a UI atualizar em tempo real. O armazenamento vetorial usa **namespaces** desde o início — a base global usa o namespace fixo `"global"`, e o mesmo módulo será reusado por `chat-messaging` (M4) com namespace `"chat:<chat_id>"`, o que já implementa o isolamento exigido por CHAT-11 sem duplicar código.

```mermaid
graph TD
    UI[DocumentsPanel React] -->|invoke import| CMD[document_commands]
    CMD --> QUEUE[Processing Queue<br/>tokio task por documento]
    QUEUE --> PARSE[parsing.rs<br/>extract_text]
    PARSE --> CHUNK[chunking.rs<br/>chunk_text — pure fn]
    CHUNK --> EMBED[embedding.rs<br/>fastembed]
    EMBED --> STORE[(LanceDB<br/>namespace='global')]
    QUEUE -->|status update a cada etapa| DB[(SQLite: documents)]
    QUEUE -->|evento| EVT[Tauri event: document-status]
    EVT --> UI

    RETRIEVE[retrieval.rs<br/>search(namespace, query)] --> STORE
    RETRIEVE -.consumido por M4.-> CHATSVC[Chat context assembly]
```

---

## Code Reuse Analysis

### Existing Components to Leverage

| Component | Location | How to Use |
| --- | --- | --- |
| `DbState` (SQLite) | `src-tauri/src/db.rs` | Nova tabela `documents` na mesma conexão |
| `config::db_path` / pasta-base | `src-tauri/src/config.rs` | Documentos originais vão em `<base_path>/documents/`; LanceDB em `<base_path>/vectors/` (ambos já previstos no `ensure_folder_structure`, AD-008) |
| Padrão nav+painel | `SettingsSection.tsx`/`uiStore.ts` (AD-014) | `DocumentsSection` vira nav item; `DocumentsPanel` some `activeView === "documents"` |
| Evento de progresso | Padrão definido em `connections-models/design.md` (`model-download-progress`) | Mesmo padrão (`emit`/`listen`) para `document-status`, consistência entre features |

### Integration Points

| System | Integration Method |
| --- | --- |
| LanceDB | Crate `lancedb`, tabela local em `<base_path>/vectors/`, uma tabela lógica com coluna `namespace` filtrável (não uma tabela física por namespace — mais simples de gerenciar) |
| fastembed-rs | Carrega o modelo de embedding uma vez (lazy, na primeira chamada) e mantém em memória pelo processo todo |

---

## Components

### `parsing.rs`

- **Purpose**: Extrai texto puro de PDF/DOCX/TXT/MD
- **Location**: `src-tauri/src/rag/parsing.rs`
- **Interfaces**:
  - `fn extract_text(path: &Path) -> Result<String, ParseError>` — despacha por extensão
- **Dependencies**: `pdf-extract` (PDF), `dotext` ou `docx-rs` (DOCX) — **a confirmar exato crate no início da implementação (T1), pesquisando disponibilidade/manutenção atual antes de fixar no `Cargo.toml`**; TXT/MD lidos diretamente via `std::fs::read_to_string`
- **Reuses**: nada

### `chunking.rs`

- **Purpose**: Divide texto extraído em pedaços com overlap, pronto pra embedding
- **Location**: `src-tauri/src/rag/chunking.rs`
- **Interfaces**:
  - `fn chunk_text(text: &str, max_tokens: usize, overlap: usize) -> Vec<TextChunk>` — **função pura, sem I/O**
- **Dependencies**: nenhuma
- **Reuses**: nada
- **Nota**: por ser pura e sem I/O, é o candidato natural a `cargo test` conforme TESTING.md — a task correspondente inclui os testes unitários

### `embedding.rs`

- **Purpose**: Gera vetores de embedding a partir de texto
- **Location**: `src-tauri/src/rag/embedding.rs`
- **Interfaces**:
  - `fn embed_batch(texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>`
- **Dependencies**: `fastembed-rs` (AD-003), modelo ONNX embutido — **modelo específico (multilíngue, já que a UI é EN+PT) a confirmar na T2 antes de baixar/embutir o `.onnx`, verificando quais modelos multilíngues o fastembed-rs suporta atualmente**
- **Reuses**: nada

### `store.rs` (`VectorStore`)

- **Purpose**: Abstrai o LanceDB com namespace, usado por documents-rag (`"global"`) e chat-messaging (`"chat:<id>"`)
- **Location**: `src-tauri/src/rag/store.rs`
- **Interfaces**:
  - `async fn upsert(&self, namespace: &str, doc_id: &str, chunks: Vec<EmbeddedChunk>) -> Result<(), StoreError>`
  - `async fn search(&self, namespace: &str, query_vec: &[f32], top_k: usize) -> Result<Vec<RetrievedChunk>, StoreError>`
  - `async fn delete_by_doc(&self, namespace: &str, doc_id: &str) -> Result<(), StoreError>`
  - `async fn delete_namespace(&self, namespace: &str) -> Result<(), StoreError>`
- **Dependencies**: crate `lancedb`
- **Reuses**: nada — é a fundação nova que M4 vai reusar diretamente (sem reescrever)

### `pipeline.rs`

- **Purpose**: Orquestra parse→chunk→embed→store para um documento, avançando status e emitindo eventos
- **Location**: `src-tauri/src/rag/pipeline.rs`
- **Interfaces**:
  - `async fn process_document(app: AppHandle, db: DbState, doc_id: String, file_path: PathBuf, namespace: String) -> Result<(), PipelineError>`
- **Dependencies**: `parsing`, `chunking`, `embedding`, `store`, `tokio::spawn`
- **Reuses**: todos os módulos acima; M4 chama esta mesma função com `namespace = "chat:<id>"` em vez de duplicar a orquestração

### `document_commands.rs`

- **Purpose**: Comandos Tauri expostos ao frontend
- **Location**: `src-tauri/src/document_commands.rs`
- **Interfaces**:
  - `import_documents(paths: Vec<String>) -> Result<Vec<DocumentRecord>, String>`
  - `list_documents() -> Result<Vec<DocumentRecord>, String>`
  - `delete_document(id: String) -> Result<(), String>`
- **Dependencies**: `pipeline`, `DbState`
- **Reuses**: padrão de `commands.rs`/`config_commands.rs` (M1/M2) — `require_conn`, tratamento de erro como `String`

### `DocumentsPanel` (React)

- **Purpose**: Importar, listar com status, remover
- **Location**: `src/components/Documents/DocumentsPanel.tsx` (+ `DocumentRow.tsx`, `DocumentStatusBadge.tsx`)
- **Interfaces**: consome `documentsStore` (Zustand) + `documentsApi`; escuta evento `document-status` via `@tauri-apps/api/event`
- **Reuses**: padrão visual de `SettingsPanel.tsx`; `useUiStore` (`activeView === "documents"`)

---

## Data Models

### SQLite

```sql
CREATE TABLE documents (
    id TEXT PRIMARY KEY,
    filename TEXT NOT NULL,
    file_path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    status TEXT NOT NULL,          -- 'queued' | 'parsing' | 'chunking' | 'embedding' | 'ready' | 'error'
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### LanceDB (tabela `chunks`, compartilhada entre namespaces)

```typescript
interface StoredChunk {
  id: string;
  namespace: string;   // "global" | "chat:<chat_id>"
  doc_id: string;       // FK lógica para documents.id (global) ou chat_attachments.id (M4)
  text: string;
  vector: number[];     // embedding
  chunk_index: number;
}
```

**Relationships**: `StoredChunk.doc_id` referencia `documents.id` quando `namespace = "global"`. Quando M4 usa `namespace = "chat:<id>"`, referencia `chat_attachments.id` (tabela definida no design de `chat-messaging`) — mesma tabela LanceDB, dono do registro varia por namespace.

---

## Error Handling Strategy

| Error Scenario | Handling | User Impact |
| --- | --- | --- |
| PDF sem texto extraível (só imagem) | `extract_text` retorna erro específico `NoTextFound` | Status "erro" com mensagem "nenhum texto encontrado neste arquivo" |
| Arquivo corrompido/não abre | `extract_text` propaga erro do parser | Status "erro" com a mensagem do parser, documento não entra no RAG |
| Falha no meio do embedding (processo caiu) | Ao reabrir o app, documentos com status "parsing"/"chunking"/"embedding" são reenfileirados do zero | Usuário só vê o processamento recomeçar, sem estado "preso" |
| Remoção durante processamento | Pipeline verifica se o `doc_id` ainda existe em SQLite antes de cada etapa; se não, aborta e limpa chunks parciais do LanceDB | Documento simplesmente some da lista, sem erro residual |

---

## Tech Decisions (only non-obvious ones)

| Decision | Choice | Rationale |
| --- | --- | --- |
| Vetor com namespace em vez de tabela por namespace | Uma tabela LanceDB, coluna `namespace` filtrada em toda query | Menos overhead de gestão (não cria/apaga tabelas dinamicamente por chat); LanceDB indexa bem por coluna |
| Reprocessamento após crash | Reenfileirar do zero (não retomar do meio) | Simplicidade — chunking/embedding é rápido o bastante pra não justificar checkpointing intermediário no v1 |
| Biblioteca de parsing exata (PDF/DOCX) | Não fixada aqui — a task de implementação pesquisa e confirma antes de escrever `Cargo.toml` | Ecossistema Rust de parsing muda; nunca fabricar um nome de crate sem confirmar que existe e está mantido |
| Modelo de embedding exato | Não fixado aqui — a task de implementação confirma quais modelos multilíngues o fastembed-rs suporta atualmente | Mesma razão — evitar fabricar um nome de modelo desatualizado |

---

## Open Questions Carried to Tasks

- **Crate de parsing PDF/DOCX**: a task correspondente deve pesquisar (Context7/web) antes de fixar a dependência
- **Modelo de embedding multilíngue exato do fastembed-rs**: idem — pesquisar na task antes de baixar/embutir
