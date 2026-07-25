# Base de Conhecimento & RAG Global Tasks

**Design**: `.specs/features/documents-rag/design.md`
**Status**: Draft

---

## Execution Plan

### Phase 1: Foundation (Parallel — sem dependências entre si)

```
T1 [P] ── DB migration (documents)
T2 [P] ── chunking.rs (pure fn) + unit test
T3 [P] ── parsing.rs (pesquisa + implementação)
T4 [P] ── embedding.rs (pesquisa + implementação)
```

### Phase 2: Vector store (depende de nada novo, mas roda depois pra isolar risco de pesquisa)

```
T5 ── store.rs (VectorStore / LanceDB)
```

### Phase 3: Pipeline & comandos (Sequential)

```
T1, T2, T3, T4, T5 ──→ T6 (pipeline.rs) ──→ T7 (document_commands.rs)
```

### Phase 4: Frontend (Sequential com um ponto paralelo)

```
T7 ──→ T8 (documentsApi + documentsStore)
T8 ──┬──→ T9 [P] (uiStore + DocumentsSection nav)
     └──→ T10 (DocumentsPanel + DocumentRow + StatusBadge) [depende de T9]
T10 ──→ T11 (Wire no App.tsx)
```

---

## Task Breakdown

### T1: Migração SQLite para `documents` [P]

**What**: Adicionar a tabela `documents` ao schema (`db.rs`)
**Where**: `src-tauri/src/db.rs` (modificar `SCHEMA`)
**Depends on**: None
**Reuses**: `db::open()` (M1)
**Requirement**: DOC-02, DOC-04, DOC-05, DOC-06, DOC-08

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `CREATE TABLE IF NOT EXISTS documents (...)` conforme design.md, com `status` cobrindo os 6 valores da máquina de estados
- [ ] `cargo check` passa

**Tests**: none
**Gate**: build

---

### T2: `chunking.rs` (função pura) + teste unitário [P]

**What**: `chunk_text(text, max_tokens, overlap) -> Vec<TextChunk>`
**Where**: `src-tauri/src/rag/chunking.rs`
**Depends on**: None
**Reuses**: nada
**Requirement**: DOC-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Função pura, sem I/O, com overlap configurável
- [ ] Testes cobrem: texto menor que `max_tokens` vira 1 chunk só; texto maior gera múltiplos chunks com overlap correto; texto vazio retorna lista vazia sem panicar
- [ ] `cargo test chunking` passa

**Tests**: unit
**Gate**: `cargo test chunking`

**Verify**: `cargo test chunking -- --nocapture`

---

### T3: `parsing.rs` — extração de texto (PDF/DOCX/TXT/MD) [P]

**What**: `extract_text(path: &Path) -> Result<String, ParseError>`, despachando por extensão
**Where**: `src-tauri/src/rag/parsing.rs`
**Depends on**: None
**Reuses**: nada
**Requirement**: DOC-04, DOC-06

**Tools**: MCP: `context7`/web search (**obrigatório pesquisar antes de escrever**: confirmar crates Rust atuais e mantidos para extrair texto de PDF e DOCX — não fabricar nome de crate; se nenhum crate confiável for encontrado para algum formato, documentar como limitação conhecida em vez de inventar suporte) · Skill: NONE

**Done when**:
- [ ] TXT/MD lidos via `std::fs::read_to_string`
- [ ] PDF e DOCX extraem texto usando crates confirmados pela pesquisa (nomes documentados no commit/PR)
- [ ] Arquivo sem texto extraível (ex.: PDF só-imagem) retorna `ParseError::NoTextFound`, não panic
- [ ] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: rodar `extract_text` manualmente contra um PDF real e um DOCX real de teste, confirmar texto não vazio

---

### T4: `embedding.rs` — geração de embeddings [P]

**What**: `embed_batch(texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>` via fastembed-rs
**Where**: `src-tauri/src/rag/embedding.rs`
**Depends on**: None
**Reuses**: nada
**Requirement**: DOC-04

**Tools**: MCP: `context7`/web search (**obrigatório**: confirmar qual(is) modelo(s) multilíngue(s) o fastembed-rs suporta atualmente antes de fixar um nome de modelo no código — a UI é EN+PT, o modelo de embedding precisa cobrir os dois) · Skill: NONE

**Done when**:
- [ ] Modelo de embedding carregado de forma lazy (primeira chamada), mantido em memória
- [ ] `embed_batch` retorna vetores de dimensão consistente
- [ ] Decisão do modelo escolhido documentada como comentário/AD (nome do modelo confirmado via pesquisa, não fabricado)
- [ ] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: embeddar duas frases parecidas e duas diferentes, confirmar (manualmente/print de similaridade) que as parecidas têm distância menor

---

### T5: `store.rs` (`VectorStore` sobre LanceDB)

**What**: `upsert`, `search`, `delete_by_doc`, `delete_namespace` com coluna `namespace`
**Where**: `src-tauri/src/rag/store.rs`
**Depends on**: None (roda em Phase 2 só por isolamento de risco de pesquisa, não por dependência real)
**Reuses**: nada — é a base que `chat-messaging` vai reusar depois
**Requirement**: DOC-10, DOC-11, DOC-12

**Tools**: MCP: `context7`/web search (**obrigatório**: confirmar API atual do crate `lancedb` para Rust — criação de tabela, add de linhas com vetor, query com filtro de coluna + top-k por similaridade, delete por filtro) · Skill: NONE

**Done when**:
- [ ] Tabela LanceDB criada em `<base_path>/vectors/` na primeira escrita
- [ ] `search(namespace, query_vec, top_k)` filtra por `namespace` e ordena por similaridade
- [ ] `delete_by_doc`/`delete_namespace` removem linhas corretamente (verificado por count antes/depois)
- [ ] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: upsert de chunks em dois namespaces diferentes, `search("a", ...)` não retorna nada de `"b"`

---

### T6: `pipeline.rs` — orquestração + eventos de progresso

**What**: `process_document(app, db, doc_id, file_path, namespace)` avançando status e emitindo `document-status`
**Where**: `src-tauri/src/rag/pipeline.rs`
**Depends on**: T1, T2, T3, T4, T5
**Reuses**: todos os módulos de `rag/` (T2-T5)
**Requirement**: DOC-04, DOC-05, DOC-06, DOC-07

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Avança `queued → parsing → chunking → embedding → ready` em SQLite a cada etapa, emitindo evento a cada mudança
- [ ] Erro em qualquer etapa vira status `error` com `error_message` preenchido, sem panicar o processo
- [ ] Múltiplos documentos processam via `tokio::spawn` sem travar a thread principal/UI
- [ ] Documento removido durante processamento é detectado (checagem antes de cada etapa) e aborta limpando chunks parciais
- [ ] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: importar 2 documentos ao mesmo tempo via `npm run tauri dev`, ver ambos progredindo nos logs/eventos sem travar a janela

---

### T7: `document_commands.rs` + reenfileiramento na inicialização

**What**: Comandos `import_documents`, `list_documents`, `delete_document`; e retomada de documentos "presos" ao iniciar o app
**Where**: `src-tauri/src/document_commands.rs`, `src-tauri/src/lib.rs` (modificar `setup`)
**Depends on**: T6
**Reuses**: `require_conn`, padrão de `commands.rs` (M1); hook de `setup()` já existente (M2, onde `DbState` é inicializado)
**Requirement**: DOC-01, DOC-02, DOC-03, DOC-08, DOC-09

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `import_documents(paths)` copia pra `documents/`, valida extensão/tamanho (rejeita com erro claro se inválido), cria registro `queued`, dispara `pipeline::process_document`
- [ ] `list_documents()` / `delete_document(id)` (apaga arquivo + registro + `delete_by_doc` do vetor)
- [ ] No `setup()`, após abrir a DB (config já completa), documentos com status `queued`/`parsing`/`chunking`/`embedding` são reenfileirados do zero
- [ ] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: importar um documento real, ver ele chegar a "ready"; matar o processo no meio do processamento de outro e reabrir — ele reinicia do zero, não fica preso

---

### T8: `documentsApi.ts` + `documentsStore.ts`

**What**: Wrappers `invoke` tipados + store Zustand (lista, status em tempo real via evento)
**Where**: `src/lib/documentsApi.ts`, `src/store/documentsStore.ts`
**Depends on**: T7
**Reuses**: padrão de `chatApi.ts`/`configApi.ts`
**Requirement**: DOC-01 a DOC-12 (camada de dados do frontend)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Wrappers para os 3 comandos de T7
- [ ] Store escuta `document-status` via `@tauri-apps/api/event` e atualiza o item certo pelo `id`
- [ ] `npm run build` passa

**Tests**: none
**Gate**: build

---

### T9: `uiStore` estendido + `DocumentsSection.tsx` (nav) [P]

**What**: Adicionar `"documents"` ao `ActiveView`; converter placeholder em item de navegação (padrão AD-014)
**Where**: `src/store/uiStore.ts` (modificar), `src/components/Sidebar/DocumentsSection.tsx` (reescrever)
**Depends on**: T8
**Reuses**: `SettingsSection.tsx` como referência de padrão
**Requirement**: DOC-01 (ponto de entrada da UI)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `ActiveView` inclui `"documents"`
- [ ] i18n: chaves novas em `en.json`/`pt.json`
- [ ] `npm run build` passa

**Tests**: none
**Gate**: build

---

### T10: `DocumentsPanel.tsx` + `DocumentRow.tsx` + `DocumentStatusBadge.tsx`

**What**: Importar (seletor nativo), listar com status visual, remover
**Where**: `src/components/Documents/DocumentsPanel.tsx`, `DocumentRow.tsx`, `DocumentStatusBadge.tsx`
**Depends on**: T9
**Reuses**: `SettingsPanel.tsx` (layout, header com voltar), `@tauri-apps/plugin-dialog` (já instalado em M2, usar `open()` para arquivo em vez de `pick_folder`)
**Requirement**: DOC-01, DOC-02, DOC-03, DOC-05, DOC-06, DOC-08, DOC-09

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Botão importar abre seletor nativo filtrado por extensão
- [ ] Lista mostra nome, tamanho, status (com indicador visual por etapa)
- [ ] Documento "erro" mostra a mensagem; documento "ready" tem opção de remover
- [ ] `npm run build` passa

**Tests**: none
**Gate**: build

---

### T11: Roteamento no `App.tsx`

**What**: Renderizar `DocumentsPanel` quando `activeView === "documents"`
**Where**: `src/App.tsx` (modificar)
**Depends on**: T10
**Reuses**: mesmo padrão de `activeView === "settings"`/`"connections"`
**Requirement**: DOC-01 (fecha o fluxo ponta a ponta)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `npm run build` passa
- [ ] `npm run tauri dev`: importar um PDF/TXT real, ver progresso até "ready", remover e confirmar que some da lista

**Tests**: none
**Gate**: full (`npm run tauri dev` até `Finished`+`Running` sem erro)

**Commit**: `feat(documents): add document import, background processing pipeline and global RAG retrieval`

---

## Parallel Execution Map

```
Phase 1 (Parallel):
  T1 [P] · T2 [P] · T3 [P] · T4 [P]

Phase 2:
  T5 (isolado por risco de pesquisa, sem dependência real)

Phase 3 (Sequential):
  T1, T2, T3, T4, T5 → T6 → T7

Phase 4:
  T7 → T8 → T9 [P] → T10 → T11
```

---

## Task Granularity Check

| Task | Scope | Status |
| --- | --- | --- |
| T1: DB migration | 1 tabela | ✅ Granular |
| T2: chunking.rs | 1 função pura | ✅ Granular |
| T3: parsing.rs | 1 função, múltiplos formatos coesos | ✅ OK (coeso) |
| T4: embedding.rs | 1 função | ✅ Granular |
| T5: store.rs | 1 componente, 4 métodos coesos | ✅ OK (coeso) |
| T6: pipeline.rs | 1 função de orquestração | ✅ Granular |
| T7: document_commands + requeue | 1 arquivo + 1 hook de setup, mesmo conceito (ciclo de vida do documento) | ✅ OK (coeso) |
| T8: API + store frontend | 2 arquivos, 1 conceito | ✅ OK (coeso) |
| T9: uiStore + nav | 2 arquivos pequenos | ✅ OK (coeso) |
| T10: Painel + row + badge | 3 componentes de uma tela | ✅ OK (coeso) |
| T11: Roteamento | 1 mudança em 1 arquivo | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| --- | --- | --- | --- |
| T1 | None | Nenhuma seta de entrada | ✅ Match |
| T2 | None | Nenhuma seta de entrada | ✅ Match |
| T3 | None | Nenhuma seta de entrada | ✅ Match |
| T4 | None | Nenhuma seta de entrada | ✅ Match |
| T5 | None | Nenhuma seta de entrada | ✅ Match |
| T6 | T1, T2, T3, T4, T5 | T1, T2, T3, T4, T5 → T6 | ✅ Match |
| T7 | T6 | T6 → T7 | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |
| T9 | T8 | T8 → T9 | ✅ Match |
| T10 | T9 | T9 → T10 | ✅ Match |
| T11 | T10 | T10 → T11 | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| --- | --- | --- | --- | --- |
| T1 | Schema SQLite (I/O) | none | none | ✅ OK |
| T2 | Função pura Rust | unit | unit | ✅ OK |
| T3 | Parsing (I/O de arquivo) | none | none | ✅ OK |
| T4 | Embedding (carrega modelo, I/O) | none | none | ✅ OK |
| T5 | VectorStore (I/O LanceDB) | none | none | ✅ OK |
| T6 | Orquestração (I/O) | none | none | ✅ OK |
| T7 | Comando Tauri (I/O) | none | none | ✅ OK |
| T8 | Camada de dados React | none | none | ✅ OK |
| T9 | Componente React | none | none | ✅ OK |
| T10 | Componente React | none | none | ✅ OK |
| T11 | Integração | none | none (gate full) | ✅ OK |

---

## MCPs & Skills — Confirmar com o usuário antes de executar

T3, T4 e T5 têm pesquisa **obrigatória** marcada (crates de parsing PDF/DOCX, modelo multilíngue do fastembed-rs, API do crate `lancedb`) — usar `context7` primeiro, web search como fallback, e nunca fabricar nome de crate/modelo. Resto: NONE.
