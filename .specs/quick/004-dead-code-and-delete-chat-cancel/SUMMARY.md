# Summary — Quick Task 004

**Date:** 2026-07-27

## O que foi feito

**C-11 — código morto.** Quatro warnings viraram zero, e três deles eram código morto de verdade,
não ruído a silenciar:

| Símbolo | Desfecho | Por quê |
| --- | --- | --- |
| `Quant::Q5/Q8/F16` | mantido com `#[allow(dead_code)]` e o motivo escrito | A tabela descreve o **esquema de quantização**, não o catálogo atual. Apagar as variantes deixaria `estimate_ram_gb` especializada em Q4 continuando a se chamar como se fosse geral, e uma entrada nova com outro quant teria que descobrir isso por erro de compilação em vez de encontrar o número aqui. |
| `HEALTH_CHECK_TIMEOUT` | removido | Sobra da tela de Conexões, que saiu com o M9 (AD-042). |
| `LlamaServerClient::health_check` | removido | O único chamador era um teste — ou seja, o método existia para o teste ter o que chamar. O teste continua, exercitando `model_limits`. |
| `PullStatus::Verifying` | removido do Rust **e** do `types.ts` | Era a fase de checksum do `pull` do Ollama. Um GGUF baixado por GET não tem essa fase. |

Mais um `let mut` desnecessário num teste de `db.rs`, que só aparecia sob `--tests`.

**C-14 — `delete_chat` não cancelava a geração.** Passou a chamar
`app.state::<CancellationRegistry>().cancel(&id)` como primeira linha, antes da transação — a mesma
via do `cancel_generation`. Sinalizar antes de apagar também estreita a janela que
`chat::memory::record_turn` cobre com a checagem de existência.

## Verificação

| Gate | Antes | Depois |
| --- | --- | --- |
| `cargo check --lib` | 4 warnings | **0** |
| `cargo check --lib --tests` | 5 warnings | **0** |
| `cargo test --lib` | 174 passando | **174 passando** — nenhum teste perdido |
| `npm run build` | limpo | limpo |
| `grep -rn "verifying" src/` | 1 uso vivo | nenhum |

## O que **não** foi verificado

**O C-14 não tem prova de execução.** `delete_chat` é um comando Tauri que só orquestra I/O, e a
matriz do `TESTING.md` põe isso explicitamente na coluna "nenhum teste" — não há runner de
integração e o comando precisa de um `AppHandle`. A prova é de UAT: apagar um chat no meio de uma
geração e observar o sidecar parar. **Isso não foi feito**, e está registrado como todo no
`STATE.md`.

O zero-warning destrava metade do C-09 — era o pré-requisito para ligar `clippy -D warnings` sem
que virasse refatoração disfarçada. **O `clippy` em si não foi rodado**, então não afirmo que ele
passe.

## Commit

Não commitado. O padrão do `AGENTS.md` é deixar as mudanças no working tree.
