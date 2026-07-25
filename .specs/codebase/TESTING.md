# Testing

**Status:** Sem suíte de testes automatizada ainda. Projeto solo, greenfield, verificado até aqui manualmente (`cargo check` + `npm run build` + rodar `npm run tauri dev` e observar log/processo). Este documento existe para dar aos `tasks.md` um gate real em vez de inventar um framework que não existe no projeto.

## Gate Check Commands

| Gate | Command | O que valida |
| --- | --- | --- |
| `build` (Rust) | `cd src-tauri && cargo check` | Backend compila, sem erros de tipo/borrow checker |
| `build` (frontend) | `npm run build` | `tsc` sem erros + Vite builda |
| `full` | `npm run tauri dev` até log mostrar `Finished` + `Running` sem erro, processo `tauri-app.exe` fica de pé | App real sobe ponta a ponta |

## Test Coverage Matrix

| Code Layer | Test Type Required | Justificativa |
| --- | --- | --- |
| Funções puras Rust (parsing, fórmula de RAM, chunking, montagem de contexto) | unit (`cargo test`) | Lógica testável sem I/O; barato de cobrir, alto valor (bugs aqui corrompem RAG silenciosamente) |
| Comandos Tauri (`#[tauri::command]`) que só orquestram I/O (DB, HTTP, filesystem) | none (por ora) | Sem test runner de integração Tauri configurado ainda; verificação é manual via `tauri dev` |
| Componentes React | none (por ora) | Sem Vitest/RTL configurado ainda; verificação é manual (rodar o app) |
| Parsers de documento / providers HTTP (Ollama, LM Studio) | unit com fixtures/mocks quando prático | Evita depender de um Ollama real rodando para `cargo test` passar |

**Parallelism Assessment:** Nenhum teste automatizado além de `cargo test` roda hoje; `cargo test` é seguro para paralelizar (Rust testa em threads por padrão). Tasks marcadas `[P]` só precisam não compartilhar o mesmo arquivo.

## Todo

- [ ] Introduzir `cargo test` para os módulos de lógica pura à medida que forem criados (não é bloqueante para as features atuais, mas cada task de lógica pura abaixo já inclui seus testes unitários)
- [ ] Avaliar Vitest/React Testing Library quando a superfície de componentes estabilizar
