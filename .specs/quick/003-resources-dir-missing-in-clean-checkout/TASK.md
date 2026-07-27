# Quick Task 003: o CI quebrou porque a pasta de recursos não existe num clone

**Date:** 2026-07-27
**Status:** Done
**Feature afetada:** `self-contained-runtime` (T13, `design.md` — pontos de integração)

## Description

O job `rust` do `ci.yml` falhou no build script do Tauri, antes de compilar
qualquer coisa:

```
error: failed to run custom build command for `tauri-app v0.2.0`
  resource path `resources` doesn't exist
```

`tauri.conf.json` declara `bundle.resources: ["resources/"]`, e
`src-tauri/resources/` estava **inteira** no `.gitignore` — a árvore de 120 MB
vem do `npm run vendor`, não do repositório. O vendoring roda pelo
`beforeBuildCommand`/`beforeDevCommand`, ou seja, **só quando quem constrói é o
Tauri CLI**. O CI chama `cargo test` direto: nunca passa por ali, a pasta nunca
aparece, e o `tauri-build` aborta.

Não é um defeito de CI. `cd src-tauri && cargo test --lib` — o gate padrão do
`AGENTS.md` — falha igual num clone novo, para qualquer pessoa que ainda não
tenha rodado o vendoring. O CI só foi a primeira máquina a fazer isso.

**A linha de spec que isso falsificou:** `self-contained-runtime/design.md`
dizia, sobre `release.yml` / `ci.yml`, *"o vendoring entra via
`beforeBuildCommand`, então nenhum passo novo de workflow é obrigatório"*. Vale
para o `release.yml`, que constrói pelo `tauri-action`. Não vale para o
`ci.yml`.

## Files Changed

- `.gitignore` — passa a ignorar o **conteúdo** (`src-tauri/resources/*`) com
  exceção para o `.gitkeep`. Git não consegue re-incluir um arquivo cujo
  diretório-pai está excluído, então ignorar a pasta e abrir exceção para o
  arquivo dentro dela não funcionaria
- `src-tauri/resources/.gitkeep` — novo, versionado; o texto dentro dele explica
  por que existe, para ninguém apagar achando que é resíduo
- `.specs/features/self-contained-runtime/design.md` — a linha `release.yml /
  ci.yml` virou duas, com o motivo de a segunda não seguir a primeira
- `.specs/features/self-contained-runtime/tasks.md` — nota na T13
- `.specs/project/STATE.md` — AD-049

## Alternativas recusadas, com o motivo

| Opção | Por que não |
| --- | --- |
| `mkdir -p src-tauri/resources` no `ci.yml` | Conserta o CI e deixa o clone novo quebrado do mesmo jeito. O defeito não é do workflow |
| `npm run vendor` antes do `cargo test` no CI | ~120 MB de download num job que é offline de propósito — o próprio `ci.yml` documenta que não roda os `#[ignore]` para não depender de rede |

## Verification

Medido nesta máquina, reproduzindo o estado do checkout limpo: a árvore
vendorizada foi movida de lado e restaurada no mesmo comando.

- [x] **(A) Sem a pasta, o erro do CI reproduz local**, verbatim:
      `resource path 'resources' doesn't exist` em `cargo check --lib`
- [x] **(B) Com a pasta contendo apenas o `.gitkeep`, compila:** `Finished
      'dev' profile ... in 10.93s`. Não é dedução — é o mesmo build script que
      falhou em (A), rodando de novo
- [x] O mecanismo confere com o fonte: `tauri-utils-2.9.3/src/resources.rs`
      erra com `ResourcePathNotFound` quando o padrão sem glob não existe, e
      **pula em silêncio** quando o diretório existe e está vazio
      (`// If the directory is empty, skip and continue to the next pattern`)
- [x] `git status --untracked-files=all -- src-tauri/resources/` lista **só**
      `.gitkeep`; `git check-ignore -v` confirma que `.vendor-stamp.json` e
      `llama/cpu` continuam ignorados pelo `src-tauri/resources/*`
- [x] A árvore vendorizada voltou intacta: 150 MB, com `.vendor-stamp.json`,
      `llama/`, `onnxruntime/` e `pdfium/`
- [x] Gates: `cargo test --lib` **174 passando / 0 falhas / 13 ignorados**,
      `npm run build` limpo, `npm run test:scripts` **49 passando**

**O que NÃO foi verificado:** o CI em si. A prova de que o job `rust` volta a
passar é o próximo push — o que foi medido aqui é o build script, que é
exatamente onde ele morreu. O `.gitkeep` também passa a viajar dentro do
instalador (0,5 KB em `$RESOURCE/.gitkeep`); nenhum bundle foi regerado para
conferir isso, e nada no app lê aquela pasta por listagem — `runtime::bundled`
procura arquivos por nome.

## Commit

pendente — `fix(build): keep the resources directory in a clean checkout`
