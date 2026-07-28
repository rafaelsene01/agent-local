# Quick Task 004: Dívidas do CONCERNS — C-11 (código morto) e C-14 (delete_chat não cancela)

**Date:** 2026-07-27
**Status:** Done

## Description

Zerar os warnings de código morto do `cargo check` (C-11) e fazer `delete_chat` cancelar a geração
em curso antes de apagar o chat (C-14). Duas dívidas registradas no `CONCERNS.md` que nenhuma
feature planejada cobria.

## Files Changed

- `src-tauri/src/models/memory_estimate.rs` — `#[allow(dead_code)]` no enum `Quant`, com o motivo
  escrito: a tabela descreve o esquema de quantização, não o catálogo atual
- `src-tauri/src/providers/mod.rs` — `HEALTH_CHECK_TIMEOUT` e `PullStatus::Verifying` removidos,
  cada um com um comentário dizendo de onde vieram e por que saíram
- `src-tauri/src/providers/llama_server.rs` — `health_check` removido; o teste que o chamava
  continua, exercitando `model_limits`
- `src/types.ts` — `"verifying"` fora do `PullStatus`, espelhando o Rust à mão (C-03)
- `src-tauri/src/db.rs` — um `let mut` desnecessário num teste
- `src-tauri/src/commands.rs` — `delete_chat` cancela pelo `CancellationRegistry` antes da transação

**Desvio da regra de 3 arquivos, declarado:** são 6. Quatro deles são a mesma remoção atravessando a
fronteira Rust↔TS, que o C-03 obriga a fazer à mão. Separar em duas tasks deixaria o repositório
num estado em que o TS descreve um valor que o backend não envia mais.

## Verification

- [x] `cargo check --lib` e `cargo check --lib --tests`: **zero warnings** (eram 4 e 5)
- [x] `cargo test --lib`: **174 passando**, o mesmo de antes — nenhum teste perdido
- [x] `npm run build` limpo
- [x] `grep -rn "verifying" src/` não encontra mais nenhuma referência viva
- [ ] **C-14 não tem prova de execução.** `delete_chat` é comando Tauri sem runner de integração
      (`TESTING.md`); a prova é apagar um chat gerando e ver o sidecar parar — **não feito**

## Commit

não commitado — o padrão do `AGENTS.md` é deixar no working tree
