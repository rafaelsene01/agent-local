# Changelog

Todas as mudanças relevantes deste projeto são registradas aqui.
O formato segue [Conventional Commits](https://www.conventionalcommits.org/)
e o versionamento segue [SemVer](https://semver.org/lang/pt-BR/).

## [0.1.1] - 2026-07-26

### Correções

- **embedded-runtime:** Make context/GPU config actually reach llama-server
- **release:** Update release workflow to exclude tauri.conf.json from version bump
- **ci:** Update Node.js version and test script glob pattern

### Documentação

- **connections:** Mark connections-models (M3) complete in specs
- **embedded-runtime:** Specify M7 (embedded llama.cpp fallback)
- **codebase:** Add brownfield mapping (STACK, ARCHITECTURE, CONVENTIONS, STRUCTURE, INTEGRATIONS, CONCERNS)
- Plan single-active-connection (M3.1) and design embedded-runtime (M7)
- **connections:** Revoke AD-016 in favor of a single global active pair
- **embedded-runtime:** Close M7 with what was and wasn't verified
- Record the requirement-by-requirement audit findings
- Close M5 and M4, recording what was verified and what wasn't

### Manutenção

- Initial commit of M1+M2 baseline (shell, config, storage, i18n)

### Novidades

- **connections:** Add connections and model_configs tables
- **connections:** Add RamDetector for total system memory
- **connections:** Add curated model catalog and RAM estimate formula
- **connections:** Add ProviderClient trait and shared provider types
- **connections:** Implement OllamaClient
- **connections:** Implement LmStudioClient
- **connections:** Add ConnectionManager and CRUD persistence
- **connections:** Expose connection management as Tauri commands
- **connections:** Expose model listing, download and config as Tauri commands
- **connections:** Add get_active_model command
- **connections:** Add connectionsApi and connectionsStore
- **connections:** Turn ConnectionsSection into a nav item (AD-014)
- **connections:** Add ConnectionsPanel shell and ConnectionsList
- **connections:** Add ModelsList and ModelDownloadCard
- **connections:** Add ModelConfigForm for context/GPU settings
- **connections:** Wire ConnectionsPanel into App.tsx
- **db:** Apply schema changes through versioned migrations
- **db:** Rename connections.enabled to is_active (migration 2)
- **connections:** Make connection activation exclusive
- **connections:** Activate model and its connection as one pair
- **connections:** Enforce a single active connection and model
- **embedded-runtime:** Add embedded_runtime table and runtime/ folder
- **embedded-runtime:** Resolve the latest llama.cpp release and pick an asset
- **embedded-runtime:** Download with progress and extract zip/tar.gz
- **embedded-runtime:** Probe GPU support by asking the binary itself
- **embedded-runtime:** Pin the default model URL after verifying it live
- **embedded-runtime:** Manage the llama-server child process
- **embedded-runtime:** Expose the sidecar as an ordinary ProviderClient
- **embedded-runtime:** Orchestrate setup and tie the sidecar to app lifetime
- **embedded-runtime:** Seed the embedded connection and route it
- **embedded-runtime:** Add the setup UI for the built-in runtime
- **documents:** Add import, background indexing pipeline and global RAG
- **chat:** Add message sending, streaming and per-chat attachments
- **chat:** Enhance chat functionality with global RAG and attachment handling
- **update:** Implement automatic update checks and user settings

### Refatoração

- **db:** Centralize require_conn and check connections concurrently

### Testes

- **rag:** Verify embeddings, vector store and SSE parsing for real

