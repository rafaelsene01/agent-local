# External Integrations

O app é offline-first: **nenhum** serviço de nuvem, telemetria, analytics ou autenticação externa. Todas as integrações são com processos locais na máquina do usuário, via HTTP em `localhost`, sem credenciais.

## LLM Runtimes locais

### Ollama

**Purpose:** Rodar modelos locais; listar e baixar modelos.
**Implementation:** `src-tauri/src/providers/ollama.rs` (`OllamaClient`, impl `ProviderClient`)
**Configuration:** URL padrão `http://localhost:11434`, semeada automaticamente em `connections` na primeira chamada de `list_connections` (desabilitada por padrão). Editável — a URL vem da linha da tabela, não é hardcoded no client.
**Authentication:** nenhuma (API local sem auth).
**Timeout:** 5s (`reqwest::Client::builder().timeout(...)`).

**Endpoints usados** (confirmados contra `ollama/ollama` `docs/api.md`):

| Endpoint | Uso | Formato |
| --- | --- | --- |
| `GET /api/tags` | `health_check` + `list_installed_models` | JSON `{ models: [{ name, size, … }] }` |
| `POST /api/pull` | `pull_model` | Request `{ model, stream: true }`; resposta **NDJSON** — uma linha por evento, com `{ status, total?, completed?, error? }` |

**Nota:** `configure_model` **não faz chamada HTTP** — Ollama não tem endpoint de "salvar config". `num_ctx`/`num_gpu` vão dentro de `options` a cada requisição de `/api/chat`, o que será feito na feature `chat-messaging`. O `ConfigApplied` retornado deixa isso explícito (`requires_reload: false` + nota).

### LM Studio

**Purpose:** Mesmo papel do Ollama, com uma API nativa incompatível.
**Implementation:** `src-tauri/src/providers/lmstudio.rs` (`LmStudioClient`)
**Configuration:** URL padrão `http://localhost:1234`, semeada igual ao Ollama.
**Authentication:** a API aceita `Authorization: Bearer $LM_API_TOKEN` na doc oficial, mas o client **não envia token** — assume instância local aberta. Se o usuário tiver auth ligada, vai falhar (ver CONCERNS.md).
**Requer:** LM Studio ≥ 0.4.0 (quando a REST API nativa `/api/v1/*` foi lançada).

**Endpoints usados** (confirmados contra `lmstudio.ai/docs/developer/rest/*`):

| Endpoint | Uso | Formato |
| --- | --- | --- |
| `GET /api/v1/models` | `health_check` + `list_installed_models` | `{ models: [{ key, size_bytes, … }] }` — identificador é `key`, não `name` |
| `POST /api/v1/models/download` | inicia `pull_model` | Request `{ model }`; resposta `{ job_id?, status, total_size_bytes? }`; `status: "already_downloaded"` vem **sem** `job_id` |
| `GET /api/v1/models/download/status/:job_id` | polling do progresso | `{ status, total_size_bytes?, downloaded_bytes? }`; sem campo de percentual — calculado no cliente |
| `POST /api/v1/models/load` | `configure_model` | `{ model, context_length?, offload_kv_cache_to_gpu?, echo_load_config: true }` |

**Diferença estrutural vs. Ollama:** download é **job + polling** (750ms de intervalo), não stream. Ambos são normalizados pro mesmo `PullProgress` antes de chegar no frontend.

**Divergência documentada:** o `design.md` original supunha `contextLength`/`gpuOffload` (camelCase, offload graduado). A API real usa `context_length` (snake_case) e `offload_kv_cache_to_gpu` (**boolean**, sem fração). `GpuOffload::Fraction` é aceito mas tratado como "ligado", com nota explicando — marcado como `SPEC_DEVIATION` em `lmstudio.rs`.

### Servidor OpenAI-compatible genérico ("custom")

**Purpose:** Permitir apontar pra qualquer outro servidor local compatível (CONN-01 AC4).
**Implementation:** `src-tauri/src/providers/custom.rs` (`CustomClient`)
**Configuration:** URL informada manualmente pelo usuário no formulário da aba Conexões.
**Endpoints:** só `GET /v1/models` (o único padrão universal). `pull_model` e `configure_model` retornam erro/nota explícitos de "não suportado" em vez de fingir sucesso.

## APIs de terceiros (somente leitura, sem auth)

### Hugging Face / catálogo de modelos

**Status:** **não integrado por API.** Nenhum dos dois runtimes expõe catálogo programático de "modelos disponíveis pra baixar" (confirmado por pesquisa — AD-015). A lista de modelos oferecidos pra download é **curada e embutida no binário**: `src-tauri/src/models/catalog.rs`, 8 entradas com `params_billions` públicos conhecidos.

**Consequência:** manter a lista atualizada é trabalho manual. O escape é o campo de pull manual (nome pro Ollama, link HF pro LM Studio), que não passa por catálogo nem por checagem de RAM.

## Sistema operacional

### Detecção de memória

**Implementation:** `src-tauri/src/system_info.rs` via crate `sysinfo` 0.39.
**Uso:** `total_ram_gb()` alimenta o flag `fits_ram` de cada modelo curado. Retorna bytes desde sysinfo 0.26 (verificado no CHANGELOG do crate).
**Fallback:** se retornar 0 (ambientes exóticos), `list_downloadable_models` devolve `ram_detected_gb: None` e marca **todos** como cabendo — nunca esconde tudo silenciosamente.
**Não faz:** detecção de GPU/VRAM (não é confiável entre fabricantes sem SDK proprietário — decisão do usuário registrada no spec do M3).

### Diálogo de arquivos

**Implementation:** plugin `tauri-plugin-dialog` 2, usado em `config_commands::pick_folder` (`blocking_pick_folder`).
**Permissão:** `dialog:default` em `capabilities/default.json`.

### Filesystem

**Implementation:** `std::fs` direto no Rust (não o plugin `fs` do Tauri).
**Escopo:** só dentro da pasta-base escolhida pelo usuário + o `config.json` no `app_config_dir` do SO.
**Validação:** `ensure_folder_structure` escreve um arquivo-sonda (`.localmind-write-test`) pra falhar cedo em pasta sem permissão.

## Webhooks

Nenhum — o app não expõe servidor HTTP nem recebe callbacks.

## Background Jobs

Não há fila nem scheduler. O que existe de assíncrono:

| Job | Mecanismo | Local |
| --- | --- | --- |
| Download de modelo com progresso | `tokio::sync::mpsc` + `tauri::async_runtime::spawn` re-emitindo evento `model-download-progress` | `model_commands::pull_model` |
| Health check de conexões | `async fn` sequencial dentro de `list_connections` (não paralelizado) | `connection_commands::list_connections` |

**Eventos Tauri emitidos** (backend → frontend):

| Evento | Payload | Consumidor |
| --- | --- | --- |
| `model-download-progress` | `{ connection_id, identifier, progress: PullProgress }` | `connectionsStore.ts` via `listen()` no escopo do módulo |
