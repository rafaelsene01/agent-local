# Release & Distribution (M8) — Tasks

**Design**: `.specs/features/release-distribution/design.md`
**Spec**: `.specs/features/release-distribution/spec.md`
**Status**: In Progress — **23/24 implementadas**; T2 concluída pelo mantenedor em 2026-07-26, **só a T24 continua aberta**

> **T2 exigiu ação humana** (gerar o par de chaves e cadastrar os secrets no GitHub) e foi feita. A T24 depende de uma release publicada de verdade.

---

## Execution Log (2026-07-26)

| Task | Status | Evidência |
| --- | --- | --- |
| T1 | ✅ | `tauri.conf.json` com `mainBinaryName`, `createUpdaterArtifacts`, NSIS `currentUser`, targets explícitos. **Não verificado:** que o binário passe a se chamar `LocalMind.exe` (exige um `tauri build`) |
| T2 | ✅ | Par gerado pelo mantenedor; `gh secret list` confirma `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (2026-07-26 19:36 UTC); `plugins.updater.pubkey` preenchida e validada (2 linhas, `minisign public key`, 42 bytes). **Incidente:** a chave **privada** foi colada primeiro por engano — pega antes de qualquer commit, e agora há um teste (`the_configured_public_key_is_a_public_key_and_parses`) que falha o `cargo test` se isso se repetir |
| T3 | ✅ | 10 testes verdes. Conferido na mão: `patch` sobre `0.1.0` → `0.1.1`; `minor` sobre `v1.9.3` → `1.10.0` (prova que o bump não é lexicográfico) |
| T4 | ✅ | `git cliff --unreleased --strip all` rodado contra o histórico real: 40+ commits agrupados em Novidades/Correções/Documentação/Refatoração/Testes/Manutenção, exit 0 |
| T5 | ✅ | YAML validado com `yaml.safe_load`. **Executado no GitHub em 2026-07-26 e falhou na primeira vez** — ver "Primeira execução real", abaixo. Corrigido e re-verificado localmente nas duas formas (shell expandindo o glob e Node expandindo) |
| T6 | ✅ | YAML válido; guardas de branch e de tag existente antes de qualquer escrita |
| T7 | ✅ | 4 testes verdes; erro claro quando o binário não existe (exit 1). **Não verificado:** o zip em si (exige `tauri build`) |
| T8 | ✅ | YAML válido. **Não verificado:** execução real |
| T9 | ✅ | 11 testes verdes, incluindo o caso em que `x64-portable.zip` casa também com `.zip.sig` — resolvido com busca por nome exato (`pickAssetUrlByName`) |
| T10 | ✅ | YAML válido; `--draft=false` é o último passo |
| T11 | ✅ | `docs/RELEASING.md` + link no README |
| T12 | ✅ | 7 testes verdes, incluindo `0.1.10 > 0.1.9` |
| T13 | ✅ | 4 testes verdes; config anterior ao M8 continua desserializando |
| T14 | ✅ | 7 testes verdes contra fixture **real** gerada pelo `tauri signer`; conteúdo adulterado é recusado |
| T15 | ✅ | 8 testes verdes; chave de plataforma desconhecida não quebra o parse |
| T16 | ✅ | 10 testes verdes (extração, traversal, rollback, cleanup) |
| T17 | ✅ | `tauri-plugin-updater = "2.10"` + registro no builder. **Desvio:** os pacotes npm do plugin **não** foram adicionados — o frontend fala só com os nossos comandos, o que evita duas superfícies de API |
| T18 | ✅ | 5 comandos registrados; `cleanup_after_update()` no `setup` |
| T19–T22 | ✅ | `npm run build` limpo (tsc + Vite, 1859 módulos) |
| T23 | ⚠️ Parcial | `[profile.release]` com `strip`, `codegen-units = 1`, `lto = "thin"`. **Desvios do plano:** `lto = true` (fat) virou `"thin"` — fat LTO sobre arrow/lancedb/onnx joga o build do CI para dezenas de minutos; e `panic = "abort"` ficou **de fora**, porque remove unwinding de que os stacks SQLite/Arrow podem depender. **Medição pendente** (exige build de release completo) |
| T24 | ⛔ **Bloqueada** | Exige publicar release de verdade (depende de T2) e instalar/atualizar em conta sem administrador |

**Totais medidos (atualizados em 2026-07-26 após a AD-036):** `cargo test` → **123 passando, 0 falhas, 4 ignorados** (112 antes, 74 antes do M8). `npm run test:scripts` → **27 passando** (eram 25). `npm run build` limpo.

### Primeira execução real do `ci.yml` (2026-07-26)

O job de scripts falhou:

```
> node --test "scripts/**/*.test.mjs"
Could not find '/home/runner/work/agent-local/agent-local/scripts/**/*.test.mjs'
Error: Process completed with exit code 1.
```

**Causa:** o padrão vinha entre aspas, o que impede a shell de expandi-lo, então quem tinha que expandir era o próprio Node — e o `--test` só ganhou suporte a glob a partir do Node 22. O CI rodava **Node 20**. Na máquina de desenvolvimento (Node 24) o mesmo comando passava, porque lá o Node expandia. Um caso clássico de verde local com vermelho no CI, e a prova de que "YAML válido" nunca foi evidência de que o workflow funciona.

**Correção, em duas camadas:**
1. `npm run test:scripts` virou `node --test scripts/*.test.mjs` — **sem aspas**. Na shell do CI quem expande é a shell (funciona em qualquer Node); no Windows, onde o npm chama o `cmd.exe`, que não expande, quem expande é o Node. Verificado nas **duas** formas nesta máquina, 27 testes em cada.
2. `node-version` passou de 20 para **24** nos quatro pontos dos dois workflows. O Node 20 saiu do suporte em abril de 2026 — o CI estava rodando numa versão morta. `engines.node: ">=22"` entrou no `package.json` para deixar o requisito escrito.

---

## Execution Plan

```
Fase 1 — Fundação (paralelo)
  T1 [P]  tauri.conf.json           T3 [P]  bump-version.mjs
  T4 [P]  cliff.toml + CHANGELOG    T5 [P]  ci.yml
  T9 [P]  patch-latest-json.mjs     T12 [P] update/mod.rs

Fase 2 — Derivadas (paralelo)
  T1 ──┬→ T2  (chaves + pubkey + capability)   [BLOQUEIA: ação humana]
       ├→ T7  (make-portable.mjs)
       └→ T23 (perfil release strip+LTO)
  T3,T4 → T6  (release.yml: job prepare)
  T12 ─┬→ T13 (config.rs portátil)
       ├→ T14 (update/signature.rs)
       └→ T15 (update/manifest.rs)

Fase 3 — Integração
  T2,T6,T7 → T8  (release.yml: job build + portátil assinado)
  T14,T15  → T16 (update/portable.rs)
  T2       → T17 (plugin updater no Cargo/npm/lib.rs)

Fase 4
  T8,T9      → T10 (release.yml: job finalize)
  T13,T16,T17 → T18 (update_commands.rs)

Fase 5
  T10 → T11 (docs/RELEASING.md)
  T18 → T19 (updateApi.ts)

Fase 6 — UI (sequencial: compartilham os JSON de i18n)
  T19 → T20 → T21 → T22

Fase 7 — Verificação real
  T10,T11,T22,T23 → T24
```

---

## Task Breakdown

### T1: Ajustar `tauri.conf.json` para bundling e update [P]

**What**: Declarar `mainBinaryName`, ligar os artefatos de update, fixar o NSIS em `currentUser` e tornar os targets de bundle explícitos.
**Where**: `src-tauri/tauri.conf.json`
**Depends on**: None
**Reuses**: config existente (não mexer em `app`, `build` nem `identifier`)
**Requirement**: REL-09, REL-10, REL-12

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `"mainBinaryName": "LocalMind"` presente
- [ ] `bundle.createUpdaterArtifacts: true`
- [ ] `bundle.windows.nsis.installMode: "currentUser"` explícito
- [ ] `bundle.targets` deixa de ser `"all"` e lista os formatos alvo
- [ ] `npm run tauri build --  --bundles nsis` (ou `cargo check`) não reclama de config inválida

**Tests**: none (arquivo de config)
**Gate**: build
**Verify**: `npx tauri build --help` roda sem erro de schema; após um build local, o executável se chama `LocalMind.exe` e não mais `tauri-app.exe`
**Commit**: `build(tauri): set mainBinaryName, updater artifacts and currentUser NSIS`

---

### T2: Gerar par de chaves de assinatura e ligá-lo ao projeto ⚠️ AÇÃO HUMANA

**What**: Criar o par minisign, cadastrar a chave privada e a senha como secrets do repositório, e commitar a chave pública + endpoint + capability.
**Where**: `src-tauri/tauri.conf.json` (`plugins.updater`), `src-tauri/capabilities/default.json`
**Depends on**: T1
**Reuses**: padrão de capability já existente (`core:default`, `opener:default`, `dialog:default`)
**Requirement**: REL-10, REL-21

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `npx tauri signer generate -w ~/.tauri/localmind.key` executado pelo mantenedor
- [ ] Secrets `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` existem no repositório (`gh secret list` confirma)
- [ ] `plugins.updater.pubkey` preenchido com a chave **pública**
- [ ] `plugins.updater.endpoints` aponta para `.../releases/latest/download/latest.json`
- [ ] `plugins.updater.windows.installMode: "passive"`
- [ ] `"updater:default"` adicionado às permissões da capability
- [ ] Nenhuma chave privada em nenhum arquivo versionado (`git grep -i "untrusted comment"` volta vazio)

**Tests**: none
**Gate**: build
**Verify**: `gh secret list` mostra os dois secrets; `git diff` mostra só a chave pública
**Commit**: `build(updater): add signing public key, endpoint and updater capability`

---

### T3: Script de bump de versão com testes [P]

**What**: Script Node sem dependências que aplica `major|minor|patch` e grava a versão nos arquivos que a duplicam.
**Where**: `scripts/bump-version.mjs`, `scripts/bump-version.test.mjs`
**Depends on**: None
**Reuses**: nenhum (primeiro script do projeto)
**Requirement**: REL-03, REL-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `node scripts/bump-version.mjs <versão>` grava em `package.json`, `package-lock.json` (`version` e `packages[""].version`) e `src-tauri/Cargo.toml` (`[package] version`, sem tocar em versões de dependências). **Revisão de 2026-07-26:** `src-tauri/tauri.conf.json` saiu da lista — o campo `version` dele virou `"../package.json"`, que o Tauri resolve no build. Uma cópia a menos para divergir, e há teste que falha se alguém colar uma versão literal de volta
- [ ] Função pura de bump exportada e testada: `1.2.3`+patch→`1.2.4`, +minor→`1.3.0`, +major→`2.0.0`, e `0.1.0`+minor→`0.2.0`
- [ ] Versão inválida ou bump desconhecido → erro com exit code ≠ 0
- [x] Gate check passa: `npm run test:scripts`
- [ ] Test count: ≥6 testes passam

**Tests**: unit
**Gate**: quick
**Verify**: rodar contra uma cópia do repo e conferir os 4 arquivos; `cargo metadata --no-deps` reflete a versão nova no `Cargo.lock`
**Commit**: `build(release): add version bump script`

---

### T4: Configurar geração de CHANGELOG [P]

**What**: `cliff.toml` agrupando Conventional Commits por tipo, e `CHANGELOG.md` inicial.
**Where**: `cliff.toml`, `CHANGELOG.md`
**Depends on**: None
**Reuses**: histórico de commits existente (`feat:`, `fix:`, `docs:`, `test:` já em uso)
**Requirement**: REL-05

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `cliff.toml` agrupa pelo menos `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`/`ci`, e joga o resto em "Outros"
- [ ] Commits `chore(release):` são excluídos do changelog
- [ ] `git cliff --unreleased` produz saída não vazia contra o histórico atual do repo
- [ ] `CHANGELOG.md` existe com cabeçalho e a seção Unreleased

**Tests**: none (configuração declarativa, validada pela saída real)
**Gate**: build
**Verify**: `git cliff --unreleased --strip all` lista os commits recentes agrupados corretamente
**Commit**: `build(release): add git-cliff config and CHANGELOG`

---

### T5: Workflow de validação (`ci.yml`) [P]

**What**: Workflow que roda em push em `master` e em PR, com os jobs `frontend`, `rust` e `commits`.
**Where**: `.github/workflows/ci.yml`
**Depends on**: None
**Reuses**: comandos de gate de `.specs/codebase/TESTING.md`
**Requirement**: REL-25, REL-26

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Job `frontend`: `npm ci` + `npm run build`, com cache de npm
- [ ] Job `rust` em `ubuntu-22.04`: deps de sistema do Tauri + `protobuf-compiler` + `Swatinem/rust-cache` + `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] `cargo test` roda **sem** `--include-ignored` (os 4 testes ignorados baixam modelo)
- [ ] Job `commits` roda só em `pull_request` e valida Conventional Commits
- [ ] O workflow **não** tem nenhum passo que crie tag, release ou publique artefato

**Tests**: none (o próprio workflow é o teste)
**Gate**: full — só está pronto quando roda verde no GitHub
**Verify**: fazer push numa branch, abrir PR, ver os três checks verdes; introduzir um erro de tipo em TS e ver o check `frontend` ficar vermelho
**Commit**: `ci: add build and test validation workflow`

---

### T6: `release.yml` — job `prepare`

**What**: Workflow de release com disparo exclusivamente manual e o job que calcula versão, gera changelog, commita e tagueia.
**Where**: `.github/workflows/release.yml`
**Depends on**: T3, T4
**Reuses**: `scripts/bump-version.mjs` (T3), `cliff.toml` (T4)
**Requirement**: REL-01, REL-02, REL-03, REL-05, REL-06, REL-07

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Único gatilho é `workflow_dispatch` com input `bump` do tipo `choice` e opções `patch|minor|major` — **nenhum** `push`/`tag`/`schedule` no arquivo
- [ ] `checkout` com `fetch-depth: 0`
- [ ] Falha se `github.ref_name != 'master'`, **antes** de qualquer escrita
- [ ] Versão base = última tag `v*`; se não houver nenhuma, a versão de `package.json`
- [ ] Falha se a tag calculada já existir, **antes** de qualquer escrita
- [ ] Chama `bump-version.mjs`, gera `CHANGELOG.md`, commita `chore(release): vX.Y.Z`, cria a tag e faz push dos dois
- [ ] Expõe os outputs `version`, `tag` e `notes`

**Tests**: none
**Gate**: build (YAML válido) — verificação real fica em T24
**Verify**: `gh workflow view release.yml` lista o input `bump` com as 3 opções e nenhum outro gatilho
**Commit**: `ci(release): add manual release workflow with version and changelog job`

---

### T7: Script que monta o bundle portátil [P]

**What**: Script que monta a árvore do portátil a partir do binário compilado e gera o `.zip`.
**Where**: `scripts/make-portable.mjs`, `scripts/make-portable.test.mjs`
**Depends on**: T1
**Reuses**: `mainBinaryName` definido em T1
**Requirement**: REL-12

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Produz `LocalMind_<versão>_x64-portable.zip` com a raiz `LocalMind/` contendo `LocalMind.exe`, `.portable` (vazio) e `README.txt`
- [ ] Falha com mensagem clara se o binário de origem não existir
- [ ] Imprime o caminho absoluto do zip em stdout (o workflow consome isso)
- [ ] Função de montagem do nome do arquivo é pura e testada
- [x] Gate check passa: `npm run test:scripts`
- [ ] Test count: ≥8 testes passam no total do diretório (≥6 de T3 + ≥2 daqui)

**Tests**: unit
**Gate**: quick
**Verify**: rodar após um `tauri build` local e descompactar o zip num diretório limpo — a árvore bate com o design
**Commit**: `build(release): add portable bundle packaging script`

---

### T8: `release.yml` — job `build` (matriz + portátil assinado)

**What**: Job matricial que compila e empacota nos dois SOs, e anexa o zip portátil assinado no Windows.
**Where**: `.github/workflows/release.yml` (modificar)
**Depends on**: T2, T6, T7
**Reuses**: `tauri-apps/tauri-action`, `scripts/make-portable.mjs`
**Requirement**: REL-08, REL-09, REL-10, REL-11, REL-12

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Matriz `windows-latest` (`--bundles msi,nsis`) e `ubuntu-22.04` (`--bundles deb,appimage`)
- [ ] `checkout` usa `ref: needs.prepare.outputs.tag`
- [ ] `protoc` instalado nos dois runners; deps de sistema do Tauri no Linux
- [ ] `tauri-action` com `releaseDraft: true` e `releaseBody` vindo de `needs.prepare.outputs.notes`
- [ ] Env `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` presentes
- [ ] Passo só-Windows: `make-portable.mjs` → `npx tauri signer sign` → `gh release upload` do `.zip` e do `.sig`

**Tests**: none
**Gate**: build — verificação real em T24
**Verify**: em T24, a release em draft mostra `.msi`, `-setup.exe`, `.deb`, `.AppImage`, `.zip`, os `.sig` e o `latest.json`
**Commit**: `ci(release): build installers and signed portable bundle`

---

### T9: Script que injeta a entrada portátil no `latest.json` [P]

**What**: Script que lê o `latest.json` gerado pelo Tauri e acrescenta a chave `windows-x86_64-portable`.
**Where**: `scripts/patch-latest-json.mjs`, `scripts/patch-latest-json.test.mjs`
**Depends on**: None
**Reuses**: formato de `latest.json` documentado no design
**Requirement**: REL-10

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Acrescenta `platforms["windows-x86_64-portable"] = { url, signature }` preservando **todas** as chaves existentes
- [ ] A URL do zip é **lida da lista de assets da release**, não montada por suposição (Open Question #4 do design)
- [ ] Erro claro se o `latest.json` de entrada não tiver `platforms`
- [x] Gate check passa: `npm run test:scripts`
- [ ] Test count: ≥11 testes passam no total do diretório

**Tests**: unit
**Gate**: quick
**Verify**: rodar contra um `latest.json` de exemplo e conferir que as chaves originais continuam intactas
**Commit**: `build(release): add script to add portable entry to latest.json`

---

### T10: `release.yml` — job `finalize`

**What**: Job que corrige o `latest.json` publicado e tira a release do estado de rascunho.
**Where**: `.github/workflows/release.yml` (modificar)
**Depends on**: T8, T9
**Reuses**: `scripts/patch-latest-json.mjs`
**Requirement**: REL-10, REL-11

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Baixa o `latest.json` da release, roda o script, faz `gh release upload --clobber`
- [ ] `gh release edit <tag> --draft=false` é o **último** passo do workflow
- [ ] Se qualquer job da matriz falhar, o `finalize` não roda e a release permanece em draft

**Tests**: none
**Gate**: build — verificação real em T24
**Verify**: em T24, o `latest.json` publicado contém as 4 chaves de plataforma e a release está pública
**Commit**: `ci(release): patch updater manifest and publish the release`

---

### T11: Documentar o processo de release

**What**: Guia curto de como publicar, o que fazer quando o workflow falha no meio, e como rotacionar a chave.
**Where**: `docs/RELEASING.md`, `README.md` (link)
**Depends on**: T10
**Reuses**: decisões registradas no design
**Requirement**: REL-01

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Explica o disparo (Actions → Release → escolher o bump) e o que cada bump significa
- [ ] Explica a recuperação: se o workflow falhar **depois** do push da tag, apagar tag + release em draft e re-rodar
- [ ] Documenta os dois secrets e como regerar/rotacionar a chave
- [ ] Registra que sem code signing o SmartScreen avisa na primeira execução
- [ ] `README.md` linka o documento

**Tests**: none
**Gate**: build
**Verify**: um leitor que nunca viu o pipeline consegue publicar seguindo só o documento
**Commit**: `docs: add release process guide`

---

### T12: Módulo `update` — modo de instalação e comparação de versão [P]

**What**: Base do módulo: enum de modo, detecção pelo marcador, resolução da pasta do app e comparação semântica de versão.
**Where**: `src-tauri/src/update/mod.rs`
**Depends on**: None
**Reuses**: estilo de `runtime/mod.rs` (submódulos por responsabilidade, erros `Result<_, String>`)
**Requirement**: REL-13, REL-14, REL-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `enum InstallFlavor { Installed, Portable }` com `Serialize`
- [ ] `flavor()` retorna `Portable` só quando existe `.portable` ao lado do `current_exe()`; qualquer erro de I/O cai em `Installed`
- [ ] `app_dir() -> Option<PathBuf>`
- [ ] `is_newer(candidate, current) -> bool` comparando major/minor/patch numericamente
- [ ] `struct UpdateInfo` conforme o design
- [ ] Testes de `is_newer`: `0.1.10 > 0.1.9` (não comparação lexicográfica), igual→false, menor→false, versão malformada→false
- [ ] `mod update;` declarado em `lib.rs`
- [ ] Gate check passa: `cd src-tauri && cargo test`
- [ ] Test count: 74 existentes + ≥5 novos passam

**Tests**: unit
**Gate**: quick
**Verify**: `cargo test update::` verde
**Commit**: `feat(update): add install flavor detection and version comparison`

---

### T13: Caminhos portáteis e novos campos de config

**What**: Fazer `config.rs` gravar bootstrap e pasta-base ao lado do executável em modo portátil, e adicionar os campos de preferência de update.
**Where**: `src-tauri/src/config.rs` (modificar)
**Depends on**: T12
**Reuses**: `bootstrap_file_path`, `default_base_path`, `ensure_folder_structure` existentes
**Requirement**: REL-13, REL-14, REL-18, REL-24

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Em modo portátil, `bootstrap_file_path` → `<exe_dir>/data/config.json` e `default_base_path` → `<exe_dir>/data`
- [ ] Em modo instalado, ambos mantêm **exatamente** o comportamento atual (AD-012/AD-008) — sem regressão
- [ ] `AppConfig` ganha `auto_update_check: bool` e `skipped_version: Option<String>`, ambos `#[serde(default)]`
- [ ] `auto_update_check` tem default `true`
- [ ] Um `config.json` de versão anterior (sem os campos novos) continua desserializando
- [ ] Testes: default dos campos novos; desserialização de config antigo; resolução dos dois caminhos por modo (com o diretório injetado, sem depender do `current_exe` real)
- [ ] Gate check passa: `cd src-tauri && cargo test`
- [ ] Test count: 74 + T12 + ≥4 novos passam

**Tests**: unit
**Gate**: quick
**Verify**: `cargo test config::` verde; a verificação real de "não escreve em %APPDATA%" fica em T24
**Commit**: `feat(config): support portable data layout and update preferences`

---

### T14: Verificação de assinatura minisign [P]

**What**: Converter o formato de chave/assinatura do `tauri signer` para o que o `minisign-verify` espera, e verificar um arquivo.
**Where**: `src-tauri/src/update/signature.rs`, `src-tauri/Cargo.toml`
**Depends on**: T12
**Reuses**: nenhum — é código novo, isolado de propósito
**Requirement**: REL-21

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `minisign-verify = "0.2"` adicionado ao `Cargo.toml`
- [ ] `decode_pubkey(&str) -> Result<PublicKey, String>` faz base64-decode e extrai a linha da chave (descartando o `untrusted comment:`)
- [ ] `verify(bytes, signature_b64, pubkey) -> Result<(), String>` com o mesmo tratamento na assinatura
- [ ] Assinatura adulterada, chave errada e base64 inválido cada um retorna `Err` distinguível
- [ ] Fixture de teste: par de chaves real gerado com `npx tauri signer generate` + um arquivo pequeno assinado, ambos commitados em `src-tauri/tests/fixtures/` (a chave privada da fixture **não** é a de produção e pode ser pública)
- [ ] Gate check passa: `cd src-tauri && cargo test`
- [ ] Test count: acumulado + ≥5 novos passam

**Tests**: unit
**Gate**: quick
**Verify**: `cargo test update::signature` verde, incluindo o caso de assinatura adulterada
**Commit**: `feat(update): verify minisign signatures from tauri signer format`

---

### T15: Leitura do manifesto de update [P]

**What**: Buscar o `latest.json` e escolher a entrada de plataforma correta para o SO + modo de instalação.
**Where**: `src-tauri/src/update/manifest.rs`
**Depends on**: T12
**Reuses**: `providers::http_client()` (AD-028 — sem timeout total, `connect_timeout` de 5 s) e `SHORT_REQUEST_TIMEOUT`
**Requirement**: REL-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Structs `Manifest { version, notes, pub_date, platforms }` e `PlatformEntry { url, signature }`
- [ ] `platform_key(flavor) -> &str` devolve `windows-x86_64-portable` no modo portátil e a chave do instalador no modo instalado
- [ ] `select(manifest, flavor) -> Option<&PlatformEntry>` — chave ausente devolve `None`, **não** erro
- [ ] `fetch(url)` usa `providers::http_client()` com timeout curto por requisição
- [ ] Testes com JSON fixture: parse completo; chaves desconhecidas no `platforms` não quebram o parse; chave da plataforma ausente → `None`
- [ ] Gate check passa: `cd src-tauri && cargo test`
- [ ] Test count: acumulado + ≥4 novos passam

**Tests**: unit
**Gate**: quick
**Verify**: `cargo test update::manifest` verde
**Commit**: `feat(update): fetch and parse the updater manifest`

---

### T16: Atualização portátil — download, troca e rollback

**What**: O caminho completo do update portátil: baixar com progresso, verificar, extrair, trocar os arquivos com rollback, e limpar `.old` no boot.
**Where**: `src-tauri/src/update/portable.rs`
**Depends on**: T14, T15
**Reuses**: `runtime/download.rs` (padrão de download com evento de progresso) e `runtime/release.rs` (extração de arquivo)
**Requirement**: REL-17, REL-20, REL-21

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Verifica que a pasta do app é gravável **antes** de baixar; se não for, `Err` explicando (edge case do pendrive/Program Files)
- [ ] Baixa para `%TEMP%` emitindo `update-download-progress`
- [ ] Verifica a assinatura via T14 **antes** de extrair; falha → apaga o temp e retorna `Err` sem tocar em nada instalado
- [ ] Extrai para `<app_dir>/.update/` descartando o primeiro componente do caminho
- [ ] Troca: renomeia o executável para `.old`, move os arquivos novos; falha no meio → restaura o `.old` e retorna `Err`
- [ ] `cleanup_old_files(app_dir)` remove `*.old` e `.update/` residual, ignorando erros
- [ ] Testes das funções puras: `strip_first_component`, decisão de rollback, e `cleanup` num diretório temporário
- [ ] Gate check passa: `cd src-tauri && cargo test`
- [ ] Test count: acumulado + ≥5 novos passam

**Tests**: unit
**Gate**: quick — o caminho completo só é verificável em T24
**Verify**: `cargo test update::portable` verde
**Commit**: `feat(update): add portable in-place updater with rollback`

---

### T17: Instalar e registrar o `tauri-plugin-updater`

**What**: Adicionar o plugin oficial nas duas pontas e registrá-lo no builder.
**Where**: `src-tauri/Cargo.toml`, `package.json`, `src-tauri/src/lib.rs`
**Depends on**: T2
**Reuses**: padrão de registro dos plugins `opener` e `dialog` em `lib.rs`
**Requirement**: REL-19

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `tauri-plugin-updater = "2"` no `Cargo.toml` (resolvido para ≥2.10, que é o que suporta as chaves `{os}-{arch}-{installer}`)
- [ ] `@tauri-apps/plugin-updater` e `@tauri-apps/plugin-process` no `package.json`
- [ ] `.plugin(tauri_plugin_updater::Builder::new().build())` no builder
- [ ] Gate check passa: `cd src-tauri && cargo check` e `npm run build`

**Tests**: none (wiring de plugin)
**Gate**: build
**Verify**: o app sobe com `npm run tauri dev` sem erro de capability
**Commit**: `build(update): add official tauri updater plugin`

---

### T18: Comandos Tauri de update

**What**: Os cinco comandos que o frontend consome, o roteamento entre os dois caminhos e a limpeza de `.old` no boot.
**Where**: `src-tauri/src/update_commands.rs`, `src-tauri/src/lib.rs` (modificar)
**Depends on**: T13, T16, T17
**Reuses**: padrão de `embedded_commands.rs` (comandos + helper de estado) e o bloco `setup` de `lib.rs`
**Requirement**: REL-15, REL-17, REL-18, REL-19, REL-20, REL-22, REL-23, REL-24

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `check_for_update` → `Option<UpdateInfo>`; devolve `None` quando a versão não é maior **ou** quando bate com `skipped_version`
- [ ] `install_update` roteia: `Installed` → plugin oficial (`download_and_install` + restart); `Portable` → `update::portable`, spawn do executável novo + `app.exit(0)`
- [ ] `skip_update_version`, `get_update_settings`, `set_auto_update_check` implementados e persistindo via `config.rs`
- [ ] Os cinco comandos registrados no `invoke_handler`
- [ ] `update::portable::cleanup_old_files` chamado no `setup`, junto de `autostart_sidecar`/`requeue_unfinished_documents`
- [ ] Falha de rede em `check_for_update` retorna `Ok(None)` + log (silencioso no boot), e o erro visível fica a cargo do "Verificar agora" na UI
- [ ] Gate check passa: `cd src-tauri && cargo test`
- [ ] Test count: acumulado passa (comandos de I/O não ganham teste — matriz de TESTING.md diz `none`)

**Tests**: none (comandos Tauri de orquestração de I/O — conforme a matriz)
**Gate**: build
**Verify**: `npm run tauri dev` sobe; `invoke("get_update_settings")` no console devolve versão, modo e o toggle
**Commit**: `feat(update): add update check, install and settings commands`

---

### T19: Cliente de update no frontend

**What**: Camada de acesso aos comandos e ao evento de progresso, mais os tipos.
**Where**: `src/lib/updateApi.ts`, `src/types.ts` (modificar)
**Depends on**: T18
**Reuses**: `src/lib/documentsApi.ts` (mesmo padrão de `invoke` + `listen`)
**Requirement**: REL-22

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Uma função por comando, tipada
- [ ] `onUpdateProgress(cb)` com `listen("update-download-progress")` devolvendo o unlisten
- [ ] `UpdateInfo`, `UpdateSettings` e `InstallFlavor` em `types.ts`, batendo com os structs do Rust
- [ ] Gate check passa: `npm run build`

**Tests**: none (matriz: componentes/frontend sem runner configurado)
**Gate**: build
**Commit**: `feat(update): add frontend update api client`

---

### T20: Store de update

**What**: Store Zustand com o estado do update e a verificação de boot condicionada.
**Where**: `src/store/updateStore.ts`
**Depends on**: T19
**Reuses**: `src/store/documentsStore.ts` (formato de store) e `configStore` (para saber se o onboarding terminou)
**Requirement**: REL-15, REL-24

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Estado: `available`, `settings`, `progress`, `installing`, `error`, `dismissed`
- [ ] `checkOnBoot()` só dispara com `onboarding_completed && auto_update_check`, com atraso de ~5 s, e engole erro (só log)
- [ ] `checkNow()` propaga o erro para a UI
- [ ] `install()`, `skip()`, `dismiss()`, `setAutoCheck()`
- [ ] Assina o progresso e cancela o listener ao desmontar
- [ ] Gate check passa: `npm run build`

**Tests**: none
**Gate**: build
**Commit**: `feat(update): add update store with boot check`

---

### T21: Banner de atualização

**What**: Aviso não bloqueante com as três ações e a barra de progresso.
**Where**: `src/components/Update/UpdateBanner.tsx`, `src/App.tsx` (modificar), `src/i18n/locales/{en,pt}.json` (modificar)
**Depends on**: T20
**Reuses**: padrão visual do `DocumentsPanel` (barra de progresso) e do `EmbeddedRuntimeCard` (card com ação + progresso)
**Requirement**: REL-16, REL-17, REL-18

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Mostra versão nova, versão atual e as notas da release
- [ ] Botões **Atualizar**, **Depois** e **Pular esta versão** ligados às ações da store
- [ ] Durante o download vira barra de progresso; erro aparece no lugar dos botões, com opção de tentar de novo
- [ ] Não empurra nem cobre o conteúdo a ponto de impedir o uso do app
- [ ] Se há geração de chat em andamento (`generatingChatId`), pede confirmação antes de instalar
- [ ] Todas as strings em `en.json` e `pt.json` — nenhuma literal no componente
- [ ] Gate check passa: `npm run build`

**Tests**: none
**Gate**: build
**Verify**: forçar `available` na store e conferir os três botões e o estado de progresso
**Commit**: `feat(update): add update notification banner`

---

### T22: Seção "Atualizações" em Configurações

**What**: Bloco em Configurações com versão instalada, modo, verificação manual e o toggle de opt-out.
**Where**: `src/components/Settings/SettingsPanel.tsx` (modificar), `src/i18n/locales/{en,pt}.json` (modificar)
**Depends on**: T21
**Reuses**: padrão de seção já usado no `SettingsPanel` (tema, idioma, pasta)
**Requirement**: REL-22, REL-23, REL-24

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Exibe a versão instalada e um badge Instalado/Portátil
- [ ] "Verificar agora" mostra resultado **nos dois casos** — inclusive "você já está na versão mais recente"
- [ ] Erro de rede na verificação manual aparece na tela (diferente do boot, que é silencioso)
- [ ] Toggle de verificação automática persiste e continua permitindo a verificação manual quando desligado
- [ ] Strings em `en.json` e `pt.json`
- [ ] Gate check passa: `npm run build`

**Tests**: none
**Gate**: build
**Verify**: desligar o toggle, reabrir o app e confirmar (log/rede) que nenhuma consulta sai no boot
**Commit**: `feat(update): add updates section to settings`

---

### T23: Enxugar o binário de release [P]

**What**: Perfil `release` com `strip` e LTO, e medição real do antes/depois.
**Where**: `src-tauri/Cargo.toml`
**Depends on**: T1
**Reuses**: nenhum
**Requirement**: REL-27

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `[profile.release]` com `strip = true`, `lto = true`, `codegen-units = 1`, `panic = "abort"` (avaliar `panic` — pode conflitar com o tratamento de erro do Tauri; se conflitar, deixar de fora e registrar)
- [ ] Tamanho do binário medido **antes e depois**, com os dois números anotados no commit e no STATE.md
- [ ] `cargo build --release` completa e o app continua abrindo
- [ ] Gate check passa: `cd src-tauri && cargo test`

**Tests**: none (configuração de build)
**Gate**: full — o app precisa abrir depois da mudança
**Verify**: comparar o tamanho de `target/release/LocalMind.exe` contra os 226 MB atuais
**Commit**: `build: strip and optimize the release profile`

---

### T24: Publicar uma release de verdade e atualizar nos dois modos

**What**: Executar o pipeline ponta a ponta e verificar o ciclo completo de atualização — a única forma de saber que isto funciona.
**Where**: nenhum arquivo; execução real + registro em `STATE.md` e nas tabelas de rastreabilidade
**Depends on**: T10, T11, T22, T23
**Reuses**: tudo
**Requirement**: verificação de REL-01 a REL-24

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Release `v0.2.0` (ou equivalente) publicada por disparo manual, com os 5 artefatos, os `.sig` e o `latest.json` com as 4 chaves de plataforma
- [ ] Confirmado que nenhum push em `master` durante o trabalho publicou release alguma (REL-02)
- [ ] `-setup.exe` instalado numa conta **sem** direitos de administrador, **sem nenhum prompt de UAC** (REL-09)
- [ ] Zip portátil descompactado numa pasta do usuário, app aberto, wizard completado, e **confirmado que nada foi escrito em `%APPDATA%`/`%LOCALAPPDATA%`** (REL-13)
- [ ] Segunda release publicada; app **instalado** avisa sozinho, atualiza e reabre na versão nova (REL-15→REL-19)
- [ ] Mesma segunda release: app **portátil** avisa, atualiza por troca de arquivos, reabre na versão nova, **sem UAC**, e o `.old` some no boot seguinte (REL-20)
- [ ] "Pular esta versão" testado: aquela versão não volta a avisar, uma posterior volta (REL-18)
- [ ] Toggle desligado → nenhuma requisição de rede no boot (REL-24)
- [ ] `spec.md` e este arquivo atualizados com os status reais; STATE.md com uma AD registrando o que foi verificado **e o que não foi**

**Tests**: none (verificação manual — é o gate `full` desta feature)
**Gate**: full
**Verify**: as evidências acima, coletadas de verdade. Nada aqui pode ser marcado por dedução ou por "compilou"
**Commit**: `chore(release): verify end-to-end release and update flow`

---

## Task Granularity Check

| Task | Escopo | Status |
| --- | --- | --- |
| T1 | 1 arquivo de config | ✅ Granular |
| T2 | 2 arquivos de config + ação externa | ✅ Granular (coeso: tudo é a chave) |
| T3 | 1 script + testes | ✅ Granular |
| T4 | 1 config + 1 arquivo gerado | ✅ Granular |
| T5 | 1 workflow | ✅ Granular |
| T6 | 1 job de 1 workflow | ✅ Granular |
| T7 | 1 script + testes | ✅ Granular |
| T8 | 1 job de 1 workflow | ✅ Granular |
| T9 | 1 script + testes | ✅ Granular |
| T10 | 1 job de 1 workflow | ✅ Granular |
| T11 | 1 documento | ✅ Granular |
| T12 | 1 módulo | ✅ Granular |
| T13 | 1 arquivo (modificação coesa) | ✅ Granular |
| T14 | 1 módulo | ✅ Granular |
| T15 | 1 módulo | ✅ Granular |
| T16 | 1 módulo | ✅ Granular |
| T17 | wiring de 1 plugin em 3 arquivos | ⚠️ OK — indivisível (o plugin só compila com as 3 pontas) |
| T18 | 1 arquivo de comandos + registro | ✅ Granular |
| T19 | 1 arquivo de API + tipos | ✅ Granular |
| T20 | 1 store | ✅ Granular |
| T21 | 1 componente + montagem | ✅ Granular |
| T22 | 1 seção em 1 componente | ✅ Granular |
| T23 | 1 seção do Cargo.toml | ✅ Granular |
| T24 | verificação (sem código) | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (corpo) | Diagrama mostra | Status |
| --- | --- | --- | --- |
| T1 | None | Fase 1 [P] | ✅ |
| T2 | T1 | T1 → T2 | ✅ |
| T3 | None | Fase 1 [P] | ✅ |
| T4 | None | Fase 1 [P] | ✅ |
| T5 | None | Fase 1 [P] | ✅ |
| T6 | T3, T4 | T3,T4 → T6 | ✅ |
| T7 | T1 | T1 → T7 | ✅ |
| T8 | T2, T6, T7 | T2,T6,T7 → T8 | ✅ |
| T9 | None | Fase 1 [P] | ✅ |
| T10 | T8, T9 | T8,T9 → T10 | ✅ |
| T11 | T10 | T10 → T11 | ✅ |
| T12 | None | Fase 1 [P] | ✅ |
| T13 | T12 | T12 → T13 | ✅ |
| T14 | T12 | T12 → T14 | ✅ |
| T15 | T12 | T12 → T15 | ✅ |
| T16 | T14, T15 | T14,T15 → T16 | ✅ |
| T17 | T2 | T2 → T17 | ✅ |
| T18 | T13, T16, T17 | T13,T16,T17 → T18 | ✅ |
| T19 | T18 | T18 → T19 | ✅ |
| T20 | T19 | T19 → T20 | ✅ |
| T21 | T20 | T20 → T21 | ✅ |
| T22 | T21 | T21 → T22 | ✅ |
| T23 | T1 | T1 → T23 | ✅ |
| T24 | T10, T11, T22, T23 | T10,T11,T22,T23 → T24 | ✅ |

Nenhuma task marcada `[P]` depende de outra `[P]` da mesma fase. T21 e T22 **não** são paralelas de propósito: as duas editam `en.json`/`pt.json`.

---

## Test Co-location Validation

Contra a matriz de `.specs/codebase/TESTING.md`:

| Task | Camada criada/modificada | Matriz exige | Task diz | Status |
| --- | --- | --- | --- | --- |
| T1, T2 | config de build | — | none | ✅ |
| T3, T7, T9 | lógica pura (scripts Node) | unit (por analogia com "funções puras") | unit (`node --test`) | ✅ |
| T4 | config declarativa | — | none | ✅ |
| T5, T6, T8, T10 | pipeline CI | — | none (o gate é a execução real em T24) | ✅ |
| T11 | documentação | — | none | ✅ |
| T12 | funções puras Rust | unit | unit | ✅ |
| T13 | funções puras Rust + I/O de config | unit | unit | ✅ |
| T14 | funções puras Rust | unit | unit | ✅ |
| T15 | parsing puro + 1 função HTTP | unit | unit | ✅ |
| T16 | funções puras + I/O de arquivo | unit | unit (só as puras; o fluxo completo em T24) | ✅ |
| T17 | wiring de plugin | none | none | ✅ |
| T18 | comandos Tauri (orquestração de I/O) | none | none | ✅ |
| T19–T22 | componentes/stores React | none | none | ✅ |
| T23 | perfil de build | — | none | ✅ |
| T24 | verificação manual | — | none | ✅ |

Nenhuma violação. As três tasks de script Node (T3, T7, T9) trazem seus próprios testes em vez de empurrá-los para frente — a matriz classifica "funções puras" como unit, e reescrever versão em JSON/TOML é exatamente isso.

---

## Requirement Coverage

| Requisito | Tasks |
| --- | --- |
| REL-01 | T6, T11 |
| REL-02 | T6 |
| REL-03 | T3, T6 |
| REL-04 | T3, T6 |
| REL-05 | T4, T6 |
| REL-06 | T6 |
| REL-07 | T6 |
| REL-08 | T8 |
| REL-09 | T1, T8 |
| REL-10 | T1, T2, T8, T9, T10 |
| REL-11 | T8, T10 |
| REL-12 | T1, T7, T8 |
| REL-13 | T12, T13 |
| REL-14 | T12, T13 |
| REL-15 | T12, T15, T18, T20 |
| REL-16 | T21 |
| REL-17 | T16, T18, T21 |
| REL-18 | T13, T18, T21 |
| REL-19 | T17, T18 |
| REL-20 | T16, T18 |
| REL-21 | T2, T14, T16 |
| REL-22 | T18, T19, T22 |
| REL-23 | T18, T22 |
| REL-24 | T13, T18, T20, T22 |
| REL-25 | T5 |
| REL-26 | T5 |
| REL-27 | T23 |

**27 requisitos, 27 mapeados, 0 órfãos.**
