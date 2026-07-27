# Project Structure

**Root:** `D:\chat-ia-local`

## Directory Tree

```
chat-ia-local/
├── .specs/                     # Spec-driven docs (este sistema)
│   ├── project/                # PROJECT.md · ROADMAP.md · STATE.md
│   ├── codebase/               # Brownfield mapping (estes arquivos)
│   └── features/               # spec.md/design.md/tasks.md por feature
├── src/                        # Frontend React + TS
│   ├── components/
│   │   ├── Chat/               # ChatPanel.tsx
│   │   ├── Connections/        # ConnectionsPanel · ConnectionsList · ModelsList
│   │   │                       #   · ModelDownloadCard · ModelConfigForm
│   │   ├── Onboarding/         # Wizard.tsx
│   │   ├── Settings/           # SettingsPanel.tsx
│   │   └── Sidebar/            # Sidebar · ChatList · DocumentsSection
│   │                           #   · RuntimeSection · SettingsSection
│   ├── i18n/                   # index.ts + locales/{en,pt}.json
│   ├── lib/                    # chatApi · configApi · documentsApi · runtimeApi · updateApi · theme
│   ├── store/                  # chatStore · configStore · uiStore · runtimeStore · documentsStore · updateStore
│   ├── styles/themes.css       # CSS variables por tema
│   ├── App.tsx · main.tsx · types.ts · index.css
├── src-tauri/                  # Backend Rust
│   ├── src/
│   │   ├── models/             # mod.rs (Chat/Message) · catalog.rs · memory_estimate.rs
│   │   ├── providers/          # mod.rs · llama_server.rs · openai_stream.rs
│   │   ├── runtime/            # bundled · detect · download · job · log
│   │   │                       #   · model · process · store
│   │   ├── rag/                # chunking · embedding · onnxruntime · parsing
│   │   │                       #   · pdfium · pipeline · store
│   │   ├── chat/               # attachments · cancellation · context_assembler · memory
│   │   ├── update/             # manifest · portable · signature
│   │   ├── lib.rs · main.rs
│   │   ├── commands.rs · chat_commands.rs · config_commands.rs
│   │   │   · document_commands.rs · runtime_commands.rs · update_commands.rs
│   │   └── config.rs · db.rs · system_info.rs
│   ├── resources/              # componentes vendorizados (gerado, gitignored)
│   ├── capabilities/default.json
│   ├── icons/ · Cargo.toml · tauri.conf.json · build.rs
├── dist/                       # Build do Vite (gerado, gitignored)
├── public/ · index.html
└── package.json · tsconfig.json · vite.config.ts · postcss.config.js
```

## Module Organization

### Comandos Tauri (fronteira frontend↔backend)

**Purpose:** Única porta de entrada do frontend pro backend. Todo `#[tauri::command]` vive num arquivo `*_commands.rs`, nunca misturado com lógica de domínio.
**Location:** `src-tauri/src/{commands,chat_commands,config_commands,document_commands,runtime_commands,update_commands}.rs`
**Key files:** `lib.rs` registra todos no `invoke_handler![]` — se não está lá, o frontend não enxerga.

### Domínio / lógica

**Purpose:** Lógica pura e orquestração, sem anotação Tauri — testável isoladamente.
**Location:** `src-tauri/src/{config,db,system_info}.rs`, `models/`, `providers/`, `runtime/`, `rag/`, `chat/`, `update/`
**Key files:** `providers/llama_server.rs` (o cliente HTTP do sidecar), `runtime/store.rs` (a linha singleton que responde "qual modelo, com qual contexto, com qual GPU"), `runtime/bundled.rs` (onde cada componente do instalador é encontrado)

### Camada de dados do frontend

**Purpose:** Espelhar cada comando Tauri num wrapper tipado e expor estado via Zustand.
**Location:** `src/lib/*Api.ts` (wrappers `invoke`) + `src/store/*Store.ts` (estado)
**Key files:** `src/types.ts` — todas as interfaces que cruzam a fronteira Rust↔TS moram aqui, num arquivo só.

### UI

**Purpose:** Componentes React, um diretório por área funcional.
**Location:** `src/components/<Área>/`
**Key files:** `App.tsx` faz o roteamento (não há react-router — é um switch em `uiStore.activeView`).

## Where Things Live

**Chats (M1):**
- UI: `src/components/Sidebar/ChatList.tsx`, `src/components/Chat/ChatPanel.tsx`
- Estado: `src/store/chatStore.ts` · API: `src/lib/chatApi.ts`
- Backend: `src-tauri/src/commands.rs` · Modelos: `src-tauri/src/models/mod.rs`

**Config/Storage/i18n (M2):**
- UI: `src/components/Onboarding/Wizard.tsx`, `src/components/Settings/SettingsPanel.tsx`
- Estado: `src/store/configStore.ts` · API: `src/lib/configApi.ts`
- Backend: `src-tauri/src/config.rs` + `config_commands.rs`
- Bootstrap: `config.json` no `app_config_dir` do SO (**fora** da pasta-base — AD-012)

**Conexões & Modelos (M3):**
- UI: `src/components/Connections/*` + `Sidebar/ConnectionsSection.tsx`
- Estado: `src/store/connectionsStore.ts` · API: `src/lib/connectionsApi.ts`
- Backend: `connections.rs`, `connection_commands.rs`, `model_commands.rs`, `providers/`, `system_info.rs`

## Special Directories

**Pasta-base do usuário** (escolhida no wizard, fora do repo — AD-008):
**Purpose:** Todos os dados reais do usuário.
**Conteúdo:** `localmind.db` (SQLite), `models/`, `documents/`, `vectors/`, `chats/` — criados por `config::ensure_folder_structure`.

**`src-tauri/target/`** e **`dist/`**: build artifacts, ambos gitignored.

**`.specs/`**: documentação do processo, não código — mas é a fonte da verdade sobre decisões (STATE.md).
