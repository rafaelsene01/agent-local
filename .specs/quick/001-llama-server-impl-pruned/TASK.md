# Quick Task 001: A poda do vendoring apagava o servidor

**Date:** 2026-07-27
**Status:** Done
**Feature afetada:** `self-contained-runtime` (SELF-09, SELF-10, SELF-11)

## Description

`shouldPrune` derrubava `llama-server-impl.dll` e `llama-common.dll` do pacote do
llama.cpp, deixando no bundle apenas o `llama-server.exe` — que desde a b10146 é
um **lançador de 9 KB**, não o servidor. O binário empacotado morria ao carregar,
com `0xC0000139` (STATUS_ENTRYPOINT_NOT_FOUND) e **nenhuma mensagem de erro**.

O comentário da função já afirmava a regra certa — *"every shared library is
kept"* — mas o código só removia o sufixo `.exe` antes de casar com `llama-`, de
modo que qualquer `.dll` com esse prefixo era tratada como ferramenta extra.

**Alcance:** só Windows. No Linux as mesmas bibliotecas se chamam
`libllama-server-impl.so` e `libllama-common.so.0.0.10146`, que não começam com
`llama-` e por isso escaparam por acidente.

## Files Changed

- `scripts/vendor-runtime.mjs` — `shouldPrune` passa a reconhecer as duas formas
  de um par de ferramenta (lançador + `-impl`), com prefixo `lib` opcional, e a
  manter qualquer outra biblioteca compartilhada
- `scripts/vendor-runtime.test.mjs` — teste novo cobrindo `llama-server-impl.dll`
  e `libllama-server-impl.so` (mantidos) contra os `-impl` das outras ferramentas
  (podados). A lista de casos anterior passava ao largo do defeito: toda
  biblioteca que ela citava evitava a combinação que quebra, o prefixo `llama-`
  num `.dll`

## Verification

- [x] `npm run test:scripts` — **44 passando** (eram 43)
- [x] `npm run vendor -- --force` refaz a árvore: **156,1 MB** (era 120,5 MB)
- [x] `llama-server.exe --list-devices` do bundle Vulkan: **exit 0**, responde
      `Vulkan0: NVIDIA GeForce RTX 3060 (12329 MiB, 11548 MiB free)` — antes era
      exit `-1073741511` sem saída nenhuma
- [x] O bundle CPU também sobe (exit 0, lista vazia, que é o esperado)
- [x] `.../llama/*/` não tem mais nenhum `-impl` órfão

## Consequência para os artefatos já medidos

Os instaladores da AD-045 (NSIS 47,6 MiB, MSI 83,8 MiB, zip 92,0 MiB) foram
gerados com a árvore quebrada e **não conseguem executar o modelo**. Precisam ser
regerados; os tamanhos daquele registro deixam de valer.

## Commit

pendente — `fix(vendor): keep the llama-server implementation library`
