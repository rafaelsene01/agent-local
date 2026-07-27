# External Integrations

O app é offline-first: **nenhum** serviço de nuvem, telemetria, analytics ou autenticação externa.

Desde o M9 (AD-039) ele também não fala com **nenhum programa externo**. O único runtime é o `llama-server` que viaja dentro do instalador; o app conversa com ele por HTTP em `127.0.0.1`, numa porta escolhida na hora de subir o processo.

> **Removido em 2026-07-27, na Fase 1+2 do M9.** Este documento descrevia integrações com **Ollama** (`:11434`), **LM Studio** (`:1234`) e um servidor OpenAI-compatible arbitrário informado por URL. As três saíram inteiras — código, tabelas e tela. O histórico do porquê está na AD-039; o que segue descreve o app que existe hoje.

## Runtime local (o único)

### `llama-server`, embutido

**Purpose:** carregar o `.gguf` ativo e responder o chat.
**Implementation:** `src-tauri/src/providers/llama_server.rs` (`LlamaServerClient` — struct concreta, sem trait).
**Origem do binário:** `src-tauri/resources/llama/{vulkan,cpu}/`, empacotado pelo Tauri como recurso do bundle. Versão fixada em `scripts/vendor.json` e trazida por `npm run vendor` antes de todo build.
**Configuration:** nada é configurável pelo usuário — a porta é livre, escolhida por `runtime::process::free_port`, e a URL nunca aparece na UI.
**Authentication:** nenhuma (processo local, filho do app).

**Endpoints usados:**

| Endpoint | Uso | Formato |
| --- | --- | --- |
| `GET /v1/models` | `health_check` e `model_limits` | `{ data: [{ id, meta: { n_ctx_train, n_ctx } }] }` — é `meta.n_ctx_train` que dá o teto de contexto (AD-029) |
| `POST /v1/chat/completions` | `stream_chat` | SSE, parseado por `providers/openai_stream.rs` |

**O que não é chamada HTTP:** modelo, tamanho de contexto e camadas de GPU são **flags de inicialização** do `llama-server` (`-m`, `-c`, `-ngl`). Mudar qualquer um reinicia o processo — foi a duplicação entre "configurar por HTTP" e "configurar por flag" que gerou o EMBED-12.

**Lista de modelos instalados:** lida dos arquivos `.gguf` na pasta de modelos, **não** de `/v1/models` — o servidor só conhece o modelo que está carregado (AD-028).

## Componentes binários empacotados

Nenhum deles é baixado em tempo de execução (SELF-09/SELF-12). Todos são resolvidos por `runtime::bundled` a partir do `resource_dir` do Tauri.

| Componente | Onde no bundle | Quem usa | Versão fixada em |
| --- | --- | --- | --- |
| `llama-server` (Vulkan + CPU) | `resources/llama/<backend>/` | `runtime::process` | `scripts/vendor.json` |
| ONNX Runtime | `resources/onnxruntime/` | `rag::embedding` via `ORT_DYLIB_PATH` | `scripts/vendor.json` |
| pdfium | `resources/pdfium/` | `rag::pdfium::extract_text` | `scripts/vendor.json` |

**Medição do vendoring (2026-07-27, Windows x64):** 120,5 MB no total — llama Vulkan 73,8 MB, llama CPU 23,1 MB, ONNX Runtime 16,2 MB, pdfium 7,4 MB. O ONNX Runtime extrai **425,9 MB** cru; 408 MB disso é um único `onnxruntime.pdb`, que a poda do script remove junto com `.lib`/`.exp`/headers.

## Rede: o que sobrou

Só duas coisas saem da máquina, ambas por ação explícita do usuário:

| O quê | Quando | Destino |
| --- | --- | --- |
| Download de um modelo GGUF | o usuário clica em baixar, no catálogo ou por link direto | Hugging Face (`resolve/main/*.gguf`) |
| Verificação de atualização | no boot, se o toggle estiver ligado (REL-13) | GitHub Releases |

### Catálogo de modelos

**Status:** **não integrado por API.** A lista é curada e embutida no binário (`src-tauri/src/models/catalog.rs`), com 6 entradas GGUF cujo `content-length` foi conferido ao vivo. O campo `provider` saiu junto com o multi-provider: toda entrada é um link `.gguf` direto.

**Consequência:** manter a lista atualizada é trabalho manual. O escape é o campo de download por link, que não passa por catálogo nem por checagem de RAM.

## Sistema operacional

### Detecção de memória

**Implementation:** `src-tauri/src/system_info.rs` via crate `sysinfo` 0.39.
**Uso:** `total_ram_gb()` alimenta o flag `fits_ram` de cada modelo curado. Retorna bytes desde sysinfo 0.26 (verificado no CHANGELOG do crate).
**Fallback:** se retornar 0 (ambientes exóticos), `list_downloadable_models` devolve `ram_detected_gb: None` e marca **todos** como cabendo — nunca esconde tudo silenciosamente.
**Não faz:** detecção de VRAM por GPU. A escolha Vulkan vs CPU vem de rodar `llama-server --list-devices` e ler a saída (AD-022), não de uma lib de detecção.

### Diálogo de arquivos

**Implementation:** plugin `tauri-plugin-dialog` 2, usado em `config_commands::pick_folder` (`blocking_pick_folder`).
**Permissão:** `dialog:default` em `capabilities/default.json`.

### Filesystem

**Implementation:** `std::fs` direto no Rust (não o plugin `fs` do Tauri).
**Escopo:** a pasta-base escolhida pelo usuário, o `config.json` no `app_config_dir` do SO, e **leitura** do `resource_dir` do próprio app.
**Validação:** `ensure_folder_structure` escreve um arquivo-sonda (`.localmind-write-test`) pra falhar cedo em pasta sem permissão.

## Webhooks

Nenhum — o app não expõe servidor HTTP nem recebe callbacks.

## Background Jobs

Não há fila nem scheduler. O que existe de assíncrono:

| Job | Mecanismo | Local |
| --- | --- | --- |
| Download de modelo com progresso | `tokio::sync::mpsc` + `tauri::async_runtime::spawn` re-emitindo `model-download-progress` | `runtime_commands::download_model` |
| Indexação de documento | `tauri::async_runtime::spawn` por documento | `rag::pipeline` |
| Autostart do sidecar no boot | `tauri::async_runtime::spawn` no `setup` | `lib.rs::autostart_sidecar` |

**Eventos Tauri emitidos** (backend → frontend):

| Evento | Payload | Consumidor |
| --- | --- | --- |
| `runtime-changed` | `()` | `runtimeStore.ts` — recarrega status e modelo ativo quando o sidecar termina de subir |
| `runtime-progress` | `{ stage, progress, message }` | `runtimeStore.ts` — o card do runtime durante o preparo |
| `model-download-progress` | `{ identifier, progress: PullProgress }` | `runtimeStore.ts` — `identifier` é a URL do `.gguf` |
| `chat-stream-chunk` | `{ chat_id, message_id, delta, done, error }` | `chatStore.ts` |
| `chat-retrieval-warning` | `{ chat_id, reason }` | `chatStore.ts` |
| `document-status` | `{ id, status, error_message }` | `documentsStore.ts` |
| `update-download-progress` | `{ downloaded, total }` | `updateStore.ts` (só no modo portátil) |
