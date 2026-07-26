# Release & Distribution (M8) — Especificação

**Context**: `.specs/features/release-distribution/context.md` (decisões do usuário sobre branches, versionamento, artefatos e UX de update)

## Problem Statement

O projeto não tem CI nenhum: `.github/` não existe, o repositório tem uma única branch (`master`), **zero tags** e a versão `0.1.0` repetida à mão em três arquivos que podem divergir a qualquer momento (`package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`). Nunca foi gerado um instalador. E mesmo que fosse, não haveria como entregar uma correção a quem já instalou — o usuário teria que descobrir sozinho que existe versão nova e reinstalar na mão. Pior: numa boa parte das máquinas corporativas onde este app faz sentido, **instalar exige credenciais de administrador que o usuário não tem**.

Esta feature fecha os três buracos de uma vez: um pipeline que produz releases versionadas por disparo manual, artefatos de instalação **e** um bundle portátil em toda release, e um mecanismo de auto-update que funciona nos dois modos sem nunca pedir elevação.

## Goals

- [ ] Publicar uma release completa é **um clique** ("Run workflow" + escolher `major`/`minor`/`patch`) — versão, CHANGELOG, tag, artefatos e release saem juntos, sem nenhum passo manual
- [ ] Nenhuma release acontece sem eu pedir: push em `master` **nunca** publica nada
- [ ] Toda release traz `.msi`, `-setup.exe`, `.deb`, `.AppImage` **e** um `.zip` portátil, todos assinados
- [ ] Um usuário sem direitos de administrador consegue rodar e **atualizar** o app do começo ao fim
- [ ] O app avisa que existe versão nova, respeita a resposta do usuário, e se atualiza sozinho quando autorizado

## Out of Scope

| Item | Motivo |
| --- | --- |
| Code signing (Authenticode / notarização) | Depende de comprar certificado e de burocracia externa; sem ele o SmartScreen avisa na 1ª execução, o que é aceitável para v1. Registrado em Deferred Ideas |
| macOS | Já é Future Consideration no ROADMAP; nenhum runner `macos-*` no pipeline |
| Canal beta / pré-releases | Modelo de branches escolhido foi `master` puro (context.md) |
| Bundle portátil no Linux | O `.AppImage` **já é** portátil (roda sem instalar) e **já é** suportado pelo updater oficial do Tauri, que substitui o próprio arquivo sem root. Um zip portátil de Linux seria pior: o binário nu depende de `webkit2gtk` do sistema, que o AppImage embute. Portátil = Windows |
| Delta updates | Complexidade desproporcional ao tamanho do projeto |
| `clippy -D warnings` / `fmt --check` no CI | O código atual não passa hoje (dead code conhecido, AD-033); adicionar junto transformaria "introduzir CI" numa refatoração |
| Rollback de versão pela UI | Fora do pedido; o `.old` da troca portátil dá um caminho manual de emergência |

---

## Research Findings (Knowledge Verification Chain)

Confirmado por documentação oficial e pela CLI local — **não** deduzido:

- **O updater oficial do Tauri 2 aceita só `.msi`, NSIS `-setup.exe` e `.AppImage`.** Não existe suporte a executável portátil nem a `.zip` no Windows. É exatamente por isso que o modo portátil precisa de código próprio.
- **O NSIS do Tauri já usa `installMode: currentUser` por padrão** — instala em `%LOCALAPPDATA%` e **não pede administrador**. `perMachine` e `both` é que exigem elevação. Ou seja, o instalador NSIS já resolve boa parte do problema de admin; o portátil cobre o caso mais duro (política que bloqueia qualquer instalador, execução de pendrive).
- **`tauri signer sign <FILE>` assina arquivo arbitrário** (verificado rodando `npx tauri signer sign --help` nesta máquina: aceita `<FILE>`, `--private-key`/`--private-key-path` e `--password`, com as env vars `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`). Logo, o `.zip` portátil pode ser assinado com **a mesma chave** dos instaladores — não é preciso um segundo mecanismo de confiança.
- **A assinatura é minisign (Ed25519).** O crate `minisign-verify` 0.2.5 ("zero-dependencies crate to verify Minisign signatures", ~4,1M downloads) é o que permite validar o zip do lado do app. É o mesmo esquema que o `tauri-plugin-updater` usa internamente.
- **`tauri-plugin-updater` está em 2.10.1** (publicado 2026-04-04). Desde a 2.10.0 o `latest.json` aceita chaves `{os}-{arch}-{installer}` (ex. `windows-x86_64-nsis`), além das clássicas `{os}-{arch}`. `platforms` é um mapa — chaves extras que o plugin não conhece são inertes para ele, o que abre espaço para uma chave `windows-x86_64-portable` lida só pelo nosso código.
- **`tauri-action` gera e publica o `latest.json`** junto dos artefatos, já com `version`, `url` e `signature` por plataforma.
- **Build de Linux precisa de `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`, `libfuse2`, `file`** e deve rodar no **sistema-base mais antigo que se pretende suportar** — `ubuntu-22.04`, porque compilar em base mais nova eleva o glibc mínimo e quebra em máquinas antigas.
- **Pré-requisito de build já documentado neste projeto** (`.specs/codebase/STACK.md`): **`protoc`** é exigido pelo `lance-encoding` (dependência do `lancedb`). Sem ele o `cargo build` falha. Precisa entrar nos dois runners.

### Fatos do repositório verificados nesta sessão

- `.github/` **não existe**; remote é `git@github.com:rafaelsene01/agent-local.git`; única branch `master`; **nenhuma tag**.
- O binário compilado hoje se chama **`tauri-app.exe`** (nome do pacote Cargo), não `LocalMind.exe` — o `productName` é `LocalMind` mas `mainBinaryName` não está configurado. Isso afeta diretamente o nome do executável dentro do zip portátil.
- `src-tauri/target/release/tauri-app.exe` tem **226 MB**. O zip portátil e os instaladores vão herdar essa ordem de grandeza.
- `tauri.conf.json` **não tem** `plugins.updater`, **não tem** `bundle.createUpdaterArtifacts` e usa `bundle.targets: "all"`.
- `capabilities/default.json` tem só `core:default`, `opener:default`, `dialog:default` — falta a permissão do updater.
- A config de bootstrap (`config.rs::bootstrap_file_path`) grava em `app_config_dir()` do SO (AD-012) e `default_base_path` usa `app_data_dir()`. **Nenhum dos dois é portátil**: um app "portátil" que escreve em `%APPDATA%` deixa rastro na máquina e não sobrevive a mudar de computador.

---

## User Stories

### P1: Publicar uma release com um clique ⭐ MVP

**User Story**: Como mantenedor, quero disparar a release manualmente escolhendo só o tipo de bump, para que versão, CHANGELOG, tag e release saiam prontos e consistentes sem eu editar arquivo nenhum.

**Why P1**: É o pedido central. Sem isso não existe release nenhuma, e tudo o mais depende de haver uma.

**Acceptance Criteria**:

1. WHEN o mantenedor abre o workflow de release no GitHub THEN o sistema SHALL oferecer **apenas** disparo manual (`workflow_dispatch`) com um select `bump` de três valores: `major`, `minor`, `patch`
2. WHEN qualquer push ou merge acontece em `master` THEN o sistema SHALL **não** publicar release alguma
3. WHEN o workflow roda THEN o sistema SHALL calcular a nova versão a partir da última tag `v*` aplicando o bump escolhido, e SHALL gravar essa mesma versão em `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` e `Cargo.lock`
4. WHEN não existe nenhuma tag `v*` no repositório THEN o sistema SHALL tratar a versão atual do `package.json` como "última publicada" e aplicar o bump sobre ela
5. WHEN a versão é calculada THEN o sistema SHALL gerar/atualizar `CHANGELOG.md` a partir dos Conventional Commits desde a última tag, agrupados por tipo
6. WHEN os arquivos são atualizados THEN o sistema SHALL criar um commit `chore(release): vX.Y.Z` em `master` e a tag `vX.Y.Z`, e SHALL publicar uma GitHub Release cujo corpo é a seção nova do CHANGELOG
7. WHEN o workflow é disparado de uma ref diferente de `master` THEN o sistema SHALL falhar imediatamente, antes de qualquer build
8. WHEN a tag calculada já existe no repositório THEN o sistema SHALL falhar com mensagem clara, sem sobrescrever nada

**Independent Test**: Disparar o workflow com `patch` num repositório sem tags e confirmar: tag `v0.1.1` criada, `CHANGELOG.md` commitado, release `v0.1.1` publicada, e os 5 arquivos de versão todos em `0.1.1`.

---

### P1: Toda release carrega os instaladores ⭐ MVP

**User Story**: Como usuário, quero baixar da página de releases o instalador do meu sistema, para instalar o app sem compilar nada.

**Why P1**: É o Goal literal do M8 no ROADMAP e o segundo pedido explícito do usuário.

**Acceptance Criteria**:

1. WHEN uma release é publicada THEN o sistema SHALL anexar a ela `.msi` e `-setup.exe` (Windows x86_64) e `.deb` e `.AppImage` (Linux x86_64)
2. WHEN o instalador NSIS é gerado THEN ele SHALL estar em modo `currentUser` — instalar em `%LOCALAPPDATA%` **sem** solicitar credenciais de administrador
3. WHEN os bundles são gerados THEN o sistema SHALL produzir também os artefatos de update assinados (`.sig`) e um `latest.json` com uma entrada por formato
4. WHEN qualquer job da matriz de build falha THEN a release SHALL permanecer em rascunho (draft), nunca publicada pela metade
5. WHEN o build de Linux roda THEN ele SHALL usar `ubuntu-22.04` como base, para não elevar o glibc mínimo exigido

**Independent Test**: Após uma release, baixar o `-setup.exe` numa conta Windows sem privilégios de administrador e instalar até o fim sem nenhum prompt de UAC.

---

### P1: Bundle portátil que roda sem instalar ⭐ MVP

**User Story**: Como usuário numa máquina que bloqueia instaladores, quero baixar um zip, descompactar numa pasta minha e rodar o app, para conseguir usá-lo sem administrador.

**Why P1**: É a motivação declarada do usuário ("pode ter computador que não deixa instalar, pedindo credenciais de administrador"). Sem ele, o público-alvo mais afetado fica de fora.

**Acceptance Criteria**:

1. WHEN uma release é publicada THEN o sistema SHALL anexar um `.zip` portátil de Windows x86_64, contendo o executável e um arquivo marcador que identifica o modo portátil
2. WHEN o `.zip` portátil é gerado THEN ele SHALL ser assinado com a mesma chave dos instaladores, e o `.sig` SHALL ser anexado à release
3. WHEN o app roda a partir de um bundle portátil THEN ele SHALL gravar a config de bootstrap e usar como pasta-base padrão um diretório **ao lado do executável**, nunca `%APPDATA%`/`%LOCALAPPDATA%`
4. WHEN o app roda a partir de uma instalação normal THEN o comportamento de armazenamento SHALL permanecer exatamente o de hoje (AD-008/AD-012), sem regressão
5. WHEN o app é iniciado a partir do zip descompactado em qualquer pasta gravável pelo usuário THEN ele SHALL abrir e completar o wizard de 1º uso sem nenhum prompt de elevação

**Independent Test**: Descompactar o zip em `C:\Users\<user>\Desktop\LocalMind`, rodar, completar o wizard, e confirmar que a pasta de dados nasceu dentro dessa mesma pasta e que nada foi escrito em `%APPDATA%`.

---

### P1: App avisa que existe versão nova e se atualiza ⭐ MVP

**User Story**: Como usuário, quero que o app me avise quando sair versão nova e se atualize sozinho se eu aceitar, para não precisar acompanhar o repositório.

**Why P1**: É o terceiro pedido explícito e o que dá sentido a ter pipeline de release.

**Acceptance Criteria**:

1. WHEN o app abre, o onboarding já está concluído e a verificação automática está ligada THEN o sistema SHALL consultar o manifesto de update em segundo plano, **sem bloquear a interface**
2. WHEN a versão publicada é maior que a instalada THEN o sistema SHALL exibir um aviso não bloqueante com o número da versão, as notas da release e três ações: **Atualizar**, **Depois** e **Pular esta versão**
3. WHEN a versão publicada é igual ou menor que a instalada THEN o sistema SHALL não exibir nada
4. WHEN o usuário escolhe **Atualizar** THEN o sistema SHALL baixar o artefato correspondente ao seu modo de instalação com **progresso visível**, verificar a assinatura, aplicar a atualização e reiniciar o app na versão nova
5. WHEN o usuário escolhe **Depois** THEN o aviso SHALL sumir nesta sessão e reaparecer na próxima verificação
6. WHEN o usuário escolhe **Pular esta versão** THEN o sistema SHALL não voltar a avisar sobre **aquela** versão específica, mas SHALL avisar normalmente sobre versões posteriores
7. WHEN o app está instalado (`.msi`/NSIS/`.AppImage`) THEN a atualização SHALL usar o `tauri-plugin-updater` oficial
8. WHEN o app está em modo portátil THEN a atualização SHALL baixar o `.zip`, validar a assinatura minisign contra a chave pública embutida na config, substituir os arquivos no lugar e relançar — **sem** nenhum prompt de administrador
9. WHEN a assinatura do artefato baixado não confere THEN o sistema SHALL abortar a atualização, manter a versão atual intacta e mostrar erro

**Independent Test**: Com a v0.1.1 instalada e a v0.1.2 publicada, abrir o app, ver o aviso, clicar Atualizar, ver o progresso, e o app reabrir reportando 0.1.2 — repetido nos dois modos (instalado e portátil).

---

### P2: Controle da verificação em Configurações

**User Story**: Como usuário, quero ver minha versão, verificar atualizações na hora e poder desligar a verificação automática, para manter o app 100% offline se eu quiser.

**Why P2**: O MVP funciona sem isso, mas o PROJECT.md promete "nenhuma chamada de rede externa por padrão" — o toggle é o que transforma a verificação automática numa escolha do usuário em vez de uma quebra silenciosa da promessa.

**Acceptance Criteria**:

1. WHEN o usuário abre Configurações THEN o sistema SHALL exibir uma seção "Atualizações" com a versão atualmente instalada
2. WHEN o usuário clica "Verificar agora" THEN o sistema SHALL consultar o manifesto na hora e informar o resultado, **inclusive quando já está atualizado** (o boot é silencioso, este não)
3. WHEN o usuário desliga o toggle de verificação automática THEN o sistema SHALL persistir a escolha e SHALL não fazer nenhuma consulta de rede no boot das próximas execuções
4. WHEN o toggle está desligado THEN "Verificar agora" SHALL continuar funcionando (a escolha é sobre o automático, não sobre o manual)
5. WHEN o app é instalado pela primeira vez THEN o toggle SHALL vir ligado

**Independent Test**: Desligar o toggle, fechar e reabrir o app com a rede monitorada, e confirmar que nenhuma requisição ao GitHub sai; depois clicar "Verificar agora" e ver a consulta acontecer.

---

### P2: CI de validação em push e PR

**User Story**: Como mantenedor, quero que todo push e PR compile e rode os testes, para nunca disparar uma release em cima de código quebrado.

**Why P2**: A release em si já roda o build (se não compilar, não sai artefato). Mas descobrir a quebra só no momento de publicar é o pior momento possível.

**Acceptance Criteria**:

1. WHEN há push em `master` ou abertura/atualização de PR THEN o sistema SHALL rodar `npm run build` (tsc + Vite) e `cargo test`
2. WHEN algum desses passos falha THEN o check SHALL ficar vermelho no commit/PR
3. WHEN o job Rust roda THEN ele SHALL instalar `protoc` e as dependências de sistema do Tauri antes de compilar
4. WHEN um PR é aberto THEN o sistema SHALL validar que os títulos/commits seguem Conventional Commits, já que o CHANGELOG depende disso

**Independent Test**: Abrir um PR com um erro de tipo em TypeScript e confirmar que o check falha; corrigir e confirmar que fica verde.

---

### P3: Reduzir o tamanho dos artefatos

**User Story**: Como usuário, quero baixar um arquivo menor, para que instalar e atualizar não custe centenas de megabytes.

**Why P3**: Não impede nada de funcionar. Mas o binário atual tem 226 MB, e é isso que trafega em toda atualização.

**Acceptance Criteria**:

1. WHEN o build de release roda THEN o perfil `release` SHALL remover símbolos de debug (`strip`) e SHALL usar LTO
2. WHEN o tamanho do artefato é medido antes e depois THEN a redução SHALL ser registrada no STATE.md como número real, não como estimativa

---

## Edge Cases

- WHEN o disparo acontece com a árvore de `master` já contendo um commit `chore(release):` sem tag correspondente THEN o sistema SHALL prosseguir normalmente (a tag é a fonte de verdade, não o commit)
- WHEN não há nenhum commit novo desde a última tag THEN o CHANGELOG SHALL sair vazio mas a release SHALL ser criada mesmo assim — o mantenedor pediu explicitamente
- WHEN o job de build do Windows passa e o do Linux falha THEN a release SHALL ficar em draft com os artefatos do Windows anexados, para o mantenedor decidir se publica ou re-roda
- WHEN o app é iniciado **sem rede** e a verificação automática está ligada THEN a falha SHALL ser silenciosa (só log) — nunca um erro na cara do usuário
- WHEN o usuário clica "Verificar agora" **sem rede** THEN o erro SHALL ser visível e explícito
- WHEN o app portátil está numa pasta somente-leitura (pendrive protegido, `C:\Program Files`) THEN o sistema SHALL detectar isso **antes** de baixar o zip e explicar que a atualização não é possível dali
- WHEN a substituição de arquivos do update portátil falha no meio THEN o sistema SHALL restaurar o executável anterior e manter o app utilizável na versão antiga
- WHEN sobrou um executável `.old` de uma atualização portátil anterior THEN o app SHALL apagá-lo no boot seguinte, sem incomodar o usuário
- WHEN o download da atualização é interrompido THEN o sistema SHALL descartar o parcial e permitir tentar de novo, sem deixar o app num estado meio-atualizado
- WHEN o usuário clica Atualizar com uma resposta sendo gerada no chat THEN o sistema SHALL avisar que o app vai reiniciar antes de prosseguir
- WHEN o `latest.json` não tem entrada para a plataforma/modo do usuário THEN o sistema SHALL tratar como "sem atualização disponível", não como erro
- WHEN a release mais recente do GitHub é um draft ou pré-release THEN o sistema SHALL ignorá-la

---

## Requirement Traceability

| Requirement ID | Story | Fase | Status (2026-07-26) |
| --- | --- | --- | --- |
| REL-01 | P1: Release só por `workflow_dispatch` com select de bump | Implemented | Escrito, **não executado** |
| REL-02 | P1: Push em `master` nunca publica release | Implemented | Verificado por inspeção (o arquivo não tem outro gatilho) |
| REL-03 | P1: Versão calculada da última tag + bump, gravada nos 5 arquivos | Implemented | Verificado (unit + dry-run real) |
| REL-04 | P1: Sem tag anterior, versão do `package.json` é a base | Implemented | Verificado (dry-run sem `--base` leu `0.1.0`) |
| REL-05 | P1: CHANGELOG gerado dos Conventional Commits desde a última tag | Implemented | Verificado (git-cliff rodado no histórico real) |
| REL-06 | P1: Commit `chore(release)`, tag `vX.Y.Z` e GitHub Release na mesma execução | Implemented | Escrito, **não executado** |
| REL-07 | P1: Disparo fora de `master` ou tag já existente falha antes do build | Implemented | Escrito, **não executado** |
| REL-08 | P1: `.msi` + `-setup.exe` + `.deb` + `.AppImage` anexados a toda release | Implemented | Escrito, **não executado** |
| REL-09 | P1: NSIS em `currentUser`, sem UAC | Implemented | Config explícita; **UAC não testado** |
| REL-10 | P1: Artefatos de update assinados + `latest.json` por formato | Implemented | Escrito; **depende da chave (T2)** |
| REL-11 | P1: Falha de build mantém a release em draft | Implemented | Escrito, **não executado** |
| REL-12 | P1: `.zip` portátil de Windows anexado e assinado com a mesma chave | Implemented | Script testado; **zip real não gerado** |
| REL-13 | P1: Modo portátil grava config e dados ao lado do executável | Implemented | Verificado por unit test; **não exercitado num zip real** |
| REL-14 | P1: Modo instalado mantém o comportamento de armazenamento atual | Implemented | Verificado por unit test |
| REL-15 | P1: Verificação silenciosa no boot, sem bloquear a UI | Implemented | Compila; **não clicado** |
| REL-16 | P1: Aviso não bloqueante com versão, notas e Atualizar/Depois/Pular | Implemented | Compila; **não clicado** |
| REL-17 | P1: Download com progresso, verificação de assinatura, reinício | Implemented | Partes puras testadas; **fluxo real não exercitado** |
| REL-18 | P1: "Pular esta versão" persiste e vale só para aquela versão | Implemented | Compila; **não clicado** |
| REL-19 | P1: Modo instalado atualiza via `tauri-plugin-updater` | Implemented | Compila; **não exercitado** |
| REL-20 | P1: Modo portátil atualiza por troca de arquivos, sem elevação | Implemented | Extração/rollback/cleanup testados; **troca real não exercitada** |
| REL-21 | P1: Assinatura inválida aborta e preserva a versão atual | Implemented | **Verificado** contra assinatura real do `tauri signer` + caso adulterado |
| REL-22 | P2: Seção "Atualizações" em Configurações com a versão instalada | Implemented | Compila; **não clicado** |
| REL-23 | P2: Botão "Verificar agora" com resultado visível nos dois casos | Implemented | Compila; **não clicado** |
| REL-24 | P2: Toggle de opt-out persistido; desligado = zero rede no boot | Implemented | Persistência testada; **ausência de rede não medida** |
| REL-25 | P2: CI de validação (`npm run build` + `cargo test`) em push e PR | Implemented | YAML válido; **não executado no GitHub** |
| REL-26 | P2: Validação de Conventional Commits em PR | Implemented | YAML válido; **não executado** |
| REL-27 | P3: `strip` + LTO no perfil de release, redução medida | Partial | Perfil configurado; **medição pendente**. `lto` ficou `"thin"` e `panic = "abort"` ficou de fora — ver tasks.md |

**ID format:** `REL-[NUMBER]`
**Status values:** Pending → In Design → In Tasks → Implementing → Verified
**Coverage:** 27 total, 27 mapeados para tasks, 27 implementados.

⚠️ **Nenhum requisito está `Verified` no sentido forte**, com uma exceção (REL-21). O gate desta feature é uma release publicada de verdade e um update aplicado de verdade — nada disso aconteceu ainda, porque depende da chave de assinatura (T2, ação humana). "Implemented" aqui significa "escrito, compila e tem os testes que dava para escrever", não "funciona".

---

## Success Criteria

- [ ] Uma release completa (5 artefatos + `latest.json` + CHANGELOG + tag) sai de um único "Run workflow" com um select preenchido
- [ ] Nenhum push em `master` publicou release alguma
- [ ] Numa conta Windows **sem** direitos de administrador: instalar pelo `-setup.exe` **e** rodar pelo zip portátil funcionam, ambos sem um único prompt de UAC
- [ ] Publicar uma versão nova faz o app avisar sozinho no próximo boot, nos dois modos
- [ ] Aceitar a atualização leva o app de vX para vY sem intervenção manual e sem elevação — verificado de verdade nos dois modos, não só compilando
- [ ] Desligar o toggle faz o app não emitir nenhuma requisição de rede no boot
