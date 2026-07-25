# Chat: Envio, Streaming & Anexos Tasks

**Design**: `.specs/features/chat-messaging/design.md`
**Status**: Complete (2026-07-25) — exceto os itens de T12 que exigem clique na UI, listados abaixo

**Pré-requisito de execução:** esta feature consome componentes de `connections-models` (`ProviderClient`, `model_configs`) e `documents-rag` (`rag::pipeline`, `rag::store`, `rag::parsing`, `rag::embedding`). As tasks abaixo referenciam essas tasks externas explicitamente em **Depends on** — não reimplementam nada, só estendem.

---

## Execution Plan

### Phase 1: Foundation (Parallel — sem dependências entre si)

```
T1 [P] ── DB migrations (chats.*, chat_attachments) + create_message
T2 [P] ── CancellationRegistry
T3 [P] ── OllamaClient::stream_chat      [externo: connections-models T5]
T4 [P] ── LmStudioClient::stream_chat    [externo: connections-models T6]
```

### Phase 2: Assembly & Ingestion (Sequential entre si, paralelas uma à outra)

```
T1 ──┬──→ T5 [P] ContextAssembler        [externo: documents-rag T4, T5]
     └──→ T6 [P] Attachment ingestion    [externo: documents-rag T3, T6]
```

### Phase 3: Orchestration (Sequential)

```
T2, T3, T4, T5, T6 ──→ T7 (send_message + cancel_generation)
T1 ──→ T8 (delete_chat cleanup)          [externo: documents-rag T5]
```

### Phase 4: Frontend (Sequential com ponto paralelo)

```
T7 ──→ T9 (chatApi + chatStore estendidos)
T9 ──┬──→ T10 [P] (MessageInput.tsx)
     └──→ T11 [P] (ChatPanel.tsx modificado + toggle RAG global)
T8, T10, T11 ──→ T12 (integração final)
```

---

## Task Breakdown

### T1: Migrações + `create_message` [P]

**What**: `chats.use_global_rag`, tabela `chat_attachments`; comando `create_message` (M1 só tinha `list_messages`). ~~`chats.model_config_id`~~ **REVOGADO por AD-021** — o modelo vem do par ativo global
**Where**: `src-tauri/src/db.rs` (schema), `src-tauri/src/commands.rs` (novo comando)
**Depends on**: None
**Reuses**: `messages` table e padrão de comandos já existentes (M1)
**Requirement**: CHAT-03, CHAT-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `ALTER TABLE chats ADD COLUMN use_global_rag INTEGER NOT NULL DEFAULT 1` (a coluna `model_config_id` foi **revogada** pela AD-021 — não criar)
- [x] `CREATE TABLE chat_attachments (...)` conforme design.md
- [x] Entra como uma nova entrada em `MIGRATIONS` no `db.rs` (infra versionada da feature `single-active-connection`), não como `execute_batch` avulso
- [x] `create_message(chat_id, role, content) -> Message` insere e retorna a linha
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

---

### T2: `CancellationRegistry` [P]

**What**: Registro de `CancellationToken` por `chat_id`, gerenciado como Tauri state
**Where**: `src-tauri/src/chat/cancellation.rs`
**Depends on**: None
**Reuses**: nada
**Requirement**: CHAT-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `register(chat_id) -> CancellationToken`, `cancel(chat_id)`
- [x] Registrado em `lib.rs` via `app.manage(...)`
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

---

### T3: `OllamaClient::stream_chat` [P]

**What**: Estender `OllamaClient` (connections-models) com streaming de chat
**Where**: `src-tauri/src/providers/ollama.rs` (modificar — adicionar método ao mesmo struct)
**Depends on**: **Externo:** `connections-models` T5 (`OllamaClient` precisa existir)
**Reuses**: `OllamaClient` existente, `ProviderClient` trait (estendido no design desta feature)
**Requirement**: CHAT-01, CHAT-05

**Tools**: MCP: `context7`/web (confirmar formato exato do NDJSON de `/api/chat` com `stream:true` ao implementar o parser) · Skill: NONE

**Done when**:
- [x] `POST /api/chat` com `stream:true`, `options:{num_ctx, num_gpu}`
- [x] Retorna um `Stream` que emite `ChatToken` por linha NDJSON recebida
- [x] Erro de rede/servidor durante o stream propaga como `Err`, não panica
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: com Ollama rodando, chamar `stream_chat` manualmente e ver tokens chegando incrementalmente no log

---

### T4: `LmStudioClient::stream_chat` [P]

**What**: Estender `LmStudioClient` com streaming; recarrega o modelo só se a config pedida diferir da carregada
**Where**: `src-tauri/src/providers/lmstudio.rs` (modificar)
**Depends on**: **Externo:** `connections-models` T6 (`LmStudioClient` precisa existir)
**Reuses**: `LmStudioClient` existente
**Requirement**: CHAT-01, CHAT-05

**Tools**: MCP: `context7`/web (confirmar formato do SSE de `/v1/chat/completions` e como checar a config atualmente carregada antes de decidir se recarrega) · Skill: NONE

**Done when**:
- [x] Antes de gerar: compara `context_length`/`gpu_offload` pedidos com o estado carregado; chama `/api/v1/models/load` só se diferente
- [x] `POST /v1/chat/completions` com `stream:true` (SSE), retorna `Stream<ChatToken>`
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: com LM Studio rodando, chamar `stream_chat` duas vezes seguidas com a mesma config e confirmar (via log) que só recarrega na primeira

---

### T5: `ContextAssembler` [P]

**What**: Monta a lista de mensagens final (system + RAG chat + histórico + RAG global + pergunta), com orçamento de tokens
**Where**: `src-tauri/src/chat/context_assembler.rs`
**Depends on**: T1. **Externo:** `documents-rag` T4 (`rag::embedding`), T5 (`rag::store`)
**Reuses**: `rag::embedding::embed_batch`, `rag::store::VectorStore::search` (documents-rag)
**Requirement**: CHAT-13, CHAT-14, CHAT-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Embeda a pergunta e busca em `namespace="chat:<id>"` e, se `use_global_rag`, em `namespace="global"`
- [x] Aplica a ordem de prioridade do design (mensagem atual > anexos do chat > histórico recente > RAG global), truncando por orçamento em vez de descartar categoria inteira
- [x] Base vazia (nenhum doc pronto) retorna lista de contexto sem erro (DOC-11 já garante isso no `store.search`)
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: com um documento global e um anexo do chat cobrindo tópicos diferentes, montar contexto pra uma pergunta e conferir manualmente que ambos aparecem

---

### T6: Attachment ingestion [P]

**What**: Serializa anexo em `chats/<id>/tmp/`; decide entre injeção inteira (arquivo pequeno) ou pipeline de RAG (`namespace="chat:<id>"`)
**Where**: `src-tauri/src/chat/attachments.rs`
**Depends on**: T1. **Externo:** `documents-rag` T3 (`rag::parsing`), T6 (`rag::pipeline`)
**Reuses**: `rag::parsing::extract_text`, `rag::pipeline::process_document` (documents-rag) — reuso direto, sem duplicar orquestração
**Requirement**: CHAT-06, CHAT-07, CHAT-08, CHAT-09, CHAT-10

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Copia arquivo pra `chats/<chat_id>/tmp/`, cria registro em `chat_attachments` (`status: queued`)
- [x] Extrai texto (reusa `extract_text`) e estima tokens; abaixo do limiar → `status: injected_whole`, texto guardado pra injeção direta
- [x] Acima do limiar → chama `pipeline::process_document(namespace="chat:<id>")`, aguarda conclusão antes de retornar (a pergunta atual precisa se beneficiar — CHAT-08 AC)
- [x] Falha de processamento → `status: error`, função retorna sem abortar o envio da mensagem de texto (CHAT-10)
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: anexar um `.txt` pequeno com um fato inventado e confirmar (manualmente, chamando a função) que ele vira `injected_whole` com o texto completo capturado

---

### T7: `chat_commands::send_message` + `cancel_generation`

**What**: Orquestra tudo — persiste mensagem do usuário, processa anexos, monta contexto, dispara streaming via evento, persiste resposta
**Where**: `src-tauri/src/chat_commands.rs`
**Depends on**: T2, T3, T4, T5, T6
**Reuses**: `create_message` (T1), `ContextAssembler` (T5), attachment ingestion (T6), `ProviderClient::stream_chat` (T3/T4), `CancellationRegistry` (T2)
**Requirement**: CHAT-01, CHAT-02, CHAT-04, CHAT-05

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Sem modelo ativo (chat nem global) → retorna erro antes de qualquer I/O de rede (CHAT-02)
- [x] Persiste mensagem do usuário, processa anexos (T6), monta contexto (T5), chama `stream_chat` do provedor certo
- [x] Cada token recebido emite `chat-stream-chunk {chat_id, message_id, delta, done}`
- [x] Ao terminar (ou cancelar), persiste a mensagem do assistente com o conteúdo acumulado até ali
- [x] `cancel_generation(chat_id)` aciona o `CancellationToken` e a geração para, mantendo o parcial (CHAT-04)
- [x] Erro do provedor durante o stream emite `chat-stream-chunk` com um campo de erro e persiste o parcial (CHAT-05)
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: com um modelo configurado, enviar "conte até 5 devagar" e cancelar no meio; confirmar que a mensagem parcial fica salva no histórico

---

### T8: `delete_chat` — limpeza de anexos e vetor

**What**: Estender o `delete_chat` existente (M1) para apagar `chats/<id>/tmp/` do disco e o namespace `chat:<id>` do vetor
**Where**: `src-tauri/src/commands.rs` (modificar `delete_chat`)
**Depends on**: T1. **Externo:** `documents-rag` T5 (`VectorStore::delete_namespace`)
**Reuses**: `delete_chat` já existente (M1), `VectorStore::delete_namespace` (documents-rag)
**Requirement**: CHAT-12

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Ao excluir um chat, `chats/<chat_id>/tmp/` é removida recursivamente do disco
- [x] `delete_namespace("chat:<chat_id>")` é chamado no LanceDB
- [x] `chat_attachments` daquele chat são removidos (via `DELETE ... WHERE chat_id = ?`, mesmo padrão de `messages` já usado em `delete_chat`)
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: anexar um arquivo a um chat, excluir o chat, confirmar que a pasta `tmp/` daquele chat não existe mais no disco

---

### T9: `chatApi.ts` + `chatStore.ts` estendidos

**What**: Wrappers pra `send_message`/`cancel_generation`; store com estado de streaming (mensagem sendo montada em tempo real) e anexos pendentes
**Where**: `src/lib/chatApi.ts` (modificar), `src/store/chatStore.ts` (modificar)
**Depends on**: T7
**Reuses**: `chatApi.ts`/`chatStore.ts` já existentes desde M1
**Requirement**: CHAT-01 a CHAT-15 (camada de dados do frontend)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `sendMessage(chatId, text, filePaths)` e `cancelGeneration(chatId)` tipados
- [x] Store escuta `chat-stream-chunk` via `@tauri-apps/api/event`, acumula `delta` na mensagem em construção, marca completa quando `done`
- [x] `npm run build` passa

**Tests**: none
**Gate**: build

---

### T10: `MessageInput.tsx` [P]

**What**: Campo de texto + botão de anexo (diálogo nativo de arquivo) + envio; mostra status do(s) anexo(s) antes/durante o envio
**Where**: `src/components/Chat/MessageInput.tsx`
**Depends on**: T9
**Reuses**: `@tauri-apps/plugin-dialog` (já instalado em M2) via `open()` para arquivo em vez de pasta
**Requirement**: CHAT-01, CHAT-02, CHAT-06, CHAT-10

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Campo desabilitado com aviso quando não há modelo ativo (CHAT-02)
- [x] Anexar mostra nome do arquivo + status; erro de anexo aparece sem travar o envio da mensagem de texto
- [x] Enter/botão envia; campo limpa após envio
- [x] `npm run build` passa

**Tests**: none
**Gate**: build

---

### T11: `ChatPanel.tsx` modificado + toggle RAG global [P]

**What**: Renderizar streaming em tempo real; botão de cancelar durante geração; toggle de "usar base global" (CHAT-14) no cabeçalho do chat
**Where**: `src/components/Chat/ChatPanel.tsx` (modificar — já existe desde M1)
**Depends on**: T9
**Reuses**: `ChatPanel.tsx` existente
**Requirement**: CHAT-01, CHAT-04, CHAT-14

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Mensagem do assistente atualiza token a token enquanto `chat-stream-chunk` chega
- [x] Botão de cancelar visível só durante geração ativa, chama `cancelGeneration`
- [x] Toggle de RAG global persiste via um comando novo simples (`set_chat_use_global_rag`, adicionar a `commands.rs` — nota: se esse comando não existir ainda, criar aqui mesmo por ser trivial e ligado 1:1 a este componente)
- [x] `npm run build` passa

**Tests**: none
**Gate**: build

---

### T12: Integração final

**What**: Conferir que `MessageInput` está montado dentro do `ChatPanel`/layout do chat e que o fluxo ponta a ponta funciona
**Where**: `src/components/Chat/ChatPanel.tsx` ou `src/App.tsx` (o que for necessário pra montar `MessageInput`)
**Depends on**: T8, T10, T11
**Reuses**: nada novo — só integração
**Requirement**: Fecha CHAT-01 a CHAT-15 ponta a ponta

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `npm run build` passa
- [ ] `npm run tauri dev`: com um modelo real configurado (via M3), enviar uma mensagem simples e ver streaming funcionando
- [ ] Anexar um `.txt` pequeno com um fato inventado, perguntar sobre ele, confirmar que a resposta reflete o conteúdo
- [ ] Repetir a mesma pergunta (sobre o fato do anexo) em outro chat e confirmar que NÃO usa esse contexto (CHAT-11)
- [ ] Excluir o chat com anexo e confirmar que `chats/<id>/tmp/` some do disco (CHAT-12)

**Tests**: none
**Gate**: full (`npm run tauri dev` + os 4 fluxos manuais acima)

**Commit**: `feat(chat): add message sending, streaming, and per-chat file attachments as isolated RAG`

---

## Parallel Execution Map

```
Phase 1 (Parallel):
  T1 [P] · T2 [P] · T3 [P] · T4 [P]

Phase 2 (Parallel, ambas dependem de T1):
  T1 ──┬── T5 [P]
       └── T6 [P]

Phase 3 (Sequential):
  T2, T3, T4, T5, T6 → T7
  T1 → T8

Phase 4:
  T7 → T9
  T9 ──┬── T10 [P]
       └── T11 [P]
  T8, T10, T11 → T12
```

---

## Task Granularity Check

| Task | Scope | Status |
| --- | --- | --- |
| T1: Migrations + create_message | 1 mudança de schema + 1 comando relacionado | ✅ OK (coeso) |
| T2: CancellationRegistry | 1 componente pequeno | ✅ Granular |
| T3: OllamaClient::stream_chat | 1 método em 1 struct existente | ✅ Granular |
| T4: LmStudioClient::stream_chat | 1 método em 1 struct existente | ✅ Granular |
| T5: ContextAssembler | 1 componente | ✅ Granular |
| T6: Attachment ingestion | 1 componente | ✅ Granular |
| T7: send_message + cancel_generation | 1 arquivo, 2 comandos fortemente acoplados (cancelamento só existe pro streaming de send_message) | ✅ OK (coeso) |
| T8: delete_chat cleanup | 1 função modificada | ✅ Granular |
| T9: API + store frontend | 2 arquivos, 1 conceito | ✅ OK (coeso) |
| T10: MessageInput | 1 componente | ✅ Granular |
| T11: ChatPanel + toggle | 1 componente + 1 toggle diretamente ligado a ele | ✅ OK (coeso) |
| T12: Integração final | 1 verificação ponta a ponta | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| --- | --- | --- | --- |
| T1 | None | Nenhuma seta de entrada | ✅ Match |
| T2 | None | Nenhuma seta de entrada | ✅ Match |
| T3 | Externo (connections-models T5) | Nenhuma seta interna de entrada (dependência externa anotada à parte) | ✅ Match |
| T4 | Externo (connections-models T6) | Nenhuma seta interna de entrada | ✅ Match |
| T5 | T1 + externo (documents-rag T4, T5) | T1 → T5 | ✅ Match |
| T6 | T1 + externo (documents-rag T3, T6) | T1 → T6 | ✅ Match |
| T7 | T2, T3, T4, T5, T6 | T2, T3, T4, T5, T6 → T7 | ✅ Match |
| T8 | T1 + externo (documents-rag T5) | T1 → T8 | ✅ Match |
| T9 | T7 | T7 → T9 | ✅ Match |
| T10 | T9 | T9 → T10 | ✅ Match |
| T11 | T9 | T9 → T11 | ✅ Match |
| T12 | T8, T10, T11 | T8, T10, T11 → T12 | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| --- | --- | --- | --- | --- |
| T1 | Schema + comando Tauri (I/O) | none | none | ✅ OK |
| T2 | Componente de estado compartilhado | none | none | ✅ OK |
| T3 | Provider (I/O HTTP streaming) | none | none | ✅ OK |
| T4 | Provider (I/O HTTP streaming) | none | none | ✅ OK |
| T5 | Orquestração (I/O DB + vetor) | none | none | ✅ OK |
| T6 | Orquestração (I/O arquivo + vetor) | none | none | ✅ OK |
| T7 | Comando Tauri (I/O) | none | none | ✅ OK |
| T8 | Comando Tauri (I/O) | none | none | ✅ OK |
| T9 | Camada de dados React | none | none | ✅ OK |
| T10 | Componente React | none | none | ✅ OK |
| T11 | Componente React | none | none | ✅ OK |
| T12 | Integração | none | none (gate full) | ✅ OK |

---

## MCPs & Skills — Confirmar com o usuário antes de executar

T3 e T4 têm pesquisa recomendada (formato exato de streaming NDJSON/SSE) via `context7`/web. Resto: NONE. Nenhuma task aqui cria lógica pura nova (a lógica pura já foi coberta em `documents-rag` T2/T3, reusada por T6) — por isso não há novo requisito de `cargo test` nesta feature.
