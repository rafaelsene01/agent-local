# Code Conventions

Observado em ~25 arquivos (todo o `src-tauri/src/` e `src/`). Convenções **em uso**, não ideais — não há linter/formatter configurado, então elas são mantidas por consistência manual.

## Naming Conventions

**Arquivos Rust:** `snake_case.rs`. Comandos Tauri sempre em arquivo com sufixo `_commands`.
Exemplos: `config_commands.rs`, `connection_commands.rs`, `model_commands.rs`, `system_info.rs`, `memory_estimate.rs`

**Arquivos React:** `PascalCase.tsx`, um componente exportado por arquivo, nome do arquivo = nome do componente.
Exemplos: `ConnectionsPanel.tsx`, `ModelDownloadCard.tsx`, `SettingsSection.tsx`

**Módulos de apoio TS:** `camelCase.ts`, com sufixo por papel — `*Api.ts` para wrappers `invoke`, `*Store.ts` para Zustand.
Exemplos: `connectionsApi.ts`, `chatStore.ts`, `theme.ts`

**Funções Rust:** `snake_case`, verbo primeiro. Comandos Tauri usam o mesmo nome que o frontend invoca.
Exemplos: `list_connections`, `create_connection`, `total_ram_gb`, `estimate_ram_gb`, `ensure_folder_structure`

**Funções/hooks TS:** `camelCase`; stores exportados como `useXStore`; handlers de evento como `handleX`.
Exemplos: `useConnectionsStore`, `loadDownloadableModels`, `handleChangeFolder`, `handleManualPull`

**Constantes:** `SCREAMING_SNAKE_CASE` nos dois lados.
Exemplos: `SCHEMA`, `SUBDIRS`, `CURATED_MODELS` (Rust); `SUPPORTED_THEMES`, `DEFAULT_LANGUAGE`, `STATUS_DOT`, `THEME_LABEL_KEYS` (TS)

**Campos que cruzam a fronteira:** `snake_case` (o serde não renomeia). Por isso `src/types.ts` tem `base_url`, `size_bytes`, `estimated_ram_gb` — quebrando o camelCase idiomático do TS de propósito, pra bater com o Rust.
**Exceção:** parâmetros de `invoke()` são `camelCase` no TS e chegam `snake_case` no Rust — o Tauri faz essa conversão sozinho (`invoke("pull_model", { connectionId })` → `connection_id: String`).

## Code Organization

**Ordem de imports (Rust):** `crate::` primeiro, depois crates externos em ordem alfabética.
```rust
use crate::connections::{self, Connection, ConnectionManager};
use crate::db::DbState;
use chrono::Utc;
use rusqlite::{params, Connection as SqlConnection};
use tauri::State;
```

**Ordem de imports (TS):** React → libs externas → ícones → stores/lib internos → componentes → `type` imports por último.
```tsx
import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { Download, Settings2 } from "lucide-react";
import { useConnectionsStore } from "../../store/connectionsStore";
import { ModelConfigForm } from "./ModelConfigForm";
```

**Estrutura de arquivo Rust:** imports → tipos/structs → `impl` → funções livres → `#[cfg(test)] mod tests` **no fim do mesmo arquivo** (nunca em `tests/` separado).

**Estrutura de componente React:** hooks primeiro (na ordem: `useTranslation`, stores, `useState`, `useEffect`), depois handlers, depois `return`. Early return (`if (!config) return null;`) antes do JSX.

## Type Safety / Documentation

**Rust:** tipos explícitos nas assinaturas públicas; `#[derive(Debug, Serialize, Clone)]` no que sai pro frontend, `Deserialize` no que entra. Structs de resposta HTTP são privadas ao módulo (`struct ModelsResponse` em `llama_server.rs`) e nunca vazam pro frontend — sempre convertidas pro tipo comum (`InstalledModel`, `ModelLimits`).

**TypeScript:** `strict` ligado (via `tsconfig.json`). Union types de string literal em vez de enum:
```ts
export type ConnectionStatus = "available" | "unavailable" | "unknown";
export type ActiveView = "chat" | "settings" | "connections";
```
Isso força o `Record<Theme, string>` a mapear todas as chaves — TypeScript quebra o build se alguém adiciona um tema e esquece o label (comportamento aproveitado de propósito, AD-013).

## Error Handling

**Rust:** `Result<T, String>` em **toda** fronteira de comando — nunca `anyhow`, `thiserror` ou tipo de erro customizado exposto. Conversão via `.map_err(|e| e.to_string())`.
```rust
conn.execute(...).map_err(|e| e.to_string())?;
```
**Exceção deliberada:** `providers/` tem um enum real (`ProviderError { Unavailable, RequestFailed, ParseError }`) porque precisa **distinguir** "servidor offline" de "resposta malformada" — mas ele é achatado pra `String` na borda do comando.

**Mensagens de erro de usuário:** em **português**, mesmo com o código em inglês.
```rust
.ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())
```

**TypeScript:** stores capturam e guardam como string, nunca propagam exception pra UI:
```ts
try { const chats = await chatApi.listChats(); set({ chats, isLoading: false }); }
catch (err) { set({ error: String(err), isLoading: false }); }
```
**Exceção:** `connectionsStore.configureModel` **re-lança** de propósito, porque o componente precisa do `ConfigApplied` de retorno e trata o erro localmente.

## Comments / Documentation

**Estilo:** comentários explicam **por quê**, nunca o quê. Densidade baixa — a maioria das funções não tem nenhum. Usados quase só para:

1. **Decisões não-óbvias**, geralmente citando o motivo real:
```rust
/// Small bootstrap pointer file that lives in the OS-standard app config dir.
/// […] This indirection is what lets the storage folder be reconfigurable
/// without knowing it in advance.
```

2. **Divergências entre spec/design e realidade**, com marcador padronizado:
```rust
// SPEC_DEVIATION: tasks.md kept the old command names after the rename […]
// Renamed here so runtimeApi.ts can map one function per registered command
```

3. **Fatos verificados** que ficariam invisíveis no código:
```rust
/// `System::total_memory()` returns bytes (since sysinfo 0.26.0).
```

**Idioma:** comentários e commits em **inglês**; strings de UI e mensagens de erro pro usuário em **português** (ou chave i18n). Docs em `.specs/` em português.

## i18n

Nenhuma string literal visível ao usuário no JSX — sempre `t("chave.aninhada")`, com a chave adicionada nos **dois** arquivos (`en.json` e `pt.json`) no mesmo commit. Interpolação no padrão i18next: `t("connections.configureModel", { model: m.name })` ↔ `"Configure {{model}}"`.

## Estilo visual

Zero cor hardcoded — sempre CSS variable via Tailwind arbitrary value:
```tsx
className="bg-[var(--bg-elevated)] text-[var(--text-secondary)] border-[var(--border-color)]"
```
As variáveis vivem em `src/styles/themes.css`, um bloco por `[data-theme=…]`.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/), escopo = nome da feature, corpo explicando o **porquê** e registrando qualquer `SPEC_DEVIATION`. Um commit por task.
```
feat(runtime): start llama-server from the bundled binaries
```
