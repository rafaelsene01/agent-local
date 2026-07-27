# Tech Stack

**Analyzed:** 2026-07-25 (após M3 / `connections-models`)

## Pré-requisitos de build (além do Rust e do Node)

- **protoc** (Protocol Buffers compiler) — exigido pelo `lance-encoding`, dependência do `lancedb`. Sem ele o `cargo build` falha com *"Could not find `protoc`"*. No Windows: `winget install Google.Protobuf` (verificado com 35.1). No Linux: `apt install protobuf-compiler`.
- **ONNX Runtime** — **não** é pré-requisito de build: o `fastembed` roda em modo `ort-load-dynamic` e o app baixa o `onnxruntime.dll`/`.so` na primeira indexação, porque a lib estática do ORT exige a STL do MSVC 2022 (VS 2019 Build Tools não serve).

## Core

- Framework: Tauri 2 (Rust backend + webview nativo do SO) — AD-001
- Language: Rust (edition 2021) no backend; TypeScript ~5.8 no frontend
- Runtime: webview do SO (WebView2 no Windows / WebKitGTK no Linux); binário Rust como processo host
- Package manager: npm (frontend) + cargo (backend)
- Crate name: `tauri-app` / lib `tauri_app_lib`; produto `LocalMind`, identifier `com.localmind.app`

## Frontend

- UI Framework: React 19 (`react` ^19.1, `react-dom` ^19.1)
- Build: Vite 7 (`@vitejs/plugin-react`), dev server fixo em `:1420` (exigido por `tauri.conf.json` `devUrl`)
- Styling: Tailwind CSS v4 via `@tailwindcss/postcss` — **sem `tailwind.config.js`** (config CSS-first, AD-006). Temas por CSS variables em `src/styles/themes.css`
- State Management: Zustand ^5 (4 stores independentes, sem store raiz)
- i18n: i18next ^26 + react-i18next ^17 — EN default, PT disponível (AD-007)
- Ícones: lucide-react ^1.26
- Form Handling: nenhuma lib — `useState` + `onSubmit` manual

## Backend

- API Style: comandos Tauri (`#[tauri::command]` + `invoke_handler`), não HTTP. 30 comandos registrados em `lib.rs`
- Database: SQLite via `rusqlite` 0.31 (feature `bundled` — compila o SQLite junto, sem dependência do SO). Sem ORM; SQL literal com `params![]`
- HTTP client: `reqwest` 0.12 (features `json`, `stream`) para falar com o sidecar `llama-server` em `127.0.0.1` e para baixar modelos GGUF
- Async: `tokio` 1 (features `sync`, `time`) + `tauri::async_runtime`; `futures-util` 0.3 para `bytes_stream()`. **`async-trait` saiu na AD-042** junto com o trait `ProviderClient` — não há mais despacho dinâmico
- Serialização: `serde` 1 (derive) + `serde_json` 1
- IDs: `uuid` 1 (v4); timestamps `chrono` 0.4 em RFC3339 (string)
- Sistema: `sysinfo` 0.39 (RAM total)

## Plugins Tauri

- `tauri-plugin-opener` 2 (do template, pouco usado)
- `tauri-plugin-dialog` 2 (`pick_folder` no wizard/configurações)
- Capability única `default` (`src-tauri/capabilities/default.json`): `core:default`, `opener:default`, `dialog:default`

## Testing

- Unit: `cargo test` nativo (150 testes + 9 `#[ignore]`, todos em `#[cfg(test)] mod tests` co-locados)
- Integration: nenhum runner configurado
- E2E: nenhum
- Frontend: **nenhum** framework de teste instalado (sem Vitest/RTL) — ver CONCERNS.md
- Detalhes e gates: `.specs/codebase/TESTING.md`

## External Services

**Nenhum.** Desde o M9 o app não fala com programa externo algum. O único runtime é o `llama-server` que viaja no instalador (`resources/llama/{vulkan,cpu}/`) e roda como processo filho em `127.0.0.1`.

O que sai da máquina, sempre por ação explícita do usuário: o download de um modelo GGUF (Hugging Face) e a verificação de atualização (GitHub Releases, com toggle de opt-out). Nenhum serviço de nuvem, telemetria ou auth externa.

## Development Tools

- Compilador Rust: rustc/cargo 1.97.1, toolchain `stable-x86_64-pc-windows-msvc` (MSVC Build Tools necessários no Windows)
- CLI: `@tauri-apps/cli` ^2 (`npm run tauri dev` / `build`)
- Type check: `tsc` roda antes do Vite build (`npm run build` = `tsc && vite build`)
- Linter/formatter: **nenhum configurado** (sem ESLint, Prettier, rustfmt.toml, clippy.toml) — ver CONCERNS.md
- CI: **nenhum** (sem `.github/workflows`) — planejado para M8
