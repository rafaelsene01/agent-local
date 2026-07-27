# Quick Task 001 — Resumo

**Status:** Done · 2026-07-27

## O que foi feito

`shouldPrune`, em `scripts/vendor-runtime.mjs`, passou a reconhecer as duas formas de um par de
ferramenta do llama.cpp — o lançador (`llama-cli`, `llama-cli.exe`) e a biblioteca que o implementa
(`llama-cli-impl.dll`, `libllama-cli-impl.so`) — e a poda ficou restrita a essas duas formas. Toda
outra biblioteca compartilhada é mantida.

Antes, só o sufixo `.exe` era removido antes do casamento com `llama-`, o que fazia a regra apagar
duas bibliotecas necessárias no Windows:

| Arquivo | Tamanho | O que é |
| --- | --- | --- |
| `llama-server-impl.dll` | 9.898.496 B | o servidor de fato |
| `llama-common.dll` | 7.890.944 B | dependência dele |

O que sobrava era o `llama-server.exe` de **9.216 bytes**, um lançador que morre ao carregar.

## Verificação executada

| Prova | Antes | Depois |
| --- | --- | --- |
| `llama-server.exe --list-devices` (Vulkan) | exit `-1073741511` (0xC0000139), sem saída | **exit 0**, `Vulkan0: NVIDIA GeForce RTX 3060 (12329 MiB, 11548 MiB free)` |
| `llama-server.exe --list-devices` (CPU) | exit `-1073741511` | **exit 0**, lista vazia (esperado) |
| `npm run test:scripts` | 43 | **44** |
| Árvore vendorizada | 120,5 MB | **156,1 MB** (+17,8 MB por backend, as duas libs) |
| `-impl` órfãos na árvore | — | nenhum |

A causa foi isolada parseando as tabelas de import/export do PE — a máquina não tem `dumpbin` nem
`objdump`, e o único import não resolvido do executável era `llama-server-impl.dll`. Confirmado
contra o zip original baixado do GitHub.

O app foi então aberto e o runtime preparou com sucesso: **b10146, GPU (Vulkan), porta 58179**, com
o modelo carregado e conversa funcionando.

## Alcance

**Só Windows.** No Linux as bibliotecas se chamam `libllama-server-impl.so` e
`libllama-common.so.0.0.10146`, que não começam com `llama-` e escaparam da regra antiga por
acidente — nenhum CI de Linux acusaria isto.

## Consequência

Os instaladores medidos na AD-045 (NSIS 47,6 MiB, MSI 83,8 MiB, zip 92,0 MiB) foram gerados com a
árvore quebrada e **não executam o modelo**. Precisam ser refeitos.

## Commit

pendente — `fix(vendor): keep the llama-server implementation library`
