# Conexões & Modelos Tasks

**Design**: `.specs/features/connections-models/design.md`
**Status**: Complete (2026-07-25) — all 15 tasks implemented; cargo test + npm run build + npm run tauri dev full gate all green. No live Ollama/LM Studio instance available in this environment, so provider clients were not manually exercised against a real server.

---

## Execution Plan

### Phase 1: Foundation (Parallel — sem dependências entre si)

```
T1 [P] ── DB migrations (connections, model_configs)
T2 [P] ── RamDetector + unit test
T3 [P] ── ModelCatalog + estimate_ram_gb + unit test
T4 [P] ── ProviderClient trait + tipos compartilhados
```

### Phase 2: Provider Implementations (Parallel — depende de T4)

```
T4 ──┬──→ T5 [P] OllamaClient
     └──→ T6 [P] LmStudioClient
```

### Phase 3: Backend orchestration (Sequential)

```
T1, T4 ──→ T7 (ConnectionManager) ──→ T8 (connection_commands)
T3, T5, T6, T7 ──→ T9 (model_commands)
```

### Phase 4: Frontend (Parallel onde possível)

```
T8, T9 ──→ T10 (connectionsApi + connectionsStore)
T10 ──┬──→ T11 [P] (uiStore + ConnectionsSection nav)
      ├──→ T12 (ConnectionsList + ConnectionsPanel shell) [depende de T11]
      ├──→ T13 [P] (ModelsList + ModelDownloadCard)
      └──→ T14 [P] (ModelConfigForm)
T12, T13, T14 ──→ T15 (Wire no App.tsx)
```

---

## Task Breakdown

### T1: Migrações SQLite para `connections` e `model_configs` [P]

**What**: Adicionar as duas tabelas novas ao schema do `db.rs` (mesma `SCHEMA` const usada desde M1)
**Where**: `src-tauri/src/db.rs` (modificar `SCHEMA`)
**Depends on**: None
**Reuses**: `db::open()` já existente (M1/M2) — só estende a string de schema
**Requirement**: CONN-01, CONN-02, CONN-05, CONN-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `CREATE TABLE IF NOT EXISTS connections (...)` e `model_configs (...)` conforme design.md
- [x] `cargo check` passa
- [x] Rodar `npm run tauri dev` uma vez e confirmar via inspeção do `.db` (ex.: `sqlite3` CLI ou DB Browser) que as tabelas existem

**Tests**: none
**Gate**: build

---

### T2: `RamDetector` + teste unitário [P]

**What**: Módulo que detecta RAM total do sistema via `sysinfo`
**Where**: `src-tauri/src/system_info.rs` (novo)
**Depends on**: None
**Reuses**: nada
**Requirement**: CONN-07

**Tools**: MCP: `context7` (confirmar API atual do crate `sysinfo`) · Skill: NONE

**Done when**:
- [x] `fn total_ram_gb() -> f32` implementado e adicionado ao `Cargo.toml`
- [x] Teste unitário confirma que o valor retornado é > 0 nesta máquina
- [x] `cargo test system_info` passa

**Tests**: unit
**Gate**: `cargo test system_info`

**Verify**: `cargo test system_info -- --nocapture` mostra o valor de RAM detectado

---

### T3: `ModelCatalog` + `estimate_ram_gb` + teste unitário [P]

**What**: Lista curada de modelos populares (dados estáticos) + função pura de estimativa de RAM
**Where**: `src-tauri/src/models/catalog.rs`, `src-tauri/src/models/memory_estimate.rs`
**Depends on**: None
**Reuses**: nada
**Requirement**: CONN-08, CONN-09

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `curated_models() -> &'static [CuratedModel]` com pelo menos 6-8 modelos conhecidos publicamente (ex.: Llama 3.1 8B, Qwen2.5 7B, Phi-3 mini) com `params_billions` e `default_quant`
- [x] `estimate_ram_gb(params_billions: f32, quant: Quant) -> f32` implementa `params × bytes_por_peso × 1.2`
- [x] Teste unitário cobre pelo menos: Q4 de um modelo 7B fica na faixa esperada (~4-5GB), Q8 fica maior que Q4 para o mesmo modelo
- [x] `cargo test memory_estimate` passa

**Tests**: unit
**Gate**: `cargo test memory_estimate`

**Verify**: `cargo test memory_estimate -- --nocapture`

---

### T4: `ProviderClient` trait + tipos compartilhados [P]

**What**: Definir o trait e os tipos (`InstalledModel`, `PullProgress`, `ConfigApplied`, `GpuOffload`, `ProviderError`) sem implementação ainda
**Where**: `src-tauri/src/providers/mod.rs`
**Depends on**: None
**Reuses**: nada
**Requirement**: CONN-01 (base para todos)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Trait `ProviderClient` com as 4 assinaturas do design.md (`health_check`, `list_installed_models`, `pull_model`, `configure_model`)
- [x] Todos os tipos de dados compilam com `derive(Serialize, Deserialize)` onde cruzam pra JS
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

---

### T5: `OllamaClient` (impl `ProviderClient`) [P]

**What**: Implementação concreta para Ollama
**Where**: `src-tauri/src/providers/ollama.rs`
**Depends on**: T4
**Reuses**: `ProviderClient` trait (T4)
**Requirement**: CONN-01, CONN-05, CONN-11, CONN-12, CONN-13

**Tools**: MCP: `context7`/`web search` (confirmar payload exato de `/api/tags`, `/api/pull`, `options.num_ctx`/`num_gpu` — já verificado no design.md, mas revalidar campos exatos do JSON ao implementar) · Skill: NONE

**Done when**:
- [x] `health_check` faz `GET /` ou `/api/tags` e trata timeout/conexão recusada como `Unavailable`
- [x] `list_installed_models` parseia `GET /api/tags`
- [x] `pull_model` faz `POST /api/pull` com `stream: true`, parseia NDJSON, envia `PullProgress` pelo `Sender` a cada linha
- [x] `configure_model` retorna `ConfigApplied` indicando que `num_ctx`/`num_gpu` serão aplicados na próxima chamada de chat (não há endpoint de "salvar config" separado no Ollama — a aplicação real acontece em `stream_chat`, feature `chat-messaging`)
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: com Ollama rodando localmente, chamar `list_installed_models` via um teste manual/`cargo run` e ver modelos reais retornados

---

### T6: `LmStudioClient` (impl `ProviderClient`) [P]

**What**: Implementação concreta para LM Studio
**Where**: `src-tauri/src/providers/lmstudio.rs`
**Depends on**: T4
**Reuses**: `ProviderClient` trait (T4)
**Requirement**: CONN-01, CONN-05, CONN-11, CONN-12, CONN-13

**Tools**: MCP: `context7`/`web search` (confirmar payload exato de `/api/v1/models`, `/api/v1/models/download`, `/api/v1/models/load` — já verificado no design.md, revalidar campos exatos ao implementar) · Skill: NONE

**Done when**:
- [x] `health_check` faz `GET /api/v1/models` e trata indisponibilidade
- [x] `list_installed_models` parseia a resposta
- [x] `pull_model` chama o endpoint de download e reporta progresso (formato de progresso do LM Studio confirmado/adaptado ao `PullProgress` comum)
- [x] `configure_model` chama `/api/v1/models/load` com `contextLength`/`gpuOffload`, retorna `ConfigApplied` indicando que a config exige (re)carregar o modelo
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: com LM Studio rodando localmente, chamar `list_installed_models` e ver modelos reais retornados

---

### T7: `ConnectionManager`

**What**: CRUD de conexões em SQLite + detecção automática + roteamento pro `ProviderClient` certo
**Where**: `src-tauri/src/connections.rs`
**Depends on**: T1, T4
**Reuses**: `DbState` (M1), `ProviderClient` trait
**Requirement**: CONN-01, CONN-02, CONN-03, CONN-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `detect_known_connections()` retorna candidatos Ollama (`:11434`) e LM Studio (`:1234`)
- [x] `refresh_status()` faz o health check via `provider_for()`
- [x] CRUD de conexão (criar manual, habilitar/desabilitar, listar) persiste em `connections`
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

---

### T8: `connection_commands.rs` (comandos Tauri)

**What**: Expor `ConnectionManager` como comandos invocáveis do frontend
**Where**: `src-tauri/src/connection_commands.rs`
**Depends on**: T7
**Reuses**: padrão de `require_conn`/tratamento de erro de `commands.rs` (M1)
**Requirement**: CONN-01, CONN-02, CONN-03, CONN-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `list_connections`, `add_connection`, `toggle_connection`, `refresh_connection_status` registrados em `lib.rs`
- [x] `cargo check` passa
- [x] `npm run tauri dev` sobe sem erro com os novos comandos registrados

**Tests**: none
**Gate**: build

---

### T9: `model_commands.rs` (comandos Tauri)

**What**: Expor listagem de instalados/curados, download com progresso (evento), configuração de modelo
**Where**: `src-tauri/src/model_commands.rs`
**Depends on**: T3, T5, T6, T7
**Reuses**: `ModelCatalog` (T3), `RamDetector` (T2), `ProviderClient` (T5/T6)
**Requirement**: CONN-05, CONN-06, CONN-08, CONN-09, CONN-10, CONN-11, CONN-12, CONN-13

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `list_installed_models(connection_id)`, `list_downloadable_models()` (curados + flag `fits_ram`), `pull_model(connection_id, identifier)` (emite `model-download-progress`), `set_active_model(model_config_id)`, `configure_model(model_config_id, context_length, gpu_offload)`
- [x] `list_downloadable_models` usa `RamDetector::total_ram_gb()` pra marcar `fits_ram: bool` por item
- [x] `cargo check` passa

**Tests**: none
**Gate**: build

**Verify**: baixar um modelo pequeno real via `npm run tauri dev` e ver progresso no log de eventos

---

### T10: `connectionsApi.ts` + `connectionsStore.ts`

**What**: Wrappers `invoke` tipados + store Zustand (conexões, modelos instalados/curados, progresso de download)
**Where**: `src/lib/connectionsApi.ts`, `src/store/connectionsStore.ts`
**Depends on**: T8, T9
**Reuses**: padrão de `chatApi.ts`/`chatStore.ts` (M1), `configApi.ts`/`configStore.ts` (M2)
**Requirement**: CONN-01 a CONN-13 (camada de dados do frontend)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Todos os comandos de T8/T9 têm wrapper tipado
- [x] Store escuta `model-download-progress` via `@tauri-apps/api/event` e atualiza estado
- [x] `npm run build` passa (tsc sem erro)

**Tests**: none
**Gate**: build

---

### T11: `uiStore` estendido + `ConnectionsSection.tsx` (nav) [P]

**What**: Adicionar `"connections"` ao union type de `ActiveView`; converter o placeholder atual em item de navegação (padrão AD-014, igual `SettingsSection.tsx`)
**Where**: `src/store/uiStore.ts` (modificar), `src/components/Sidebar/ConnectionsSection.tsx` (reescrever)
**Depends on**: T10
**Reuses**: `SettingsSection.tsx` como referência direta de padrão
**Requirement**: CONN-01 (ponto de entrada da UI)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `ActiveView = "chat" | "settings" | "connections"`
- [x] Item de nav mostra indicador de status (ex.: bolinha verde/cinza) somando os status das conexões habilitadas
- [x] Todo texto novo usa chave i18n (`sidebar.connections` já existe; adicionar o que faltar em `en.json`/`pt.json`)
- [x] `npm run build` passa

**Tests**: none
**Gate**: build

---

### T12: `ConnectionsList.tsx` + `ConnectionsPanel.tsx` (shell)

**What**: Painel de tela cheia (como `SettingsPanel.tsx`) com sub-aba "Conexões": lista com status, toggle habilitar, form de conexão manual
**Where**: `src/components/Connections/ConnectionsPanel.tsx`, `src/components/Connections/ConnectionsList.tsx`
**Depends on**: T11
**Reuses**: `SettingsPanel.tsx` (header com voltar, layout, CSS vars de tema)
**Requirement**: CONN-01, CONN-02, CONN-03, CONN-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Lista mostra Ollama/LM Studio detectados + conexões manuais, com status visual
- [x] Toggle habilitar/desabilitar funciona e persiste (via T10)
- [x] Estado vazio (CONN-04) quando nenhuma conexão disponível, com botão de retry
- [x] `npm run build` passa

**Tests**: none
**Gate**: build

---

### T13: `ModelsList.tsx` + `ModelDownloadCard.tsx` [P]

**What**: Sub-aba "Modelos" — instalados (selecionáveis) e para baixar (filtrados por RAM, com progresso)
**Where**: `src/components/Connections/ModelsList.tsx`, `src/components/Connections/ModelDownloadCard.tsx`
**Depends on**: T10
**Reuses**: `SettingsPanel.tsx` (padrão visual)
**Requirement**: CONN-05, CONN-06, CONN-08, CONN-09, CONN-10, CONN-11

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Modelos instalados listados com opção de marcar como ativo (CONN-06)
- [x] Modelos curados que não cabem na RAM aparecem ocultos por padrão ou marcados "não recomendado", com toggle "mostrar mesmo assim"
- [x] Campo de pull manual (nome/link) sempre visível, sem checagem de RAM
- [x] Barra de progresso atualiza em tempo real durante download
- [x] `npm run build` passa

**Tests**: none
**Gate**: build

---

### T14: `ModelConfigForm.tsx` [P]

**What**: Form de contexto + CPU/GPU por modelo, mostrando quando o provedor não suporta 100% (CONN-13 AC3)
**Where**: `src/components/Connections/ModelConfigForm.tsx`
**Depends on**: T10
**Reuses**: padrão de inputs de `Wizard.tsx`/`SettingsPanel.tsx`
**Requirement**: CONN-12, CONN-13

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Campo de tamanho de contexto (número) e seletor CPU/GPU
- [x] Salvar chama `configure_model` (T9) e mostra o que foi de fato aplicado (`ConfigApplied`)
- [x] `npm run build` passa

**Tests**: none
**Gate**: build

---

### T15: Roteamento no `App.tsx`

**What**: Renderizar `ConnectionsPanel` quando `activeView === "connections"`
**Where**: `src/App.tsx` (modificar)
**Depends on**: T12, T13, T14
**Reuses**: mesmo padrão de `activeView === "settings"` já existente (M2)
**Requirement**: CONN-01 (fecha o fluxo ponta a ponta)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `activeView === "connections"` renderiza `ConnectionsPanel`
- [x] `npm run build` passa
- [x] `npm run tauri dev`: clicar em Conexões na sidebar abre o painel; criar/selecionar chat volta pra view de chat (mesmo comportamento do padrão Settings)

**Tests**: none
**Gate**: full (`npm run tauri dev` até `Finished`+`Running` sem erro)

**Commit**: `feat(connections): add connection detection, model management and RAM-based download filtering`

---

## Parallel Execution Map

```
Phase 1 (Parallel):
  T1 [P] · T2 [P] · T3 [P] · T4 [P]

Phase 2 (Parallel, depende de T4):
  T4 ──┬── T5 [P]
       └── T6 [P]

Phase 3 (Sequential):
  T1, T4 → T7 → T8
  T3, T5, T6, T7 → T9

Phase 4 (Parallel onde marcado):
  T8, T9 → T10
  T10 → T11 [P]
  T11 → T12
  T10 → T13 [P]
  T10 → T14 [P]
  T12, T13, T14 → T15
```

---

## Task Granularity Check

| Task | Scope | Status |
| --- | --- | --- |
| T1: DB migrations | 1 mudança de schema | ✅ Granular |
| T2: RamDetector | 1 módulo, 1 função | ✅ Granular |
| T3: ModelCatalog + fórmula | 2 arquivos, 1 conceito coeso (dados + fórmula que os usa) | ✅ OK (coeso) |
| T4: ProviderClient trait | 1 trait + tipos | ✅ Granular |
| T5: OllamaClient | 1 struct implementando 1 trait | ✅ Granular |
| T6: LmStudioClient | 1 struct implementando 1 trait | ✅ Granular |
| T7: ConnectionManager | 1 componente | ✅ Granular |
| T8: connection_commands | 1 arquivo, 4 comandos relacionados | ✅ OK (coeso) |
| T9: model_commands | 1 arquivo, 5 comandos relacionados | ✅ OK (coeso) |
| T10: API + store frontend | 2 arquivos, 1 conceito (camada de dados) | ✅ OK (coeso) |
| T11: uiStore + nav | 2 arquivos pequenos, 1 conceito | ✅ OK (coeso) |
| T12: Lista + painel shell | 2 componentes de uma sub-aba | ✅ OK (coeso) |
| T13: Lista de modelos + card | 2 componentes de uma sub-aba | ✅ OK (coeso) |
| T14: Form de config | 1 componente | ✅ Granular |
| T15: Roteamento | 1 mudança em 1 arquivo | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| --- | --- | --- | --- |
| T1 | None | Nenhuma seta de entrada | ✅ Match |
| T2 | None | Nenhuma seta de entrada | ✅ Match |
| T3 | None | Nenhuma seta de entrada | ✅ Match |
| T4 | None | Nenhuma seta de entrada | ✅ Match |
| T5 | T4 | T4 → T5 | ✅ Match |
| T6 | T4 | T4 → T6 | ✅ Match |
| T7 | T1, T4 | T1, T4 → T7 | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |
| T9 | T3, T5, T6, T7 | T3, T5, T6, T7 → T9 | ✅ Match |
| T10 | T8, T9 | T8, T9 → T10 | ✅ Match |
| T11 | T10 | T10 → T11 | ✅ Match |
| T12 | T11 | T11 → T12 | ✅ Match |
| T13 | T10 | T10 → T13 | ✅ Match |
| T14 | T10 | T10 → T14 | ✅ Match |
| T15 | T12, T13, T14 | T12, T13, T14 → T15 | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| --- | --- | --- | --- | --- |
| T1 | Schema SQLite (I/O) | none | none | ✅ OK |
| T2 | Função pura Rust | unit | unit | ✅ OK |
| T3 | Função pura Rust | unit | unit | ✅ OK |
| T4 | Trait/tipos (sem lógica) | none | none | ✅ OK |
| T5 | Provider (I/O HTTP) | none | none | ✅ OK |
| T6 | Provider (I/O HTTP) | none | none | ✅ OK |
| T7 | Comando Tauri/orquestração (I/O) | none | none | ✅ OK |
| T8 | Comando Tauri (I/O) | none | none | ✅ OK |
| T9 | Comando Tauri (I/O) | none | none | ✅ OK |
| T10 | Componente React (camada de dados) | none | none | ✅ OK |
| T11 | Componente React | none | none | ✅ OK |
| T12 | Componente React | none | none | ✅ OK |
| T13 | Componente React | none | none | ✅ OK |
| T14 | Componente React | none | none | ✅ OK |
| T15 | Integração (App.tsx) | none | none (gate full) | ✅ OK |

---

## MCPs & Skills — Confirmar com o usuário antes de executar

Sugestão por task já anotada acima (`context7`/web search pra T2, T5, T6 — confirmar campos exatos de API antes de escrever o código; NONE para o resto). Nenhum skill deste projeto (`ui-ux-pro-max`, `mermaid-studio` etc.) foi detectado como necessário além do que já está em uso — confirmar antes de iniciar a Execução.
