# Architecture

**Pattern:** Monolito desktop de duas camadas — webview React (apresentação) sobre um core Rust (domínio + I/O), acopladas apenas por comandos Tauri e eventos. Não há servidor nem rede externa obrigatória. Há **um** processo separado: o `llama-server`, sidecar filho do app, iniciado e morto por ele.

## High-Level Structure

```mermaid
graph TD
    subgraph WV["Webview — React 19 + TS"]
        APP["App.tsx<br/>switch em uiStore.activeView"]
        COMP["Components/<br/>Sidebar · Chat · Runtime · Documents · Settings · Onboarding"]
        STORE["Zustand stores<br/>chat · config · ui · runtime · documents"]
        API["lib/*Api.ts<br/>wrappers invoke() tipados"]
    end
    subgraph RS["Core — Rust (tauri_app_lib)"]
        CMD["*_commands.rs<br/>#[tauri::command]"]
        DOM["Domínio<br/>config · runtime · db · rag · system_info · models"]
        PROV["providers/<br/>LlamaServerClient (struct concreta)"]
    end
    subgraph EXT["Fora do processo"]
        SQL[("SQLite<br/>localmind.db")]
        SIDE["llama-server<br/>sidecar filho em 127.0.0.1"]
        RES[("resources/<br/>llama · onnxruntime · pdfium")]
        FS[("Pasta-base<br/>models/ documents/ vectors/ chats/")]
    end
    COMP --> STORE --> API -->|invoke| CMD
    CMD --> DOM --> PROV
    CMD -->|emit event| STORE
    DOM --> SQL
    DOM --> FS
    DOM -->|spawn| SIDE
    RES --> SIDE
    PROV -->|HTTP| SIDE
```

## Identified Patterns

### Comando Tauri como única fronteira

**Location:** `src-tauri/src/*_commands.rs` → registrados em `lib.rs` `invoke_handler![]`
**Purpose:** O frontend nunca toca SQL, filesystem ou HTTP — só chama comandos.
**Implementation:** Cada comando recebe `State<DbState>` e/ou `AppHandle`, valida, delega pro domínio, devolve `Result<T, String>` (erro sempre `String`, nunca tipo customizado).
**Example:** `connection_commands::list_connections` → `connections::list_connections(sql)` + `ConnectionManager::refresh_status`.

### `DbState` = `Mutex<Option<Connection>>`

**Location:** `src-tauri/src/db.rs:5`
**Purpose:** O banco só existe **depois** que o usuário escolhe a pasta-base no wizard (AD-011). `None` = ainda não configurado.
**Implementation:** Todo comando que precisa do banco chama um helper local `require_conn(&guard)` que converte `None` num erro amigável. Esse helper está **duplicado** em 3 arquivos (ver CONCERNS.md).
**Example:** `commands.rs:8`, `connection_commands.rs:7`, `model_commands.rs:11`.

### Schema como string única idempotente

**Location:** `src-tauri/src/db.rs` (`const SCHEMA`)
**Purpose:** "Migração" é só um `execute_batch` de `CREATE TABLE IF NOT EXISTS` rodado em todo `db::open()`.
**Implementation:** Adicionar tabela = concatenar no `SCHEMA`. Não há versionamento nem migração destrutiva — funciona porque o projeto é greenfield e nenhuma tabela mudou de forma ainda (ver CONCERNS.md).

### Um cliente concreto, sem trait (AD-039)

**Location:** `src-tauri/src/providers/llama_server.rs`
**Purpose:** Falar HTTP com o sidecar.
**Implementation:** `LlamaServerClient` é uma struct, não um `impl` de trait. Até o M9 havia um `trait ProviderClient` com `Box<dyn>`, quatro implementadores e um `match` de provedor — cerimônia que só se paga quando há de fato mais de um provedor para escolher. `runtime_commands::client(&app)` devolve o cliente já apontado para a porta que o processo escolheu; com o sidecar parado, as chamadas reportam `Unavailable`, que é um **estado**, não um erro a tratar.
**Trade-off registrado:** perdeu-se o escape hatch de apontar para um servidor OpenAI-compatible externo. Foi escolha explícita do usuário (AD-039).

### Progresso longo via evento, não polling

**Location:** `runtime_commands::download_model` → `app.emit("model-download-progress", …)`
**Purpose:** Operações longas (download de modelo) empurram progresso pro frontend.
**Implementation:** O comando cria um `tokio::sync::mpsc::channel`, passa o `Sender` para o downloader, e sobe uma task (`tauri::async_runtime::spawn`) que drena o `Receiver` e re-emite cada item como evento Tauri. O frontend escuta com `listen()` no escopo do módulo do store.
**Example:** `runtimeStore.ts` (fim do arquivo) — indexa o progresso pela **URL** do `.gguf`, que é a única identidade que um download tem agora.

### Nav + painel de tela cheia (AD-014)

**Location:** `src/store/uiStore.ts` + `App.tsx` + `components/Sidebar/*Section.tsx`
**Purpose:** Cada área da sidebar é só um botão de navegação; o conteúdo abre num painel à direita que substitui o `ChatPanel`.
**Implementation:** `uiStore.activeView: "chat" | "settings" | "runtime" | "documents"`; `App.tsx` faz um ternário aninhado. Sem react-router.
**Example:** `SettingsSection.tsx` é o template canônico; `RuntimeSection.tsx` segue ele.

### Store Zustand por domínio

**Location:** `src/store/*.ts`
**Purpose:** Estado + ações no mesmo objeto, sem reducers/actions separados.
**Implementation:** `create<State>((set, get) => ({ …dados, …ações }))`. Toda ação assíncrona segue o mesmo shape: `try { await api…; set({…}) } catch (err) { set({ error: String(err) }) }`. Erro é sempre `string | null` no store.

## Data Flow

### Boot / onboarding

1. `App.tsx` monta → `configStore.loadConfig()` → `invoke("get_app_config")`
2. Backend lê `config.json` do `app_config_dir` (**não** da pasta-base — AD-012, resolve o ovo-e-galinha de "onde está a pasta-base?")
3. Sem config ou `onboarding_completed: false` → `status: "needs-onboarding"` → renderiza `Wizard`
4. Wizard chama `complete_onboarding(basePath, theme, language)` → backend cria as 4 subpastas, abre/cria o `localmind.db`, popula `DbState`, salva o `config.json`
5. `status: "ready"` → renderiza `Sidebar` + painel. **Só a partir daqui** qualquer comando que precise do banco é chamado.

### Preparar o runtime (M9)

1. `RuntimeCard` monta → `runtimeStore.loadStatus()` → `invoke("runtime_status")`
2. Sem `binary_path` gravado, o estágio é `not_prepared` e o card oferece **Preparar**
3. `prepare_runtime` resolve `resources/llama/vulkan/llama-server` pelo `resource_dir`, roda `--list-devices` e lê a saída (`probe_devices`). GPU → `-ngl -1`; só CPU → `-ngl 0`; binário que não executa → cai para o backend CPU embutido. **Nenhuma requisição HTTP acontece aqui.**
4. Grava só a metade "motor" da linha `embedded_runtime` — o modelo já escolhido sobrevive a um re-preparo
5. Sem modelo, o estágio é `no_model`, que é o estado normal de uma instalação nova; com modelo, o sidecar sobe e o estágio vira `running`

### Download de modelo (M9)

1. `ModelsList` → `downloadModel(url)` → `invoke("download_model", { url })`
2. Backend valida que a URL termina em `.gguf`, cria o canal mpsc e baixa para a pasta de modelos
3. Cada item vira evento `model-download-progress` com a URL como `identifier`; o store atualiza `downloadProgress[url]`; o card re-renderiza a barra
4. No fim, um quadro `success`/`error` fecha a barra — sem ele, um download concluído ficaria parado na última porcentagem

## Code Organization

**Approach:** Híbrido — backend por **camada** (comandos / domínio / providers), frontend por **área funcional** (`components/Runtime/`, `components/Chat/`).

**Module boundaries:**
- `src/types.ts` é o contrato compartilhado: toda struct Rust com `#[derive(Serialize)]` que cruza a fronteira tem uma `interface` espelhada lá. **Não há geração automática** — o espelhamento é manual (ver CONCERNS.md).
- `providers/` só conhece HTTP e o trait; não conhece SQLite nem Tauri.
- `*_commands.rs` conhece tudo, mas não implementa nada.
