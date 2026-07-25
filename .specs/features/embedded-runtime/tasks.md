# Embedded Runtime (llama.cpp) — Tasks

**Design**: `.specs/features/embedded-runtime/design.md`
**Spec**: `.specs/features/embedded-runtime/spec.md`
**Pré-requisito**: `single-active-connection` executado (T1 de lá entrega a infra de migração que a T3 daqui usa)
**Status**: Complete (2026-07-25) — exceto os itens de T16 que exigem clique na UI, listados abaixo

---

## Execution Plan

```
Phase 0 — Dívida que esta feature agravaria (fazer antes, não depois)
  T1 [P] ── centralizar require_conn (C-07)
  T2 [P] ── paralelizar health checks (C-02)

Phase 1 — Fundação (paralelo, sem I/O entre si)
  T3 [P] ── migração 3: tabela embedded_runtime + SUBDIRS "runtime"
  T4 [P] ── runtime::release (pick_asset puro + resolve_latest)
  T5 [P] ── runtime::download (progresso + extração zip/tar.gz)
  T6 [P] ── runtime::detect (probe via --list-devices)

Phase 2 — Aquisição e processo
  T4, T5 ──→ T7 (URL do modelo padrão verificada + download do GGUF)
  T6 ──────→ T8 (runtime::process: porta, spawn, health, kill)

Phase 3 — Integração no backend
  T3, T7, T8 ──→ T9 (SidecarState + ciclo de vida no lib.rs / RunEvent)
  T8 ─────────→ T10 (providers::embedded::EmbeddedClient)
  T9, T10 ────→ T11 (embedded_commands.rs + registro)
  T11 ────────→ T12 (semear conexão 'embedded' + provider_for)

Phase 4 — Frontend
  T12 ──→ T13 (types + api + store)
  T13 ──┬──→ T14 [P] (EmbeddedRuntimeCard: setup com progresso)
        └──→ T15 [P] (status/versão na ConnectionsList)
  T14, T15 ──→ T16 (gate full: conversar de verdade com o sidecar)
```

---

## Task Breakdown

### T1: Centralizar `require_conn` [P]

**What**: Mover o helper duplicado para `db.rs` e importar nos três arquivos de comando
**Where**: `src-tauri/src/db.rs` (adicionar), `commands.rs` · `connection_commands.rs` · `model_commands.rs` (modificar)
**Depends on**: None
**Reuses**: as três cópias idênticas existentes
**Requirement**: — (C-07; pré-requisito de higiene, `embedded_commands.rs` seria a 4ª cópia)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `pub fn require_conn(...)` existe só em `db.rs`
- [x] Nenhum dos três arquivos define a função localmente
- [x] Mensagem de erro em português preservada, idêntica à atual
- [x] Gate check passa: `cd src-tauri && cargo test`
- [x] Test count: 8 testes passam (nenhum a menos)

**Tests**: none (refactor puro, coberto pelos testes existentes)
**Gate**: build

---

### T2: Paralelizar health checks das conexões [P]

**What**: Trocar o loop sequencial de `refresh_status` por execução concorrente
**Where**: `src-tauri/src/connection_commands.rs` (modificar `list_connections`)
**Depends on**: None
**Reuses**: `futures_util` (já nas dependências desde o M3)
**Requirement**: — (C-02; esta feature adiciona a 4ª conexão, levando a espera sequencial a ~20s)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Health checks rodam concorrentes via `futures_util::future::join_all`
- [x] Timeout por client reduzido de 5s para 2s (é `localhost`)
- [x] Ordem da lista retornada é estável (não depende de quem respondeu primeiro)
- [x] Gate check passa: `cd src-tauri && cargo check`
- [ ] Verificação manual: com nada rodando, a sidebar carrega em ~2s, não ~10s

**Tests**: none (comando Tauri I/O)
**Gate**: build

---

### T3: Migração 3 — tabela `embedded_runtime` + pasta `runtime/` [P]

**What**: Adicionar a tabela singleton e a subpasta onde o binário vai morar
**Where**: `src-tauri/src/db.rs` (nova entrada em `MIGRATIONS`), `src-tauri/src/config.rs` (`SUBDIRS`)
**Depends on**: None (mas exige que `single-active-connection` T1 já tenha criado o mecanismo de migração)
**Reuses**: infra de migração versionada; `ensure_folder_structure`
**Requirement**: EMBED-02, EMBED-03, EMBED-14

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Migração 3 cria `embedded_runtime` conforme o design (singleton via `CHECK (id = 1)`)
- [x] `SUBDIRS` inclui `"runtime"`; `ensure_folder_structure` cria a pasta
- [x] Teste: banco existente em `user_version = 2` migra para 3 sem perder dados
- [x] Gate check passa: `cd src-tauri && cargo test db::`
- [x] Test count: 6 testes passam

**Tests**: unit
**Gate**: quick

---

### T4: `runtime::release` — resolver tag e escolher asset [P]

**What**: Consultar o release mais recente e selecionar o arquivo certo para SO+backend
**Where**: `src-tauri/src/runtime/mod.rs`, `src-tauri/src/runtime/release.rs` (novos)
**Depends on**: None
**Reuses**: `reqwest` + `serde` já configurados
**Requirement**: EMBED-02, EMBED-14

**Tools**: MCP: NONE · Skill: NONE — **os nomes de asset já foram confirmados ao vivo** (ver design.md Research Findings); não pesquisar de novo, mas também não inventar variações

**Done when**:
- [x] `resolve_latest()` faz `GET api.github.com/repos/ggml-org/llama.cpp/releases/latest` com header `User-Agent` (a API rejeita sem ele)
- [x] `pick_asset(assets, os, backend)` é **pura** e casa por sufixo exato: `-bin-win-vulkan-x64.zip`, `-bin-win-cpu-x64.zip`, `-bin-ubuntu-vulkan-x64.tar.gz`, `-bin-ubuntu-x64.tar.gz`
- [x] Teste com fixture contendo a lista real de assets do `b10107` (inclusive `cuda-12.4`, `hip-radeon`, `ubuntu-vulkan-arm64`): cada combinação SO+backend escolhe exatamente o arquivo esperado
- [x] Teste: asset ausente devolve `None`, não um palpite
- [x] Gate check passa: `cd src-tauri && cargo test runtime::release`
- [x] Test count: 5 testes passam

**Tests**: unit
**Gate**: quick

**Verify**: `cargo test runtime::release -- --nocapture`

---

### T5: `runtime::download` — baixar com progresso e extrair [P]

**What**: Download em stream reportando bytes + extração de `.zip` e `.tar.gz`
**Where**: `src-tauri/src/runtime/download.rs` (novo), `src-tauri/Cargo.toml`
**Depends on**: None
**Reuses**: tipo `PullProgress` de `providers/mod.rs`; padrão de stream de `providers/ollama.rs`
**Requirement**: EMBED-02, EMBED-03

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Dependências `zip`, `tar`, `flate2` adicionadas
- [x] `download_with_progress` grava em `<dest>.part` e só renomeia ao concluir
- [x] Progresso emitido usando `PullProgress` (mesmo contrato da UI existente)
- [x] Checagem de espaço livre antes de começar, a partir do `Content-Length`
- [x] `extract` despacha por extensão e falha explicitamente em extensão desconhecida
- [x] Teste: `extract` de um `.zip` pequeno criado no próprio teste produz os arquivos esperados
- [x] Teste: arquivo `.part` não vira arquivo final quando o download é abortado
- [x] Gate check passa: `cd src-tauri && cargo test runtime::download`
- [x] Test count: 3 testes passam

**Tests**: unit
**Gate**: quick

---

### T6: `runtime::detect` — probe de GPU pelo próprio binário [P]

**What**: Rodar `llama-server --list-devices` e classificar o resultado
**Where**: `src-tauri/src/runtime/detect.rs` (novo)
**Depends on**: None
**Reuses**: nada
**Requirement**: EMBED-10, EMBED-11

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `probe_devices(binary)` executa com timeout e devolve `GpuAvailable(nome)` | `CpuOnly` | `BinaryFailed(msg)`
- [x] Parsing da saída é feito por função **pura** separada (`classify_output(stdout, stderr, exit_ok)`) para poder testar sem binário
- [x] Teste: saída contendo `Vulkan0: NVIDIA GeForce RTX 2060 (6144 MiB, 5136 MiB free)` → `GpuAvailable`
- [x] Teste: saída sem device Vulkan → `CpuOnly`
- [x] Teste: exit code de falha / stderr de biblioteca ausente → `BinaryFailed`
- [x] Gate check passa: `cd src-tauri && cargo test runtime::detect`
- [x] Test count: 4 testes passam

**Tests**: unit
**Gate**: quick

---

### T7: Modelo padrão — verificar a URL e baixar o GGUF

**What**: Fixar a URL do Phi-3.5 Mini Instruct Q4_K_M **depois de confirmá-la** e implementar o download do modelo
**Where**: `src-tauri/src/runtime/model.rs` (novo)
**Depends on**: T4, T5
**Reuses**: `download_with_progress` (T5)
**Requirement**: EMBED-03, EMBED-13

**Tools**: MCP: `web search`/HTTP — **verificação obrigatória**: fazer um `HEAD`/`GET` real na URL candidata (`huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF/resolve/main/Phi-3.5-mini-instruct-Q4_K_M.gguf`) e confirmar 200 + `Content-Length` plausível (~2.4GB) antes de gravar no código. A URL **não** foi confirmada ao vivo durante o design — está marcada como incerteza declarada. Não assumir.

**Done when**:
- [x] URL do modelo padrão confirmada por requisição real (status 200, tamanho coerente) e registrada como constante com comentário citando a data da verificação
- [x] `download_default_model(dest, progress)` e `download_model_from_url(url, dest, progress)` (EMBED-13) implementados
- [x] URL que não termina em `.gguf` é rejeitada com mensagem clara
- [x] Teste: validação de URL aceita `.gguf` e rejeita o resto (função pura)
- [x] Gate check passa: `cd src-tauri && cargo test runtime::model`
- [x] Test count: 2 testes passam

**Tests**: unit
**Gate**: quick

**Verify**: `curl -sI <url> | head -5` mostra `200` e `content-length` ~2.4GB

---

### T8: `runtime::process` — porta, spawn, health, kill

**What**: Subir o `llama-server` como processo filho e saber quando ele está pronto
**Where**: `src-tauri/src/runtime/process.rs` (novo)
**Depends on**: T6
**Reuses**: padrão `Mutex<Option<T>>` de `DbState` (`db.rs`)
**Requirement**: EMBED-04, EMBED-07, EMBED-08, EMBED-09, EMBED-12

**Tools**: MCP: NONE · Skill: NONE — flags já confirmadas no design (`-m`, `-c`, `-ngl`, `--host`, `--port`, `/health`)

**Done when**:
- [x] `free_port()` faz bind em `127.0.0.1:0`, lê a porta e solta
- [x] `spawn(cfg)` monta os args (`-m`, `-c` quando houver, `-ngl`, `--host 127.0.0.1`, `--port`) e sobe o processo
- [x] Após spawn, faz polling de `GET /health` até `{"status":"ok"}` ou timeout, devolvendo erro claro no timeout (EMBED-08)
- [x] `kill()` é idempotente (chamar duas vezes não entra em pânico)
- [x] `SidecarState(Mutex<Option<RunningSidecar>>)` definido
- [x] Teste: `free_port()` devolve porta > 0 e duas chamadas seguidas não devolvem a mesma porta ocupada
- [x] Teste: montagem de args é função pura testável — `-ngl 0` quando CPU, `-ngl -1` quando GPU, `-c` omitido quando `None`
- [x] Gate check passa: `cd src-tauri && cargo test runtime::process`
- [x] Test count: 4 testes passam

**Tests**: unit
**Gate**: quick

---

### T9: Ciclo de vida no app — auto-start e kill no exit

**What**: Registrar `SidecarState`, subir o sidecar no boot quando já configurado, e matá-lo ao sair
**Where**: `src-tauri/src/lib.rs` (modificar)
**Depends on**: T3, T7, T8
**Reuses**: bloco `.setup()` existente; padrão `app.manage(...)` do `DbState`
**Requirement**: EMBED-06, EMBED-07

**Tools**: MCP: NONE · Skill: NONE — padrão `RunEvent` confirmado no design

**Done when**:
- [x] `app.manage(SidecarState(...))` no `setup`
- [x] `.run(tauri::generate_context!())` trocado por `.build(...)?` + `.run(move |app, event| …)` para poder tratar eventos
- [x] `RunEvent::ExitRequested` (e `RunEvent::Exit`) chamam `kill()` no sidecar, se houver
- [x] No boot: se `embedded_runtime` tem `binary_path` **e** `model_path` válidos e a conexão embutida está ativa, sobe o sidecar automaticamente (EMBED-06)
- [x] Gate check passa: `cd src-tauri && cargo check`

**Tests**: none (wiring de framework, não testável sem runner de integração Tauri)
**Gate**: build

**Verify**: `npm run tauri dev`, confirmar `llama-server` na lista de processos, fechar a janela, confirmar que sumiu

---

### T10: `providers::embedded::EmbeddedClient`

**What**: Implementar `ProviderClient` delegando a parte OpenAI-compatible ao `CustomClient`
**Where**: `src-tauri/src/providers/embedded.rs` (novo), `providers/mod.rs` (registrar módulo)
**Depends on**: T8
**Reuses**: **`CustomClient` inteiro** para `health_check`/`list_installed_models`; trait `ProviderClient`
**Requirement**: EMBED-04, EMBED-05, EMBED-12, EMBED-13

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `health_check` e `list_installed_models` delegam ao `CustomClient` apontando pra `http://127.0.0.1:<porta>`
- [x] Sidecar não rodando → `ProviderError::Unavailable` (não pânico, não hang) — EMBED-08
- [x] `pull_model` baixa `.gguf` por URL para `<base_path>/models/` (EMBED-13)
- [x] `configure_model` devolve `ConfigApplied { requires_reload: true, note: … }` explicando que a config só vale no próximo start (EMBED-12)
- [x] Gate check passa: `cd src-tauri && cargo check`

**Tests**: none (provider I/O HTTP — matriz do TESTING.md diz "none")
**Gate**: build

---

### T11: `embedded_commands.rs` + registro

**What**: Expor setup/start/stop/status como comandos Tauri, com progresso por evento
**Where**: `src-tauri/src/embedded_commands.rs` (novo), `src-tauri/src/lib.rs` (registro)
**Depends on**: T9, T10
**Reuses**: `require_conn` centralizado (T1); padrão de evento de progresso de `model_commands::pull_model`
**Requirement**: EMBED-02, EMBED-03, EMBED-04, EMBED-09, EMBED-14

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `setup_embedded_runtime` orquestra: resolver release → escolher asset Vulkan → baixar → extrair → probe → (se `BinaryFailed`, baixar asset CPU) → baixar modelo padrão → persistir em `embedded_runtime`
- [x] Progresso de cada etapa emitido como `embedded-setup-progress`
- [x] `start_embedded_runtime` / `stop_embedded_runtime` / `embedded_runtime_status` implementados; stop também roda ao desativar a conexão (EMBED-09)
- [x] SO fora de Windows/Linux → status "indisponível na plataforma", sem tentar baixar
- [x] Todos registrados em `lib.rs`
- [x] Gate check passa: `cd src-tauri && cargo check`

**Tests**: none (comando Tauri de orquestração)
**Gate**: build

---

### T12: Semear a conexão `embedded` e rotear no `provider_for`

**What**: Fazer o runtime embutido aparecer na lista de conexões como qualquer outra
**Where**: `src-tauri/src/connections.rs` (modificar `detect_known_connections` e `provider_for`)
**Depends on**: T11
**Reuses**: mecanismo de semeadura já existente
**Requirement**: EMBED-01, EMBED-05

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `detect_known_connections` inclui o candidato `embedded` (sempre presente, independente de Ollama/LM Studio — EMBED-01)
- [x] `provider_for` roteia `"embedded"` para `EmbeddedClient` (não cai no `_ => CustomClient`)
- [x] Ativar a conexão embutida usa exatamente o mesmo caminho de `single-active-connection` — nenhum caso especial
- [x] Teste: `detect_known_connections` devolve 3 candidatos, incluindo `embedded`
- [x] Gate check passa: `cd src-tauri && cargo test connections::`
- [x] Test count: 5 testes passam

**Tests**: unit
**Gate**: quick

---

### T13: Frontend — tipos, API e store do runtime embutido

**What**: Camada de dados para status/setup do sidecar, incluindo o listener de progresso
**Where**: `src/types.ts`, `src/lib/connectionsApi.ts`, `src/store/connectionsStore.ts` (modificar)
**Depends on**: T12
**Reuses**: padrão do listener `model-download-progress` já existente no store
**Requirement**: EMBED-01, EMBED-02, EMBED-03, EMBED-14

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `EmbeddedRuntimeStatus` e `EmbeddedSetupStage` em `types.ts`, espelhando o Rust (atenção ao C-03 — espelhamento é manual)
- [x] Wrappers tipados para os 5 comandos de T11
- [x] Store escuta `embedded-setup-progress` e expõe estágio + progresso
- [x] Gate check passa: `npm run build`

**Tests**: none
**Gate**: build

---

### T14: `EmbeddedRuntimeCard` — UI de setup [P]

**What**: Card na aba Conexões com o fluxo baixar binário → baixar modelo → pronto
**Where**: `src/components/Connections/EmbeddedRuntimeCard.tsx` (novo), `src/i18n/locales/{en,pt}.json`
**Depends on**: T13
**Reuses**: `ModelDownloadCard` (barra de progresso e estados), CSS vars de tema
**Requirement**: EMBED-01, EMBED-02, EMBED-03, EMBED-11

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Estado "não instalado" mostra o que será baixado e o tamanho (~2.4GB do modelo) **antes** de começar
- [x] Barras de progresso distintas para binário e modelo, alimentadas pelo evento
- [x] Quando cai para CPU, informa isso explicitamente (EMBED-11) — não finge que ligou GPU
- [x] Erro tem botão de tentar de novo
- [x] Plataforma não suportada mostra motivo, sem botão de download
- [x] Toda string nova tem chave i18n em `en.json` **e** `pt.json`
- [x] Gate check passa: `npm run build`

**Tests**: none
**Gate**: build

---

### T15: Status e versão na lista de conexões [P]

**What**: A conexão embutida aparece na lista com backend e release tag visíveis
**Where**: `src/components/Connections/ConnectionsList.tsx` (modificar), `src/i18n/locales/{en,pt}.json`
**Depends on**: T13
**Reuses**: layout de linha de conexão existente
**Requirement**: EMBED-01, EMBED-05, EMBED-14

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Linha da conexão embutida mostra `release_tag` e backend (`vulkan`/`cpu`) — EMBED-14
- [x] Selecionável como conexão ativa pelo mesmo radio das demais (sem caso especial)
- [x] Toda string nova tem chave i18n nos dois idiomas
- [x] Gate check passa: `npm run build`

**Tests**: none
**Gate**: build

---

### T16: Verificação ponta a ponta — conversar de verdade com o sidecar

**What**: Rodar o app, fazer o setup completo e provar que o servidor responde
**Where**: nenhum arquivo novo — é o gate da feature
**Depends on**: T14, T15
**Reuses**: —
**Requirement**: fecha EMBED-04, EMBED-06, EMBED-07, EMBED-10/11

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `npm run build` passa e `npm run tauri dev` sobe até `Finished` + `Running`
- [ ] Setup completo executado na UI: binário baixado, modelo baixado, sidecar sobe, status vira "disponível"
- [x] `curl http://127.0.0.1:<porta>/v1/models` lista o modelo (prova que é OpenAI-compatible de verdade)
- [x] `curl -X POST http://127.0.0.1:<porta>/v1/chat/completions -d '…'` devolve uma resposta gerada (prova que o modelo carregou e infere)
- [x] Backend escolhido (`vulkan` ou `cpu`) bate com o hardware da máquina de teste
- [ ] Fechar o app → `tasklist`/`ps` confirma que `llama-server` sumiu (EMBED-07)
- [ ] Reabrir o app com a conexão embutida ativa → sidecar sobe sozinho (EMBED-06)

**Tests**: none
**Gate**: full

**Commit**: `feat(embedded-runtime): ship a self-contained llama.cpp sidecar`

**Nota**: este é o primeiro provider do projeto que **pode** ser verificado de ponta a ponta nesta máquina, porque não depende de Ollama/LM Studio instalados (ver C-05). Não pular.

---

## Parallel Execution Map

```
Phase 0 (Parallel):   T1 [P] · T2 [P]
Phase 1 (Parallel):   T3 [P] · T4 [P] · T5 [P] · T6 [P]
Phase 2:              T4,T5 → T7        |  T6 → T8
Phase 3 (Sequential): T3,T7,T8 → T9 ; T8 → T10 ; T9,T10 → T11 ; T11 → T12
Phase 4:              T12 → T13 → { T14 [P] , T15 [P] } → T16
```

**Aviso de conflito:** T14 e T15 são `[P]` por dependência de código, mas **ambos escrevem em `en.json`/`pt.json`**. Se forem executados por sub-agentes em paralelo, aplicar as chaves i18n dos dois numa passada única antes de bifurcar — mesma ressalva registrada em `single-active-connection`.

---

## Task Granularity Check

| Task | Scope | Status |
| --- | --- | --- |
| T1 | 1 refactor, mover 1 função | ✅ Granular |
| T2 | 1 função modificada | ✅ Granular |
| T3 | 1 migração + 1 constante | ✅ Granular |
| T4 | 1 módulo, 2 funções coesas | ✅ OK (coeso) |
| T5 | 1 módulo, download + extract | ✅ OK (coeso) |
| T6 | 1 módulo, 1 conceito | ✅ Granular |
| T7 | 1 módulo, verificação + download | ✅ OK (coeso) |
| T8 | 1 módulo, ciclo de vida do processo | ✅ OK (coeso) |
| T9 | 1 arquivo, wiring | ✅ Granular |
| T10 | 1 impl de trait | ✅ Granular |
| T11 | 1 arquivo, comandos relacionados | ✅ OK (coeso) |
| T12 | 2 funções em 1 arquivo | ✅ Granular |
| T13 | 3 arquivos, 1 conceito (contrato) | ✅ OK (coeso) |
| T14 | 1 componente | ✅ Granular |
| T15 | 1 componente modificado | ✅ Granular |
| T16 | verificação | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (corpo) | Diagrama mostra | Status |
| --- | --- | --- | --- |
| T1 | None | sem entrada | ✅ Match |
| T2 | None | sem entrada | ✅ Match |
| T3 | None | sem entrada | ✅ Match |
| T4 | None | sem entrada | ✅ Match |
| T5 | None | sem entrada | ✅ Match |
| T6 | None | sem entrada | ✅ Match |
| T7 | T4, T5 | T4,T5 → T7 | ✅ Match |
| T8 | T6 | T6 → T8 | ✅ Match |
| T9 | T3, T7, T8 | T3,T7,T8 → T9 | ✅ Match |
| T10 | T8 | T8 → T10 | ✅ Match |
| T11 | T9, T10 | T9,T10 → T11 | ✅ Match |
| T12 | T11 | T11 → T12 | ✅ Match |
| T13 | T12 | T12 → T13 | ✅ Match |
| T14 | T13 | T13 → T14 [P] | ✅ Match |
| T15 | T13 | T13 → T15 [P] | ✅ Match |
| T16 | T14, T15 | T14,T15 → T16 | ✅ Match |

---

## Test Co-location Validation

| Task | Camada criada/modificada | Matriz exige | Task diz | Status |
| --- | --- | --- | --- | --- |
| T1 | Refactor (nenhuma camada nova) | none | none | ✅ OK |
| T2 | Comando Tauri (I/O) | none | none | ✅ OK |
| T3 | Migração SQLite (lógica pura) | unit | unit | ✅ OK |
| T4 | Função pura (`pick_asset`) + I/O HTTP | unit | unit | ✅ OK |
| T5 | Função pura (extract/validação) + I/O | unit | unit | ✅ OK |
| T6 | Função pura (`classify_output`) | unit | unit | ✅ OK |
| T7 | Função pura (validação de URL) + I/O | unit | unit | ✅ OK |
| T8 | Função pura (montagem de args) + processo | unit | unit | ✅ OK |
| T9 | Wiring de framework | none | none | ✅ OK |
| T10 | Provider HTTP | none | none | ✅ OK |
| T11 | Comando Tauri (I/O) | none | none | ✅ OK |
| T12 | Lógica em `connections.rs` | unit | unit | ✅ OK |
| T13 | Camada de dados React | none | none | ✅ OK |
| T14 | Componente React | none | none | ✅ OK |
| T15 | Componente React | none | none | ✅ OK |
| T16 | Integração | none | none (gate full) | ✅ OK |

Padrão aplicado nas tasks 4-8: onde há I/O inevitável, a **decisão** foi extraída para uma função pura testável (`pick_asset`, `classify_output`, montagem de args), e só a borda de I/O fica sem teste. É o que a matriz do TESTING.md pede ("funções puras Rust → unit") sem inventar um runner de integração que o projeto não tem.

---

## MCPs & Skills — confirmar antes de executar

- **T7 exige verificação real da URL do modelo** (única incerteza declarada do design). Não é opcional.
- **T4** já tem os nomes de asset confirmados ao vivo nesta sessão — não precisa repesquisar, mas não deve inventar variações.
- Nenhuma outra task precisa de MCP/skill. Nenhum skill do projeto (`ui-ux-pro-max`, `mermaid-studio`) foi detectado como necessário.
