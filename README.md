# LocalMind

Chat de IA 100% local com RAG sobre documentos, conectando a runtimes locais (Ollama, LM Studio) e empacotado como instalador único para Windows e Linux.

Ver planejamento completo em [`.specs/`](.specs/project/PROJECT.md) (visão, roadmap, decisões) e [`.specs/features/app-shell/spec.md`](.specs/features/app-shell/spec.md) (spec desta primeira fase).

## Pré-requisitos

| Ferramenta | Necessário para | Instalação |
| --- | --- | --- |
| **Node.js 18+** | Frontend (Vite/React) | já instalado nesta máquina |
| **Rust (rustup/cargo)** | Backend Tauri — **obrigatório**, ainda não instalado nesta máquina | https://www.rust-lang.org/tools/install |
| **Windows: "Desktop development with C++"** | Linker MSVC exigido pelo Rust no Windows | Visual Studio Build Tools: https://visualstudio.microsoft.com/visual-cpp-build-tools/ |
| **Linux: libwebkit2gtk, build-essential, etc.** | Webview e build no Linux | ver checklist oficial: https://tauri.app/start/prerequisites/ |
| **WebView2 Runtime (Windows)** | Renderização da UI | normalmente já vem no Windows 11 |
| **protoc (Protocol Buffers)** | Exigido pelo `lance-encoding`, dependência do `lancedb`. **Sem ele o `cargo build` falha** com *"Could not find `protoc`"* | Windows: `winget install Google.Protobuf` · Linux: `apt install protobuf-compiler` |

O **ONNX Runtime não** é pré-requisito de build: o app baixa a biblioteca na
primeira indexação de documento, assim como faz com o binário do llama.cpp e com
o pdfium.

Checklist oficial e completo por SO: https://tauri.app/start/prerequisites/

Depois de instalar o Rust, feche e reabra o terminal e confirme:

```bash
rustc --version
cargo --version
```

## Instalar dependências do projeto

```bash
npm install
```

## Rodar em desenvolvimento

Abre a janela nativa do app com hot-reload do frontend e do backend Rust:

```bash
npm run tauri dev
```

> `npm run dev` sozinho só sobe o Vite no navegador (`http://localhost:1420`) — útil para iterar rápido na UI, mas **sem** acesso aos comandos Tauri/SQLite (invoke falha fora da janela nativa). Para testar o fluxo completo (criar/listar/renomear/excluir chats), use sempre `npm run tauri dev`.

## Gerar o instalador final (build)

```bash
npm run tauri build
```

Gera o instalador **para o sistema operacional em que o comando é executado** (Tauri não faz cross-compile de bundle nativo por padrão):

- **Rodando no Windows** → `.msi` e `.exe` (NSIS) em `src-tauri/target/release/bundle/{msi,nsis}/`
- **Rodando no Linux** → `.AppImage` e `.deb` em `src-tauri/target/release/bundle/{appimage,deb}/`

Como esta máquina é Windows, `npm run tauri build` aqui produz apenas os instaladores Windows. Os artefatos das duas plataformas saem juntos pelo CI.

## Publicar uma release

Releases são **manuais**: nenhum push publica nada. Vá em **Actions → Release →
Run workflow** e escolha `patch`, `minor` ou `major`; a execução cuida de versão,
CHANGELOG, tag, instaladores (`.msi`, `-setup.exe`, `.deb`, `.AppImage`), bundle
portátil e do manifesto de auto-update.

O passo-a-passo, o setup da chave de assinatura e o que fazer quando o workflow
falha no meio estão em **[docs/RELEASING.md](docs/RELEASING.md)**.

Build só do frontend (sem empacotar o app nativo), útil para checar erros de TypeScript/Vite:

```bash
npm run build
```

## Estrutura

```
src/                    Frontend React + TypeScript + Tailwind
  components/Sidebar/    Sidebar de 3 zonas: Chats · Documentos · Conexões
  components/Chat/        Painel de chat
  store/                  Estado (Zustand)
  lib/                    Wrappers tipados dos comandos Tauri (invoke)
src-tauri/              Backend Rust
  src/db.rs               Conexão SQLite + migrações
  src/models.rs            Structs Chat/Message
  src/commands.rs          Comandos expostos ao frontend (create_chat, list_chats, ...)
.specs/                 Planejamento (spec-driven): visão, roadmap, specs de feature, decisões
```

## Status

**M1 — Fundação & Shell**: implementado (sidebar de 3 zonas, CRUD de chats persistido em SQLite). Ainda não verificado em execução real nesta máquina por falta do toolchain Rust — ver `.specs/project/STATE.md` (bloqueador B-001).
