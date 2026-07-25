# Conexão e Modelo Ativos Únicos — Tasks

**Spec**: `.specs/features/single-active-connection/spec.md`
**Design**: inline (escopo Medium — sem `design.md`; as decisões de arquitetura estão abaixo e nas próprias tasks)
**Status**: Draft

---

## Decisões de design (inline)

| Decisão | Escolha | Motivo |
| --- | --- | --- |
| Como versionar migração | `PRAGMA user_version` + slice ordenado `&[(u32, &str)]` em `db.rs` | Zero dependência nova; é o mecanismo nativo do SQLite. Resolve C-01 |
| Renomear coluna ou nova coluna | `ALTER TABLE connections RENAME COLUMN enabled TO is_active` | SQLite suporta desde 3.25 (o `rusqlite` com feature `bundled` traz versão muito posterior). Preserva os dados; alternativa (nova coluna + copy + drop) seria mais código pelo mesmo efeito |
| Onde mora a invariante "só um ativo" | No backend, dentro de uma transação, nunca na UI | A UI é um cliente; a garantia tem que valer independente de quem chama o comando |
| Uma ação ou duas para ativar par | Uma: `set_active_model(connection_id, model_name)` ativa os dois | ACTIVE-05/06 — evita janela de inconsistência entre duas chamadas |
| O que substitui `toggle_connection` | `set_active_connection(id)` + `clear_active_connection()` | `toggle(id, bool)` não expressa exclusividade; a assinatura nova torna o estado ilegal inexprimível |

---

## Execution Plan

```
Phase 1 (Sequential — schema é base de tudo)
  T1 (infra de migração) ──→ T2 (migração connections.is_active)

Phase 2 (Sequential — backend sobre o schema novo)
  T2 ──→ T3 (connections.rs: set/clear/get active)
  T3 ──→ T4 (connection_commands.rs)
  T3 ──→ T5 (model_commands.rs: par ativo)

Phase 3 (Frontend — depende de T4 e T5)
  T4, T5 ──→ T6 (types.ts + connectionsApi + connectionsStore)
  T6 ──┬──→ T7 [P] (ConnectionsList: radio)
       └──→ T8 [P] (ModelsList: agrupado por conexão, ação única)
  T7, T8 ──→ T9 (ConnectionsSection + gate full)

Phase 4 (Docs — sem código)
  T9 ──→ T10 (revogar AD-016 no chat-messaging design + STATE.md)
```

---

## Task Breakdown

### T1: Infraestrutura de migração versionada

**What**: Trocar o `execute_batch(SCHEMA)` único por um mecanismo que lê `PRAGMA user_version`, aplica só as migrações acima da versão atual e grava a nova versão
**Where**: `src-tauri/src/db.rs` (modificar)
**Depends on**: None
**Reuses**: a `const SCHEMA` atual vira a migração de versão 1 (sem reescrevê-la)
**Requirement**: ACTIVE-09

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `const MIGRATIONS: &[(u32, &str)]` com a migração 1 = conteúdo atual de `SCHEMA`
- [ ] `db::open()` lê `PRAGMA user_version`, aplica em ordem só as migrações com versão maior, cada uma dentro de uma transação, e atualiza `PRAGMA user_version` ao final
- [ ] Banco novo (in-memory) chega a `user_version = 1` com todas as tabelas do M1-M3
- [ ] Rodar `open()` duas vezes sobre a mesma conexão não reaplica nada (idempotente)
- [ ] Gate check passa: `cd src-tauri && cargo test db::`
- [ ] Test count: 3 testes passam (o teste existente `open_creates_connections_and_model_configs_tables` + 2 novos)

**Tests**: unit
**Gate**: quick

**Verify**: `cargo test db:: -- --nocapture`

---

### T2: Migração 2 — `connections.enabled` vira `is_active` com invariante

**What**: Adicionar a migração de versão 2 que renomeia a coluna e normaliza múltiplos habilitados para no máximo um ativo
**Where**: `src-tauri/src/db.rs` (adicionar entrada em `MIGRATIONS`)
**Depends on**: T1
**Reuses**: mecanismo de migração de T1
**Requirement**: ACTIVE-09, ACTIVE-10

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Migração 2 faz `ALTER TABLE connections RENAME COLUMN enabled TO is_active`
- [ ] Migração 2 normaliza: se mais de uma linha tem `is_active = 1`, mantém só a de `created_at` mais antigo e zera as demais
- [ ] Teste: banco no schema v1 com 2 conexões habilitadas → após `open()`, exatamente 1 ativa e `user_version = 2`
- [ ] Teste: banco criado do zero chega em `user_version = 2` com a coluna já chamada `is_active`
- [ ] Gate check passa: `cd src-tauri && cargo test db::`
- [ ] Test count: 5 testes passam

**Tests**: unit
**Gate**: quick

**Verify**: `cargo test db:: -- --nocapture`

---

### T3: `connections.rs` — ativar/limpar/consultar conexão ativa

**What**: Substituir `toggle_connection` por operações que garantem exclusividade dentro de transação
**Where**: `src-tauri/src/connections.rs` (modificar)
**Depends on**: T2
**Reuses**: `create_connection`/`list_connections` já existentes; padrão de transação de `commands::delete_chat`
**Requirement**: ACTIVE-01, ACTIVE-02, ACTIVE-05, ACTIVE-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `set_active_connection(sql, id)` zera todas e ativa a informada, **numa transação**
- [ ] `clear_active_connection(sql)` zera todas as conexões **e** todos os `model_configs.is_active` (edge case: modelo ativo sem conexão)
- [ ] `active_connection(sql) -> Option<Connection>` devolve a ativa ou `None`
- [ ] `toggle_connection` removida (nenhum caller restante compila com ela)
- [ ] Campo `Connection.enabled` renomeado para `is_active`
- [ ] Teste: ativar A, depois ativar B → só B ativa
- [ ] Teste: `clear_active_connection` zera conexão e modelo juntos
- [ ] Gate check passa: `cd src-tauri && cargo test connections::`
- [ ] Test count: 4 testes passam (2 existentes adaptados + 2 novos)

**Tests**: unit
**Gate**: quick

---

### T4: `connection_commands.rs` — comandos de conexão ativa

**What**: Expor as operações de T3 como comandos Tauri, substituindo `toggle_connection`
**Where**: `src-tauri/src/connection_commands.rs` (modificar), `src-tauri/src/lib.rs` (registro)
**Depends on**: T3
**Reuses**: `require_conn` local, padrão dos comandos existentes
**Requirement**: ACTIVE-01, ACTIVE-02, ACTIVE-03, ACTIVE-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `set_active_connection(id)` e `clear_active_connection()` registrados em `lib.rs`; `toggle_connection` removido do `invoke_handler!`
- [ ] `list_connections` continua devolvendo **todas** as conexões com status calculado (ACTIVE-03) — semeadura de candidatos agora insere com `is_active = 0`
- [ ] Ativar conexão indisponível não é bloqueado (ACTIVE-04) — o status apenas reflete a realidade
- [ ] Gate check passa: `cd src-tauri && cargo check`

**Tests**: none (comando Tauri de orquestração I/O — matriz do TESTING.md diz "none")
**Gate**: build

---

### T5: `model_commands.rs` — par ativo consistente

**What**: `set_active_model` passa a ativar também a conexão dona; `get_active_model` vira `get_active_pair` devolvendo conexão + modelo
**Where**: `src-tauri/src/model_commands.rs` (modificar), `src-tauri/src/lib.rs` (registro)
**Depends on**: T3
**Reuses**: `get_or_create_model_config` já existente; `connections::set_active_connection` (T3)
**Requirement**: ACTIVE-05, ACTIVE-06, ACTIVE-07

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `set_active_model(connection_id, model_name)` ativa modelo **e** conexão na mesma transação
- [ ] `get_active_pair()` devolve `{ connection, model }` ou `None` num único retorno (substitui `get_active_model`)
- [ ] Trocar de conexão ativa via `set_active_connection` (T3) zera o modelo ativo se ele não pertencer à nova conexão (invariante ACTIVE-06)
- [ ] `list_installed_models` deixa de exigir conexão ativa — funciona para qualquer conexão informada (ACTIVE-08)
- [ ] Gate check passa: `cd src-tauri && cargo check`

**Tests**: none (comando Tauri de orquestração I/O)
**Gate**: build

---

### T6: Frontend — tipos, API e store

**What**: Refletir a nova superfície de comandos na camada de dados do frontend
**Where**: `src/types.ts`, `src/lib/connectionsApi.ts`, `src/store/connectionsStore.ts` (todos modificar)
**Depends on**: T4, T5
**Reuses**: padrão dos wrappers/store já existentes
**Requirement**: ACTIVE-01, ACTIVE-02, ACTIVE-07, ACTIVE-08

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `Connection.enabled` vira `is_active`; `ActiveModel` substituído por `ActivePair { connection, model }`
- [ ] `connectionsApi`: `toggleConnection` removido; `setActiveConnection`, `clearActiveConnection`, `getActivePair` adicionados
- [ ] Store: `activeModel` vira `activePair`; ações correspondentes; após ativar conexão ou modelo, recarrega o par (evita estado divergente)
- [ ] Store carrega modelos instalados de **todas** as conexões com status "available", não só da ativa (ACTIVE-08)
- [ ] Gate check passa: `npm run build`

**Tests**: none (matriz do TESTING.md: componentes/stores React = "none" por ora — ver C-04)
**Gate**: build

---

### T7: `ConnectionsList` — radio em vez de checkbox [P]

**What**: Trocar o checkbox "habilitada" por seleção exclusiva, com a ativa destacada e um botão para limpar a seleção
**Where**: `src/components/Connections/ConnectionsList.tsx` (modificar), `src/i18n/locales/{en,pt}.json`
**Depends on**: T6
**Reuses**: layout/estilo atual do componente; CSS vars de tema
**Requirement**: ACTIVE-01, ACTIVE-02, ACTIVE-03, ACTIVE-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `<input type="radio" name="active-connection">` por linha, marcado apenas na ativa
- [ ] Todas as conexões continuam listadas com bolinha de status (ACTIVE-03)
- [ ] Estado "nenhuma ativa" é visível (ex.: aviso no topo) e alcançável por um botão de limpar (ACTIVE-02)
- [ ] Ativar uma indisponível funciona, exibindo aviso de que ela não está respondendo (ACTIVE-04)
- [ ] Toda string nova tem chave i18n em `en.json` **e** `pt.json`
- [ ] Gate check passa: `npm run build`

**Tests**: none
**Gate**: build

---

### T8: `ModelsList` — agrupado por conexão, ação única [P]

**What**: Listar modelos de todas as conexões disponíveis; escolher um modelo ativa o par (modelo + conexão) numa ação
**Where**: `src/components/Connections/ModelsList.tsx` (modificar), `src/i18n/locales/{en,pt}.json`
**Depends on**: T6
**Reuses**: estrutura de agrupamento por conexão já existente no componente
**Requirement**: ACTIVE-05, ACTIVE-06, ACTIVE-08

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Agrupa por **todas** as conexões com status "available" (não filtra por ativa) — ACTIVE-08
- [ ] Conexão indisponível mostra aviso no lugar da sua lista, sem quebrar as outras
- [ ] Botão "usar este modelo" ativa modelo + conexão numa chamada; o modelo ativo aparece marcado e é o único marcado em toda a tela
- [ ] Modelo ativo que não pertence mais à conexão ativa nunca aparece marcado
- [ ] Toda string nova tem chave i18n em `en.json` **e** `pt.json`
- [ ] Gate check passa: `npm run build`

**Tests**: none
**Gate**: build

---

### T9: `ConnectionsSection` — indicador do par ativo + verificação ponta a ponta

**What**: A bolinha da sidebar passa a refletir a **conexão ativa** (não "alguma habilitada"), e o app é verificado rodando
**Where**: `src/components/Sidebar/ConnectionsSection.tsx` (modificar), `src/i18n/locales/{en,pt}.json`
**Depends on**: T7, T8
**Reuses**: componente atual
**Requirement**: ACTIVE-02, ACTIVE-03

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Bolinha: verde = ativa e disponível, vermelha = ativa e indisponível, cinza = nenhuma ativa
- [ ] Tooltip nomeia a conexão ativa (ou "nenhuma")
- [ ] Gate check passa: `npm run build` **e** `npm run tauri dev` sobe até `Finished` + `Running` sem erro
- [ ] Verificação manual na UI: ativar Ollama → ativar LM Studio → confirmar que só a última fica marcada; escolher um modelo da outra conexão → confirmar que a conexão ativa acompanhou

**Tests**: none
**Gate**: full

**Commit**: `feat(connections): enforce a single active connection and model`

---

### T10: Revogar a AD-016 na documentação

**What**: Atualizar o design de `chat-messaging` e o STATE.md para refletir que não existe mais modelo por chat
**Where**: `.specs/features/chat-messaging/design.md`, `.specs/features/chat-messaging/tasks.md` (se referenciarem `model_config_id`), `.specs/project/STATE.md`
**Depends on**: T9
**Reuses**: —
**Requirement**: rastreabilidade (nenhum ACTIVE-*, é doc)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Nenhuma menção a `chats.model_config_id` sobrevive no design/tasks de `chat-messaging` sem estar marcada como revogada
- [ ] STATE.md: AD-016 marcada como **REVOGADA** com data e motivo, e nova AD registrando a regra de par ativo único
- [ ] `grep -ri "model_config_id" .specs/features/chat-messaging` não retorna nada não-marcado

**Tests**: none
**Gate**: none (só documentação)

**Commit**: `docs(connections): revoke AD-016 in favor of a single global active pair`

---

## Task Granularity Check

| Task | Scope | Status |
| --- | --- | --- |
| T1: infra de migração | 1 arquivo, 1 mecanismo | ✅ Granular |
| T2: migração 2 | 1 entrada de migração | ✅ Granular |
| T3: connections.rs | 3 funções coesas + 1 remoção, 1 arquivo | ✅ OK (coeso) |
| T4: connection_commands | 1 arquivo, comandos relacionados | ✅ OK (coeso) |
| T5: model_commands | 1 arquivo, o par ativo | ✅ OK (coeso) |
| T6: camada de dados frontend | 3 arquivos, 1 conceito (contrato novo) | ✅ OK (coeso) |
| T7: ConnectionsList | 1 componente | ✅ Granular |
| T8: ModelsList | 1 componente | ✅ Granular |
| T9: ConnectionsSection + gate | 1 componente + verificação | ✅ Granular |
| T10: docs | só markdown | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (corpo) | Diagrama mostra | Status |
| --- | --- | --- | --- |
| T1 | None | sem seta de entrada | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T3 | T3 → T4 | ✅ Match |
| T5 | T3 | T3 → T5 | ✅ Match |
| T6 | T4, T5 | T4, T5 → T6 | ✅ Match |
| T7 | T6 | T6 → T7 [P] | ✅ Match |
| T8 | T6 | T6 → T8 [P] | ✅ Match |
| T9 | T7, T8 | T7, T8 → T9 | ✅ Match |
| T10 | T9 | T9 → T10 | ✅ Match |

T7 e T8 são `[P]`: arquivos distintos, sem dependência mútua. Ambos tocam os JSONs de i18n — chaves em seções diferentes, mas se executados por sub-agentes paralelos há risco de conflito de escrita no mesmo arquivo; **se for executar em paralelo, aplicar as chaves i18n de ambos numa passada só antes de bifurcar.**

---

## Test Co-location Validation

| Task | Camada criada/modificada | Matriz exige | Task diz | Status |
| --- | --- | --- | --- | --- |
| T1 | Lógica pura Rust (migração) | unit | unit | ✅ OK |
| T2 | Lógica pura Rust (migração) | unit | unit | ✅ OK |
| T3 | Lógica pura Rust + SQL local | unit | unit | ✅ OK |
| T4 | Comando Tauri (I/O) | none | none | ✅ OK |
| T5 | Comando Tauri (I/O) | none | none | ✅ OK |
| T6 | Camada de dados React | none | none | ✅ OK |
| T7 | Componente React | none | none | ✅ OK |
| T8 | Componente React | none | none | ✅ OK |
| T9 | Componente React + integração | none | none (gate full) | ✅ OK |
| T10 | Documentação | none | none | ✅ OK |

---

## MCPs & Skills

Nenhum MCP ou skill necessário — não há API externa nova nem tecnologia desconhecida nesta feature (o `ALTER TABLE RENAME COLUMN` do SQLite e o `PRAGMA user_version` são recursos nativos estáveis). Confirmar antes de executar.
