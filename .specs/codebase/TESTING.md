# Testing

**Status (2026-07-27):** **169 testes Rust passando e 11 `#[ignore]`**, mais **43 testes de script** em `node --test`. Frontend continua sem runner.

> **Este cabeçalho dizia "Sem suíte de testes automatizada ainda" até 2026-07-27**, quando o M6 foi implementado — muito depois de a suíte existir e crescer para quase 170 testes. Era exatamente a divergência que o `AGENTS.md` manda corrigir: um leitor podia acreditar no documento e concluir que não havia nada para rodar. Corrigido junto com a AD-044.

## Gate Check Commands

| Gate | Command | O que valida |
| --- | --- | --- |
| `quick` (Rust) | `cd src-tauri && cargo test --lib <módulo>::` | O módulo que a task mexeu |
| `quick` (scripts) | `npm run test:scripts` | Os scripts Node de release e vendoring |
| `build` (Rust) | `cd src-tauri && cargo check` | Backend compila, sem erros de tipo/borrow checker |
| `build` (frontend) | `npm run build` | `tsc` sem erros + Vite builda |
| `full` | `cargo test --lib` inteiro + `npm run tauri dev` até log mostrar `Finished` + `Running` sem erro, processo de pé | Suíte completa e app real subindo ponta a ponta |

**Os `#[ignore]` não são testes desligados** — são os que tocam um recurso real e por isso não pesam na suíte padrão. Dois formatos em uso: o que **cria** o recurso numa pasta temporária (`rag::store`, `chat::memory`) e o que usa um recurso **já existente na máquina**, sempre por variável de ambiente e nunca por caminho adivinhado (`db::real_database`, `runtime::detect::detect_real`, `runtime::process::sidecar_real`). Rodar: `cargo test --lib <módulo> -- --ignored`.

## Test Coverage Matrix

| Code Layer | Test Type Required | Justificativa |
| --- | --- | --- |
| Funções puras Rust (parsing, fórmula de RAM, chunking, montagem de contexto) | unit (`cargo test`) | Lógica testável sem I/O; barato de cobrir, alto valor (bugs aqui corrompem RAG silenciosamente) |
| Comandos Tauri (`#[tauri::command]`) que só orquestram I/O (DB, HTTP, filesystem) | none (por ora) | Sem test runner de integração Tauri configurado ainda; verificação é manual via `tauri dev` |
| Componentes React | none (por ora) | Sem Vitest/RTL configurado ainda; verificação é manual (rodar o app) |
| Parsers de documento / o cliente HTTP do `llama-server` | unit com fixtures/mocks quando prático | Evita depender de um sidecar de pé para `cargo test` passar; o caminho real é coberto pelos `#[ignore]` |

**Parallelism Assessment:** `cargo test` é seguro para paralelizar (Rust testa em threads por padrão), e os `#[ignore]` que escrevem em disco derivam o diretório do PID do processo, então não colidem entre si. Tasks marcadas `[P]` só precisam não compartilhar o mesmo arquivo.

## Onde os testes moram

`#[cfg(test)] mod tests` no **fim do mesmo arquivo**, nunca num diretório `tests/` separado. Uma função que precisa de teste e não é testável sem `AppHandle` é sinal de que falta extrair a decisão como função pura — foi assim que nasceram `should_record_turn` e `recall_blocks` no M6, e `classify_output` antes deles.

## Todo

- [x] ~~Introduzir `cargo test` para os módulos de lógica pura~~ — feito e mantido desde então; a linha de base atual é 169
- [ ] Avaliar Vitest/React Testing Library quando a superfície de componentes estabilizar (é o C-04 do CONCERNS.md, e o toggle de memória do M6 é mais uma superfície sem cobertura)
