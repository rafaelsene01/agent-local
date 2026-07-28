# Quick Task 005: O AppImage parou de empacotar quando o runtime passou a viajar dentro dele

**Date:** 2026-07-27
**Status:** Corrigido no papel — **não verificado**, e só um `workflow_dispatch` verifica

## Description

O `release.yml` v0.3.0 morreu em `failed to bundle project: 'failed to run linuxdeploy'` durante o
AppImage. O `.deb` da **mesma execução** foi empacotado sem erro.

## O diagnóstico, e o que é evidência e o que é hipótese

**O que está estabelecido, contra artefato publicado e não por dedução:**

| Release | `bundle.resources`? | AppImage |
| --- | --- | --- |
| `v0.2.0` | não (a tag é anterior ao M9) | ✅ `LocalMind_0.2.0_amd64.AppImage` publicado, com `.sig` |
| `v0.3.0` | sim — **256,2 MB** vendorizados | ❌ falhou no `linuxdeploy` |

Conferido com `gh release view v0.2.0` e `git show v0.2.0:src-tauri/tauri.conf.json`, que não tem
`bundle.resources`. **O AppImage quebrou na primeira release que carregou a árvore vendorizada.**

**A causa provável, que é hipótese e está marcada como tal:** o `linuxdeploy` roda `strip` em todo
ELF que empacota e aborta o AppImage inteiro quando o `strip` recusa um deles. Desde o M9 o AppDir
carrega dois builds do `llama-server`, o ONNX Runtime e o pdfium — binários de terceiros que ninguém
aqui compilou. É o modo de falha documentado para esta mensagem exata, e a resposta documentada é
`NO_STRIP=true` ([tauri#15106](https://github.com/tauri-apps/tauri/issues/15106),
[tauri#14796](https://github.com/tauri-apps/tauri/issues/14796)).

**Por que não dá para ser mais preciso agora:** o log da v0.3.0 tem `failed to run linuxdeploy` e
**nenhuma linha do próprio linuxdeploy**. O diagnóstico teve que sair dos artefatos publicados
porque a mensagem do erro não existe.

## Por que não foi resolvido pelo `tauri.conf.json`

O `config.schema.json` do CLI **2.11.4** (o que está no `node_modules`) mostra que `AppImageConfig`
só aceita `bundleMediaFramework` e `files`. **Não há passagem de flags para o `linuxdeploy`.** O
único ponto de controle é variável de ambiente no step, que o processo filho herda.

## Files Changed

- `.github/workflows/release.yml` — `NO_STRIP: "true"` no `env` do step *Build + bundle*, e
  `--verbose` nos `args`

**Por que `NO_STRIP` não custa nada aqui:** o binário do próprio app já sai stripado pelo perfil de
release (REL-27, medido em −26,7%), então a variável só impede o `linuxdeploy` de re-stripar os
arquivos vendorizados — que é exatamente o que a AD-046 ensinou a não fazer, quando mexer neles
deixou o runtime sem executar.

**Por que `--verbose` entra junto:** para a próxima falha chegar com o motivo anexado. Diagnosticar
esta pelos artefatos publicados funcionou uma vez e não é método.

## Verification

- [x] O YAML parseia e os valores chegam onde deviam: `NO_STRIP='true'` no `env`, `args` terminando
      em `--verbose`
- [x] `-v, --verbose` existe no `tauri build --help` do CLI instalado — conferido, não presumido
- [x] `origin/master` é **idêntico em conteúdo** à base local (o `chore(release): v0.3.0` foi
      revertido), então a mudança aplica sem conflito
- [ ] ⛔ **O AppImage não foi empacotado.** Isto é Linux, o desenvolvimento é Windows, e o
      `linuxdeploy` nem roda aqui. **"YAML validado" não é verificação** — é literalmente a L-005
      deste repositório. Só um `workflow_dispatch` responde.

## Se não resolver

O `--verbose` traz a mensagem do `linuxdeploy`, e aí o diagnóstico deixa de ser por correlação. A
suspeita seguinte, se o `strip` não for a causa: os dois builds do llama trazem bibliotecas de
**mesmo nome** (`libggml-base.so`, `libllama.so`, `libllama-common.so`) com conteúdos diferentes, e
o `linuxdeploy` achata tudo num `usr/lib/` só. Isso é palpite e está aqui como próximo passo, não
como causa.

## Commit

Não commitado — o padrão do `AGENTS.md` é deixar no working tree.
