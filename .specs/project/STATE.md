# State

**Last Updated:** 2026-07-26
**Current Work:** **Auditoria spec-a-código de 2026-07-26 (ver AD-036).** A revisão achou uma coisa não implementada, quatro dívidas conhecidas e não pagas, um arquivo de diagnóstico esquecido no repositório, e — o mais enganoso — documentação defasada: ROADMAP e o cabeçalho desta STATE ainda diziam "M8 nada implementado" depois da AD-035, e a AD-035 dizia que a T2 estava bloqueada depois de o mantenedor tê-la concluído. Tudo corrigido. **Situação real agora:** M1, M2, M3, M3.1, M4, M5 e M7 completos; **M8 com 23 das 24 tasks** (falta a T24 — publicar release de verdade e atualizar nos dois modos); **M6 é o único milestone sem spec e sem código**. 123 testes Rust verdes, 27 de script Node, `npm run build` limpo. Contexto anterior: **M8 planejado em 2026-07-26** (ver AD-034): `.specs/features/release-distribution/` com `context.md` + `spec.md` (27 requisitos) + `design.md` + `tasks.md` (24 tasks), pronto para Execute e sem dependência do M6. Cobre as três coisas que o usuário pediu numa tacada: CI de release semântica com disparo **manual** (select `major`/`minor`/`patch`, e a execução faz versão + CHANGELOG + tag + release sozinha), artefatos de instalação em toda release (`.msi`, `-setup.exe`, `.deb`, `.AppImage`) **mais** um `.zip` portátil, e auto-update no app que funciona nos dois modos **sem pedir administrador**. Nada implementado ainda — é planejamento. Contexto anterior: RAG consertado de verdade em 2026-07-26 (ver AD-033, que corrige a AD-032): o `pdf-extract` estava engolindo letras inteiras em **51,3% dos chunks** do corpus do usuário, e três defeitos de montagem de contexto faziam o modelo copiar as próprias respostas anteriores em vez de ler o documento. Motor de PDF trocado por pdfium, trechos recuperados passaram a entrar colados na pergunta, orçamento de histórico invertido (derruba o antigo, não o recente) e janela real do modelo (21760) passou a ser consultada em vez do chute de 4096. 74 testes Rust verdes; build de release rodado e **verificado pelo usuário na UI** — a continuação do Art. 968 passou a sair correta depois de reimportar o documento. Contexto anterior: App rodado de verdade em 2026-07-25 (ver AD-028): conversa funcionando ponta a ponta com o llama.cpp embutido, depois de corrigir o timeout de 5 s que matava toda resposta longa e o status de conexão que nascia velho. Catálogo agora tem 6 modelos GGUF para o runtime embutido (URLs verificadas), a lista de instalados virou nome + tamanho/conexão, e trocar de modelo reinicia o sidecar. Antes disso, auditoria spec-a-código (ver AD-027): seis requisitos estavam implementados só no backend e foram fechados — toggle de RAG global, streaming ao trocar de chat, aviso de anexo com erro, citação da fonte nos trechos, pasta do modelo de embedding e importação parcial de documentos. 59 testes Rust verdes, build do frontend limpo. Contexto anterior: todas as features planejadas estão implementadas (2026-07-25): M3.1 (10/10), M7 (16/16), M5 `documents-rag` (11/11) e M4 `chat-messaging` (12/12). 58 testes Rust verdes + 4 `#[ignore]` que exercitam ONNX/LanceDB de verdade. Falta: verificação clicando na UI (listada em Todos) e os milestones M6 (memória de conversa) e M8 (empacotamento), que ainda não têm spec. Contexto anterior: M3.1 e M7 implementados em 2026-07-25 — 38 testes Rust verdes, `npm run build` e `npm run tauri dev` limpos, sidecar llama.cpp exercitado de verdade (ver AD-024). Pendente nos dois: os passos que exigem clicar na UI. Próximo: M5 (`documents-rag`, 11 tasks), depois M4 (`chat-messaging`, 12 tasks). Mapeamento brownfield completo (`.specs/codebase/`, 7 docs). Dois planejamentos prontos para Execute, **nesta ordem obrigatória**: (1) `single-active-connection` — spec + tasks (10 tasks), regra nova de uma conexão/um modelo ativos; (2) `embedded-runtime` (M7) — spec + context + design + tasks (16 tasks), llama.cpp embutido. `documents-rag` (M5) e `chat-messaging` (M4) vêm depois.

---

## Recent Decisions (Last 60 days)

### AD-036: Auditoria spec-a-código — o que faltava, e o que a documentação estava contando errado (2026-07-26)

**Decision:** Uma revisão requisito-a-requisito de todas as 8 specs contra o código, a pedido do usuário ("revise as specs e veja se tem algo não implementado"), seguida da correção de tudo que dava para corrigir sem uma release publicada. Sete frentes:

1. **A pasta-base que some entre sessões nunca foi tratada** (`settings-storage-i18n/spec.md`, edge case). O boot fazia `eprintln!`, deixava o `DbState` em `None`, e o `configStore` via `onboarding_completed: true` e entrava em `ready` — o app abria com cara de normal e **todo** comando falhava com "Nenhuma pasta de armazenamento configurada ainda". Entram `config::evaluate_storage` (decisão pura, 4 testes), o comando `get_storage_status` e a reabertura do wizard nomeando a pasta perdida, com tema, idioma e caminho anterior preservados — um drive removível que voltou vira um clique.
2. **`src-tauri/src/rag/diag.rs` removido.** Começava com "TEMPORARY diagnostic — delete after the investigation", tinha caminhos absolutos da máquina do usuário (`D:\aaaaaaaaaaa\…`) e **não estava declarado em nenhum `mod`** — código morto que a AD-032 dava como removido e não estava.
3. **As quatro dívidas de RAG da AD-033, pagas.** (a) `retrieve` usava `distance` e `chunk_index` para nada: agora os candidatos de todos os namespaces são ranqueados **juntos**, filtrados por um piso de relevância e expandidos com o chunk seguinte. (b) Falha de retrieval virou evento `chat-retrieval-warning` e aviso na conversa. (c) `RESPONSE_RESERVE_TOKENS` (512) saiu; o orçamento do prompt agora reserva exatamente o `answer_token_budget` que o provedor vai receber. (d) O `SYSTEM_PROMPT` deixou de exigir "o menor número de frases possível" e passou a amarrar o tamanho ao pedido.
4. **A versão parou de existir em dois lugares.** `tauri.conf.json` passou a declarar `"version": "../package.json"`, que o Tauri 2 resolve no build; `bump-version.mjs` escreve 3 arquivos em vez de 4.
5. **Tema `claude` renomeado para `terracotta`**, a pedido do usuário, com migração do id antigo.
6. **Documentação sincronizada com a realidade** (ver "O que a documentação contava errado", abaixo).
7. **Não corrigido, e por quê:** M6 não tem spec — planejá-lo é uma sessão de Specify, não uma correção; a T24 do M8 exige publicar uma release de verdade; os itens de verificação por clique exigem o usuário; e os C-03/C-04/C-06/C-10/C-11 do CONCERNS são refatorações fora do escopo desta revisão.

**Reason:** Pedido do usuário, seguido de "corrija tudo".

**O que a documentação contava errado (o achado mais perigoso):**
- **O ROADMAP dava o M8 como `📋 PLANEJADO` e as três features como `PLANNED`**, e o cabeçalho desta STATE dizia "Nada implementado ainda — é planejamento", tudo isso **depois** da AD-035 ter registrado o M8 implementado no mesmo dia. Quem lesse os documentos concluiria que o M8 não existia.
- **A AD-035 dizia que a T2 (chave de assinatura) estava bloqueada** e que `plugins.updater.pubkey` estava `""`. O `tasks.md` registra a T2 concluída pelo mantenedor às 19:36 UTC e o `tauri.conf.json` tem a chave pública preenchida. A AD-035 nunca foi corrigida; está corrigida agora.
- **As tabelas de rastreabilidade de `app-shell` (SHELL-01…08) e `settings-storage-i18n` (CFG-01…08) marcavam tudo como `Pending`** desde 2026-07-24, enquanto M1 e M2 estavam `✅ COMPLETE` no ROADMAP desde a mesma data.
- **O ROADMAP marcava as features do M3 como `PLANNED`** dentro de um milestone `✅ COMPLETE`.

**Verificado de verdade (não é "compilou"):**
- **O Tauri realmente lê a versão do `package.json`, e falha alto se não conseguir.** Trocando o campo para um caminho inexistente e rodando `cargo check`, o `tauri-build` aborta com ``tauri.conf.json > version` must be a semver string``. Experimento feito e revertido — é o que garante que a derivação não degrada em silêncio para uma versão errada.
- **`cargo test`: 123 passando, 0 falhas, 4 ignorados** (eram 112 — 11 testes novos: 4 do estado de armazenamento, 5 do ranqueamento de candidatos, 2 do orçamento e da derivação de versão).
- **`node --test`: 27 passando** (eram 25).
- **`npm run build` limpo**; i18n com **163 chaves em EN e 163 em PT**, sem divergência.
- **Os avisos de dead code de `distance` e `chunk_index` sumiram** do `cargo check` — é a prova mecânica de que os dois campos passaram a ser usados de fato.

**Trade-off/Notas:**
- **O piso de relevância é relativo ao melhor resultado, não absoluto.** Um piso absoluto de cosseno não separa nada com este modelo de embedding: pela medição da AD-025, um trecho **não relacionado** ainda marca 0,826 contra 0,957 de uma paráfrase. A razão para o melhor hit separa; o valor absoluto não. Constante em 3× com um mínimo de 0,1, porque um acerto exato tem distância 0 e zero vezes qualquer coisa continua zero.
- **A expansão de vizinho custa uma consulta por chunk selecionado** (até 4 por mensagem). É leitura filtrada por chave, não busca vetorial.
- **O orçamento de prompt encolheu** em janelas pequenas: com 4096 configurados, o prompt cai de 3584 para 2048 tokens. É a correção, não um efeito colateral — o app estava montando prompt até um limite que a própria resposta ia estourar.
- **O `retrievalWarning` é por chat ativo e some ao trocar de conversa.** Guardá-lo por chat seria mais estado para um aviso transitório.
- **A migração do tema é do lado do frontend** (`normalizeTheme` + regravação da config no boot). O backend não valida tema, então não havia onde colocar uma migração de banco.

**Não verificado (e não dá para verificar daqui):** nenhum dos fluxos novos foi exercitado clicando — o wizard de recuperação, o aviso de retrieval, o tema renomeado e o efeito prático da expansão de vizinho na qualidade das respostas seguem por verificar na UI.

### AD-035: M8 implementado — 23 de 24 tasks; a que falta não é código (2026-07-26)

> **Corrigida em 2026-07-26 pela AD-036:** este registro dizia "22 de 24" e dava a **T2 como bloqueada**, com `plugins.updater.pubkey` em `""`. O mantenedor concluiu a T2 no mesmo dia (par gerado, secrets cadastrados, chave pública commitada e validada por teste) e o `tasks.md` registra isso — só esta AD não foi atualizada. O número correto é **23 de 24**, e a única task aberta é a **T24**.

**Decision:** Executado o `tasks.md` de `release-distribution` inteiro, menos o que exige um humano ou uma release de verdade. Entraram: dois workflows (`ci.yml`, `release.yml`), três scripts Node com teste (`bump-version`, `make-portable`, `patch-latest-json`), o módulo `update/` no backend (`mod`, `signature`, `manifest`, `portable`), `update_commands.rs`, a bifurcação portátil no `config.rs`, e a UI (banner + seção em Configurações).

**Reason:** Pedido do usuário — "execute em paralelo todas task/spec não executada".

**O que está bloqueado, e por quê:**
- ~~**T2 (chave de assinatura)**~~ — **concluída pelo mantenedor no mesmo dia** (ver o `tasks.md`): par gerado, `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` cadastrados, `plugins.updater.pubkey` preenchida e coberta por um teste que falha se alguém colar a chave **privada** ali por engano — o que quase aconteceu (ver L-004). O motivo de nenhum agente fazer isso sozinho segue valendo: a senha e o segredo são do mantenedor.
- **T24 (verificação real)** — instalar/atualizar numa conta Windows sem administrador, a partir de uma release publicada de verdade. **Única task aberta do M8.**

**Verificado de verdade (não é "compilou"):**
- **O formato de chave do Tauri é mesmo a pegadinha que o design previu.** Rodando `npx tauri signer generate` e `sign` nesta máquina: o `.pub` é base64 de um arquivo minisign de 2 linhas, e o `.sig` é base64 do arquivo de 4 linhas — nenhum dos dois é o que `PublicKey::from_base64`/`Signature::decode` aceitam. A conversão virou `update::signature` com fixture real commitada; **assinatura válida passa, conteúdo adulterado é recusado**.
- `tauri signer sign <FILE>` **escreve `<FILE>.sig` ao lado do arquivo** — confirmado, e é disso que o workflow depende para o zip portátil.
- `git cliff` rodado contra o histórico real: 40+ commits agrupados corretamente, exit 0.
- `bump-version.mjs`: `minor` sobre `v1.9.3` → `1.10.0` (bump numérico, não lexicográfico), e sem `--base` lê o `package.json`.
- `is_newer("0.1.10", "0.1.9") == true` — o mesmo erro do outro lado, coberto por teste.
- Ambos os workflows passam por `yaml.safe_load`.
- `npm run build` limpo (1859 módulos); i18n com **158 chaves em EN e 158 em PT**, sem divergência.
- **`cargo test`: 112 passando, 0 falhas, 4 ignorados** (eram 74 antes do M8 — 38 testes novos, todos em `update::*`, `config::` e nos scripts Node contados à parte: mais 25 em `node --test`).
- **O app ainda sobe.** Esse era o risco real de regressão: registrar o `tauri-plugin-updater` com `pubkey: ""` poderia derrubar o boot. `npm run tauri dev` rodado — processo de pé por vários minutos, nenhum panic, nenhum erro no log. O plugin valida a chave em `app.updater()`, não na inicialização.
- **`mainBinaryName` não afeta o `cargo run` de desenvolvimento** — em dev o binário continua `tauri-app.exe`; o rename para `LocalMind.exe` acontece no bundling. Ou seja, `scripts/make-portable.mjs` só pode ser exercitado depois de um `tauri build`, não de um `tauri dev`.

**Trade-off/Notas (desvios conscientes do plano):**
- **Os pacotes npm do `tauri-plugin-updater` não foram instalados.** O frontend fala só com os nossos 5 comandos; o plugin é usado apenas do lado Rust. Uma superfície de API em vez de duas.
- **`lto = "thin"`, não `true`.** Fat LTO sobre arrow/lancedb/onnx joga o build do CI para dezenas de minutos por um ganho marginal. E **`panic = "abort"` ficou de fora**: removeria unwinding de que os stacks SQLite/Arrow podem depender, o que troca bytes por uma classe de crash que só aparece em produção. A **medição** do REL-27 segue pendente (exige build de release completo).
- **`platform_key(Installed)` devolve `None`**, não uma chave de instalador. O design dizia "a chave do instalador", mas quem resolve isso no modo instalado é o próprio plugin — devolver uma chave que nunca usamos seria código morto que parece útil.
- **`current_exe()` é lido antes da troca**, não depois: no Windows o caminho da imagem é cacheado no PEB e não acompanha o rename, então ler depois seria apostar em comportamento não documentado.
- **`app.restart()` não serve no caminho portátil** (relançaria o `.old`); é spawn explícito do caminho novo + `exit(0)`.
- **`pickAssetUrl` por substring é ambíguo na vida real**: `"…x64-portable.zip"` casa também com `"…x64-portable.zip.sig"`. O workflow usa `pickAssetUrlByName` (nome exato), e a versão por substring recusa ambiguidade em vez de escolher errado.
- **`ci.yml` roda `cargo test` só em `ubuntu-22.04`**, não em matriz — decisão registrada na AD-034.
- **Só o `.zip` portátil é Windows.** No Linux o AppImage já cobre o caso.

**Não verificado (e não dá para verificar daqui):** publicar uma release, gerar os instaladores, o zip portátil real, instalar sem UAC, a troca de arquivos do update portátil, qualquer clique na UI nova, e se o `tauri-plugin-updater` de fato ignora a chave `windows-x86_64-portable` no `latest.json` (Open Question #2 do design — o plano B é um manifesto separado, uma linha no `finalize` e uma URL no `manifest.rs`).

### AD-034: M8 planejado — release manual com versão semântica, bundle portátil e auto-update sem administrador (2026-07-26)

**Decision:** Planejamento completo do M8 em `.specs/features/release-distribution/` (context + spec + design + tasks). Quatro escolhas fecharam o desenho, todas confirmadas pelo usuário por pergunta direta:

1. **Branches:** `master` + feature branches. Sem `develop`, sem `release/*` — projeto solo, um mantenedor. Releases saem sempre de `master` e o workflow recusa disparo de qualquer outra ref.
2. **Versionamento:** `workflow_dispatch` com **select `major`/`minor`/`patch`** — o usuário escolhe o bump, e a mesma execução calcula a versão a partir da última tag, grava nos 5 arquivos que a duplicam, gera o CHANGELOG (git-cliff, dos Conventional Commits), commita, tagueia e publica a release. Nenhuma versão digitada à mão. Descartados `semantic-release`/`release-please`: deduzir a versão dos commits foi explicitamente recusado.
3. **Artefatos:** instaladores nativos **e** `.zip` portátil, com **dois caminhos de atualização e uma única UI** — instalado usa o `tauri-plugin-updater` oficial, portátil usa atualizador próprio.
4. **UX:** verifica no boot + botão "Verificar agora" em Configurações + **toggle de opt-out** (padrão ligado).

**Reason:** Pedido literal do usuário — *"ajuste o CI do gitflow, para termos releases semânticas, mas lançamento de novas release eu quero engatilhar manualmente… pois pode ter computador que não deixa instalar, pedindo credenciais de administrador"*.

**Pesquisa obrigatória cumprida (verificada, não deduzida):**
- **O updater oficial do Tauri 2 aceita só `.msi`, NSIS `-setup.exe` e `.AppImage`** — **não** tem suporte a portátil/zip no Windows. É esta a razão de o modo portátil precisar de código próprio; não é preferência.
- **O NSIS do Tauri já usa `installMode: currentUser` por padrão** (instala em `%LOCALAPPDATA%`, sem UAC). Ou seja, boa parte do problema de admin já se resolve por config — o portátil cobre o caso mais duro (política que bloqueia instaladores, execução de pendrive).
- **`tauri signer sign <FILE>` assina arquivo arbitrário** — confirmado rodando `npx tauri signer sign --help` nesta máquina. Logo o zip portátil usa **a mesma chave** dos instaladores: um segredo, uma rotação, uma superfície de confiança.
- **`minisign-verify` 0.2.5** (zero deps, ~4,1M downloads) é o que valida a assinatura do lado do app. **Pegadinha registrada no design:** o `tauri signer` emite o *arquivo* minisign inteiro em base64 (2 linhas, com `untrusted comment:`), enquanto o crate espera a linha da chave — a conversão é pura string e ganhou teste unitário com par de chaves real, porque é o tipo de bug que passa em `cargo check` e só falha no dia do update.
- **`tauri-plugin-updater` 2.10.1** (2026-04-04); desde a 2.10.0 o `latest.json` aceita chaves `{os}-{arch}-{installer}`. `platforms` é um mapa, então uma chave extra `windows-x86_64-portable` convive com as oficiais — um manifesto, dois leitores.
- **Linux precisa compilar em `ubuntu-22.04`**: base mais nova eleva o glibc mínimo e quebra em máquinas antigas.

**Trade-off/Notas:**
- **Portátil é Windows-only.** No Linux o `.AppImage` já roda sem instalar, já é atualizável pelo plugin oficial sem root, e embute o `webkit2gtk` que o binário nu exigiria do sistema — um zip de Linux seria estritamente pior.
- **Troca de arquivos sem processo auxiliar:** no Windows não se sobrescreve um `.exe` em execução, mas **se renomeia**. O fluxo é rename-then-replace com rollback; dispensa um helper que seria mais um binário para assinar, distribuir e explicar ao antivírus corporativo. `app.restart()` não serve depois do rename (aponta para o `.old`) — é spawn explícito + `exit(0)`.
- **Tensão real com o offline-first do PROJECT.md:** verificar update é chamada de rede. O toggle de opt-out é o que a transforma em escolha do usuário, e a verificação só roda **depois** do onboarding concluído. Foi decisão do usuário deixar ligado por padrão.
- **Modo detectado por marcador `.portable`**, não por caminho: NSIS `currentUser` instala em `%LOCALAPPDATA%` e o portátil pode ser descompactado em qualquer lugar, inclusive `Program Files`.
- **O portátil obriga a mexer no `config.rs`:** um app "portátil" que grava em `%APPDATA%` não é portátil. `bootstrap_file_path` e `default_base_path` ganham uma bifurcação por modo — a AD-012 e a AD-008 seguem valendo, muda só *onde* o ponteiro mora.
- **`cargo test` só em `ubuntu-22.04` no CI de validação**, não em matriz: o build é caro (lancedb/fastembed/rusqlite bundled) e o que diverge por SO é o *bundling*, exercitado na release.
- **`mainBinaryName` vai mudar** de `tauri-app` para `LocalMind` — hoje o executável compilado se chama `tauri-app.exe` apesar do `productName` ser `LocalMind`. Não há release publicada, é a hora certa.
- **Fora de escopo por decisão:** code signing (sem certificado, o SmartScreen vai avisar na 1ª execução), macOS, canal beta, delta updates, e `clippy -D warnings`/`fmt --check` no CI (o código atual não passa — ver as dívidas da AD-033 — e isso viraria uma refatoração disfarçada de "introduzir CI").

**Números do estado atual que o plano precisa encarar:** `.github/` não existe; **zero tags**; versão `0.1.0` repetida em 3 arquivos; `tauri-app.exe` com **226 MB** (é isso que trafega em cada atualização — daí o REL-27 de `strip`+LTO, com a redução a ser **medida**, não estimada).

**Impact:** M8 sai de "PLANNED sem spec" para planejado por inteiro no ROADMAP. Resolve o C-09 do CONCERNS.md (sem linter/CI) na parte de CI. **Nada implementado** — o gate `full` desta feature não é "compila", é uma release publicada de verdade e um update aplicado de verdade nos dois modos (T24), justamente a classe de coisa que as AD-024/AD-028 mostraram que só aparece quando se executa.

### AD-033: O `pdf-extract` corrompia metade do corpus, e o contexto do RAG estava no lugar errado do prompt — corrige a AD-032 (2026-07-26)

**Decision:** Quatro mudanças, todas medidas contra a base real do usuário:

1. **Motor de PDF trocado: `pdf-extract` 0.12 → `pdfium-render` 0.9.3**, com a biblioteca baixada em runtime (`rag/pdfium.rs`), mesmo padrão do llama.cpp (AD-022) e do ONNX Runtime (AD-025). Release fixado em `bblanchon/pdfium-binaries` `chromium/7961`, asset `pdfium-win-x64.tgz` verificado ao vivo (200, 3,74 MB). A feature `thread_safe` do crate é default e serializa o acesso, então DOC-07 (dois documentos indexando junto) não precisou do tratamento que o `embedding.rs` teve que fazer com o `INIT_LOCK`.
2. **Trechos recuperados entram no mesmo turno da pergunta**, logo acima dela (`question_with_context`), em vez de num bloco `system` no topo do prompt.
3. **Orçamento de histórico consumido do mais novo para o mais antigo** (`fit_history`). Antes, `recent_history` era percorrido em ordem cronológica e o `budget.take` gastava o orçamento nas mensagens velhas — quando apertava, quem era descartado era o turno recente.
4. **`context_length` NULL passou a ser resolvido no provedor** (`budget_context` → `ProviderClient::model_limits`), com fallback silencioso. O sidecar reporta `n_ctx_slot = 21760` e o montador assumia 4096.

**Reason:** Usuário relatou de novo que a IA não continuava um trecho do documento, e desta vez que "no documento que está no RAG, os textos estão completamente diferentes". A AD-032 tinha fechado o caso como limitação do modelo — estava errado.

**Evidência (medida, não deduzida):**
- **Corrupção quantificada:** 322 de 628 chunks (**51,3%**) continham pelo menos uma palavra destruída. 551 ocorrências de "que" tinham perdido o `q` contra 3.144 intactas (14,9%). O `pdf-extract` engolia `q`, `v`, `x`, `b`, `f` e todas as vogais acentuadas, além de vírgulas e hífens: "salvo se o exercício da profissão" saía como `salo se o eerccio da profisso`.
- **Não era PDF quebrado nem caso de OCR:** o `pdftotext` (poppler) lê o mesmo arquivo com **zero** perdas — 3.227 "que", nenhum quebrado. Foi a referência independente que provou que o defeito era do crate. O pdfium pelo caminho do app deu **exatamente os mesmos 3.227/0**.
- **A montagem de prompt era um defeito separado:** para a pergunta do Art. 968 o chunk 257 estava **íntegro** e mesmo assim o modelo errava. Olhando as mensagens no banco, a resposta das 02:55 era cópia quase literal da das 02:11 — o modelo estava imitando o próprio histórico, que ficava colado na pergunta enquanto o documento ficava ~10 mil chars acima.

**Trade-off/Notas:**
- Documentos indexados antes desta mudança continuam corrompidos no LanceDB; **nada reindexa sozinho**, é apagar e reimportar. O usuário fez isso e confirmou o resultado na UI.
- `pdfium-render` traz o crate `image` junto pelas features default. Dá para enxugar com `default-features = false` + `["pdfium_latest", "thread_safe"]`; não feito, para não trocar risco por bytes sem necessidade.
- O valor resolvido de `context_length` alimenta **só** o orçamento do prompt; o que vai para `stream_chat` continua sendo o configurado, para não mudar o que é enviado ao provedor de carona.
- `fit_history` e `question_with_context` foram extraídas como funções puras exatamente para serem testáveis — `assemble` exige um `AppHandle` e não é coberto por teste.

**Impact:** 74 testes Rust verdes (6 novos). `pdf-extract` saiu do `Cargo.toml`. **Verificado pelo usuário na UI:** depois de reimportar o documento e abrir um chat novo, a continuação do Art. 968 saiu correta.

**~~Ainda em aberto~~ — as quatro pagas em 2026-07-26 pela AD-036:**
- ~~`retrieve` descarta `distance` e `chunk_index`~~ → candidatos de todos os namespaces ranqueados juntos, piso de relevância **relativo ao melhor hit** (um absoluto não separa nada com este modelo) e expansão para o chunk `index+1`. Os dois warnings de dead code sumiram do `cargo check`.
- ~~Falha de retrieval é invisível~~ → evento `chat-retrieval-warning` e aviso na conversa, separado do erro da mensagem.
- ~~`RESPONSE_RESERVE_TOKENS` (512) não bate com `answer_token_budget`~~ → a constante saiu; o orçamento reserva exatamente o que o provedor vai receber.
- ~~O `SYSTEM_PROMPT` briga com "continue este texto"~~ → "menor número de frases possível" saiu; o tamanho agora é amarrado ao pedido, mantendo as cláusulas anti-cortesia.

### AD-032: ~~"O RAG não funciona" — o RAG funciona, o modelo é que é fraco~~ — **PARCIALMENTE CORRIGIDA em 2026-07-26 pela AD-033** (2026-07-25)

> **O que se sustentou:** o pipeline de retrieval funciona mesmo — a busca devolve o chunk certo em primeiro lugar, e o `rejoin_hyphenated_words` era uma correção real.
>
> **O que caiu:** a conclusão "o modelo é que é fraco" e o veredito de que a perda de letras era "limitação registrada, sem correção". Nunca foi testado outro extrator; quando foi, o poppler leu o mesmo PDF perfeitamente e o pdfium resolveu por completo. A investigação também não mediu o estrago — eram **51,3% dos chunks**, não "partes do texto". E o "1 acerto em 4" atribuído ao modelo tinha uma causa estrutural: o histórico com as respostas erradas anteriores ficava mais perto da pergunta do que o documento. Ver AD-033.
>
> Texto original preservado abaixo como histórico.

**Decision:** Nenhuma mudança na arquitetura de RAG. A investigação (diagnóstico temporário rodando contra a base real do usuário, removido depois) mostrou:
- O documento estava `ready`, `use_global_rag = 1`, e a busca devolveu **o trecho certo em 1º lugar** — o chunk 259 contém literalmente "Art. 968. A inscrição do empresário far-se-á mediante requerimento que contenha: I – o seu nome…". Nenhum "retrieval skipped" no log.
- Reproduzindo o **prompt real inteiro** (10.365 chars, 4 chunks) contra o sidecar: o Phi-3.5 acerta a continuação **1 vez em 4**. Nas outras, responde com *outra* frase verdadeira do mesmo artigo (o §1º) — ou seja, usa o documento, mas erra a passagem.
- `temperature` não é a causa: 1/4 em 0.8 e 1/4 em 0.2. Reordenar os trechos e reduzir para top-2/top-1 também não deu resultado estável.

**O que era defeito de verdade e foi corrigido:** o PDF quebra palavras na paginação e o extrator entregava "liqui- dação", "empre- sário". `rejoin_hyphenated_words` junta os pedaços quando há hífen + espaço + minúscula, preservando hífen legítimo ("far-se-á", "guarda-chuva") e início de frase. Vale para documentos importados **daqui em diante** — os já indexados precisam ser reimportados.

**Limitação registrada, sem correção:** o mesmo PDF perde letras em partes do texto ("crdito", "cnjuge soreio", "atiidade", "profisso") — é o `pdf-extract` não resolvendo a codificação de fonte daquelas seções. Já está na versão mais recente publicada (0.12, 2026-06-25), então não há bump disponível; trocar de motor (pdfium baixado em runtime, como o llama.cpp e o ONNX Runtime) seria a saída.
**Reason:** Usuário relatou que a IA não continuou uma frase que estava no documento e concluiu que "possivelmente o RAG não está funcionando".
**Impact:** O caminho para respostas melhores sobre documento é modelo maior — o catálogo já oferece Qwen2.5 7B e Llama 3.1 8B, que cabem na RTX 3060 de 12 GB desta máquina.

### AD-031: Turnos precisam alternar — geração interrompida deixava dois `user` seguidos (2026-07-25)

**Decision:** `assemble` passou a normalizar a conversa com `merge_consecutive_turns`: mensagens seguidas do mesmo papel viram um turno só (unidas por linha em branco). Isso cobre o caso real — cancelar ou quebrar uma geração persiste a pergunta **sem resposta**, então todo pedido seguinte mandava dois `user` em sequência — e de quebra funde os dois `system` (prompt base + contexto) num só, que é o que templates de um único system esperam. O `SYSTEM_PROMPT` também ficou mais restritivo contra parágrafo de cortesia.

**Reason:** Usuário perguntou por que o assistente respondia "oi" e emendava "Sinta-se à vontade para compartilhar seus pensamentos…".
**Evidência (medida no sidecar, mesmo histórico, só mudando a estrutura):**
- com a mensagem órfã → *"Entendido! Se você tiver mais perguntas… fique à vontade para perguntar."* (119 chars de cortesia)
- sem ela, turnos alternando → *"Olá! Como posso ajudá-lo hoje?"* (30 chars)

O `/apply-template` do llama.cpp confirmou que o prompt em si estava bem formado (`<|system|>…<|end|><|user|>…<|end|><|assistant|>`) — o defeito era a sequência de papéis, não a formatação.
**Verificado na UI:** chat novo respondeu curto e direto. O chat antigo continua degradado porque o histórico dele guarda o texto da geração desgovernada da AD-030 — o modelo imita o que já está na conversa. Não há limpeza retroativa: apagar mensagens do usuário sem ele pedir seria pior.
**Ainda em aberto:** o Phi-3.5 é verborrágico por natureza e às vezes ainda fecha com uma frase de cortesia. As alavancas restantes seriam expor `temperature` (hoje fica no padrão 0.8 do llama-server) ou usar um modelo menos falante.

### AD-030: A pergunta ia duplicada no prompt e a resposta não tinha teto — chat entrava em loop (2026-07-25)

**Decision:** Duas correções na montagem e no envio da conversa:
1. **`send_message` persiste a mensagem do usuário antes de montar o contexto**, e o `recent_history` lia as últimas 20 mensagens do banco — incluindo essa. Como o `assemble` ainda anexa a pergunta no fim, o modelo recebia **dois turnos `user` idênticos e seguidos**. `assemble` passou a receber o id da mensagem e o `SELECT` a excluí-la (`AND id <> ?`).
2. **Nenhum teto de geração.** `max_tokens` só era enviado quando havia contexto configurado — e, quando ia, era o tamanho da janela inteira, não o orçamento da resposta. Entra `providers::answer_token_budget()`: 2048 tokens por padrão, limitado a metade da janela quando ela é pequena. Vale para o caminho OpenAI-compatible (`max_tokens`) e para o Ollama (`num_predict`, que também é ilimitado por padrão).

**Reason:** Relato do usuário — "mandei uma mensagem e ele bugou, parece que a resposta está em loop dando enter infinito". O log do sidecar mostrou `n_decoded = 6189` e subindo, sem parar.
**Evidência (chamada direta ao sidecar, não dedução):** com os dois turnos `user` duplicados o Phi-3.5 emenda seções novas e nunca emite o stop (`finish_reason: "length"` no teto artificial de 80 tokens); com um único turno, `finish_reason: "stop"` e resposta fechada. O bug era o prompt malformado; o teto de tokens é a rede de segurança para quando o modelo erra o stop token de qualquer forma.
**Verificado na UI:** pergunta enviada depois da correção respondeu e parou sozinha.
**Trade-off:** resposta acima de 2048 tokens é cortada. Preferível a um chat travado, e o corte é visível.

### AD-029: Tamanho de contexto vira spinner com o teto real do modelo (2026-07-25)

**Decision:** O campo de contexto (CONN-12) deixou de ser um número solto: agora é spinner (`min` 512, `step` 512, `max` = janela treinada do modelo) + slider, com o rótulo "máx. X · em uso: Y". O teto vem de um comando novo, `model_limits`, e cada provedor responde do jeito que sabe:
- **llama.cpp (embutido/custom)**: `GET /v1/models` → `data[].meta.n_ctx_train` (teto) e `meta.n_ctx` (alocado). **Verificado ao vivo** no sidecar rodando: 131072 e 21760 para o Phi-3.5.
- **Ollama**: `POST /api/show` → `model_info["<arch>.context_length"]` — o prefixo é a arquitetura (`llama.`, `gemma4.`…), então a chave é casada por sufixo. Confirmado na doc oficial, **não** contra um Ollama rodando (não há um nesta máquina).
- **LM Studio**: `max_context_length` da listagem de modelos. Documentado, **não verificado ao vivo**.
- Qualquer outro: `ModelLimits::default()` — sem teto, o campo continua livre e o slider nem aparece.

**Reason:** Pergunta do usuário: "o tamanho de contexto poderia ser um spinner? cada modelo tem tamanho máximo, é possível já ter essa informação?". Tem sim, e vinha sendo ignorada.
**Trade-off/Notas:** `max_context` e `current_context` são `Option`; um provedor que não informa não ganha um número inventado — a UI cai para campo livre. O teto é a janela **treinada**, não o que cabe na memória: pedir 131072 no llama.cpp pode falhar ao alocar o KV cache, e é por isso que o "em uso" aparece ao lado.
**Verificado na UI:** o formulário abriu mostrando `máx. 131.072 · em uso: 21.760` com spinner e slider funcionando. De quebra, a tela confirmou que o download de GGUF do catálogo funciona: o `Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` (1.0 GB) apareceu na lista de instalados depois de baixado pelo card.

### AD-028: App rodado de verdade — 2 bugs de bloqueio encontrados, e o catálogo passou a servir o runtime embutido (2026-07-25)

**Decision:** `npm run tauri dev` executado e a UI dirigida por script (clique/screenshot via PowerShell). Rodar achou o que teste nenhum tinha achado:

1. **Timeout de 5 s matava toda resposta longa.** Os três `ProviderClient` construíam o `reqwest::Client` com `.timeout(5s)`, que no reqwest vale para a requisição inteira — inclusive o corpo. O `llama-server` registrou `stop: cancel task` exatos 5 s depois de começar a gerar, e a UI ficava em "Gerando…" para sempre. O mesmo timeout também limitaria um `pull` de modelo de vários GB. Trocado por `providers::http_client()`: `connect_timeout` de 5 s (falha rápido quando não há ninguém escutando) e **nenhum** timeout total; chamadas curtas passaram a declarar `SHORT_REQUEST_TIMEOUT` (30 s) por requisição. Teste de regressão em `openai_stream` com servidor falso que espera 7 s antes do primeiro token.
2. **Status de conexão nascia velho.** As conexões eram checadas uma única vez, no boot da sidebar — antes do sidecar terminar de carregar o modelo (~5 s). O runtime embutido ficava "indisponível" até o usuário atualizar na mão, e a aba Modelos não listava nada. O autostart passou a emitir `connections-changed`, e Conexões/Modelos recarregam ao abrir.

**Pedidos do usuário atendidos na mesma passada:**
- **Lista de modelos instalados** virou uma lista plana: nome à esquerda, `tamanho em GB · conexão` à direita. Os três blocos "esta conexão não está respondendo" saíram.
- **Botão "Baixar" que não baixava**: todo o catálogo era `provider: "ollama"`, então sem Ollama rodando o botão ficava desabilitado com o motivo escondido num `title`. O motivo agora é texto visível, e o que dá para baixar aparece primeiro.
- **Modelos para o runtime embutido**: seis entradas GGUF novas no catálogo (Qwen2.5 1.5B/7B, Llama 3.2 3B, Phi-3.5 Mini, Mistral 7B v0.3, Llama 3.1 8B). Cada URL foi verificada com `HEAD` (200 + `content-length`) e o `content-length` virou `download_bytes` — o card mostra o tamanho real de download, não a estimativa de RAM.
- **Trocar de modelo no runtime embutido** passou a funcionar: `list_installed_models` do `EmbeddedClient` lê os `.gguf` da pasta (o `/v1/models` só conhece o que está carregado e não tem tamanho), e `set_active_model` virou async — para o provider `embedded` ele reescreve `embedded_runtime.model_path` e reinicia o sidecar, porque o modelo é flag de inicialização.
- **A mensagem do usuário aparece na hora** (otimista na store): antes ela só surgia quando a geração terminava, porque o comando só retorna no fim.
- **Instrução de citação saiu do system prompt** e foi para o bloco de contexto: sem documento nenhum, o Phi-3.5 imitava o formato e respondia "[fonte: GPT-3 informações geral]".

**Verificado ao vivo:** app abre; sidecar sobe sozinho no boot (EMBED-06, agora exercitado de verdade); conexão embutida fica verde; Phi-3.5 listado como `2.4 GB · Runtime embutido`; marcar como ativo funciona; **conversa real com streaming respondeu duas perguntas** pelo llama.cpp embutido.
**Não verificado:** se um chat **novo** (sem histórico contaminado) ainda produz "[fonte: ...]" inventado — as duas observações vieram de um chat cujo histórico já continha o padrão. Também não testados por clique: download de um GGUF do catálogo, troca entre dois modelos com restart do sidecar, e o fim do processo ao fechar o app (EMBED-07).

### AD-027: Auditoria de código fechou 6 requisitos implementados pela metade (2026-07-25)

**Decision:** Uma auditoria spec-a-código (a pedido do usuário) encontrou seis requisitos em que o backend cumpria e a UI não fechava o ciclo. Todos corrigidos na mesma sessão; 59 testes Rust verdes (era 58, +1 novo) e `npm run build` limpo.

1. **CHAT-14 — o toggle nunca era lido de volta.** `ChatPanel` guardava `useState(true)` local e `list_chats` não devolvia a coluna. `models::Chat` ganhou `use_global_rag`, `SELECT_CHAT` passou a ser um só (list/rename), e a store atualiza a lista junto com o banco. Trocar de chat agora mostra a escolha real de cada um.
2. **Trocar de chat durante o streaming corrompia a lista.** O `finally` de `sendMessage` recarregava as mensagens do chat que enviou e jogava em `messages` sem checar quem estava na tela. Estado passou de `isGenerating`/`streamingContent` globais para `generatingChatId` + `streamingChatId`: o parcial continua acumulando em background e reaparece ao voltar, e `cancelGeneration` cancela o chat que está gerando, não o que está visível.
3. **CHAT-10 — falha de anexo era invisível.** Nenhum comando lia `chat_attachments`. Entrou `list_chat_attachments` (sem `extracted_text`, que pode ter milhares de chars); o chat mostra os anexos aceitos e um aviso por anexo com erro. O seletor de anexo ganhou o filtro de formatos e recusa não suportados **antes** do envio, como o edge case pedia.
4. **Citações (DOC-12 no consumo do M4).** `retrieve` descartava o `doc_id` que o `VectorStore` já devolvia. Cada bloco agora entra como `[fonte: <arquivo>]`, resolvido em `documents` ou `chat_attachments` conforme o namespace, e o system prompt manda citar. Anexo pequeno injetado inteiro usa o mesmo formato.
5. **Modelo de embedding fora da pasta-base na 1ª sessão.** `set_cache_dir` só rodava no boot com config existente; quem acabava de passar pelo wizard baixava os ~120MB no cache padrão do fastembed. `MODEL_CACHE_DIR` virou `Mutex<Option<PathBuf>>` (era `OnceLock`) e `complete_onboarding`/`update_base_path` passaram a apontá-lo. Vale a pasta vigente na primeira carga do modelo — o processo só carrega uma vez.
6. **DOC-03 derrubava o lote inteiro.** Um arquivo inválido abortava a importação e os já copiados sumiam do retorno. `import_documents` devolve `ImportResult { imported, rejected }` e a aba Documentos lista os recusados com o motivo.

**Reason:** O usuário pediu "veja minhas specs e avalie o código para ver se foi tudo implementado" e mandou corrigir o que a auditoria achou.
**Trade-off/Notas:**
- Enviar em A, trocar para B e enviar em B faz o parcial de A parar de ser exibido (só um `streamingChatId` por vez); o texto não se perde — o backend persiste e o `selectChat` recarrega. Um mapa por chat resolveria, e não pareceu justificar o estado extra.
- `ImportResult` mudou a assinatura de `import_documents`; `documentsApi` e a store acompanharam.
**Impact:** Nada nas specs mudou de status — os requisitos já estavam marcados como implementados e agora de fato estão. Segue pendente tudo que exige clicar na UI.

### AD-026: M4 (chat-messaging) implementado — 12/12 tasks (2026-07-25)

**Decision:** Executado o `tasks.md` completo. `ProviderClient` ganhou `stream_chat`; Ollama usa NDJSON próprio e LM Studio/custom/embedded compartilham **um** parser SSE (`providers/openai_stream.rs`) em vez de três cópias. `chat_commands::send_message` persiste a mensagem, ingere anexos, monta contexto e emite `chat-stream-chunk`; `CancellationRegistry` para o loop entre tokens.
**Reason:** Último item da fila de Todos; o usuário pediu "executa specs que falta e depois valide".
**Trade-off/Notas:**
- Anexo pequeno (≤8000 chars) entra inteiro no prompt; acima disso reusa `rag::pipeline::process_document` com `namespace = "chat:<id>"` (AD-017), **aguardado** antes de responder, porque a pergunta atual é justamente a que precisa dele.
- O pipeline registra estado na tabela `documents`; o anexo grande cria uma linha temporária ali, que é removida ao fim — `chat_attachments` é o registro definitivo. Não estava no design, é a consequência de reusar o pipeline.
- Cancelamento e erro de provedor **preservam o parcial**: o usuário fica com o que já viu na tela.
- Orçamento de contexto trunca a categoria que estoura em vez de descartá-la (CHAT-15), e a pergunta atual nunca é truncada.
- **Não verificado**: enviar mensagem de verdade pela UI, perguntar sobre um anexo, confirmar isolamento entre chats e a limpeza do `tmp/` ao excluir o chat (T12).

### AD-025: M5 (documents-rag) implementado — 11/11 tasks (2026-07-25)

**Decision:** Executado o `tasks.md` completo. `rag/` novo (`parsing`, `chunking`, `embedding`, `store`, `pipeline`, `onnxruntime`), `document_commands.rs`, `DocumentsPanel` e reenfileiramento no boot.
**Reason:** Pré-requisito real do M4 (AD-017).
**Pesquisa obrigatória cumprida (crates confirmados na crates.io no dia):** `pdf-extract` 0.12, `docx-rs` 0.4.22 (o `dotext` do design foi **rejeitado** — último release de 2017), `fastembed` 5.17 com `MultilingualE5Small` (a UI é EN+PT, modelo só-inglês recupera mal português), `lancedb` 0.31.
**Dois bloqueios de ambiente resolvidos:**
- `lancedb` exige o compilador **protoc** no build. Instalado via `winget install Google.Protobuf` (35.1) com aprovação do usuário — vira pré-requisito de build documentado.
- O ONNX Runtime estático do `fastembed` exige a STL do MSVC 2022; a máquina só tem VS 2019 Build Tools. Escolha do usuário: `ort-load-dynamic` + download do `onnxruntime.dll` em runtime (`rag/onnxruntime.rs`), mesmo padrão do sidecar llama.cpp.
**Bug real encontrado na validação:** dois documentos indexando ao mesmo tempo (DOC-07 permite explicitamente) inicializavam o modelo em paralelo e corrompiam o cache (`Failed to retrieve onnx/model.onnx`). Corrigido serializando a init com double-check.
**Verificação real (não só compilação):** embeddings via ONNX Runtime de verdade — paráfrase 0,957 vs texto não relacionado 0,826; pergunta em PT casa com passagem em EN 0,774 vs 0,683 (justifica o modelo multilíngue). LanceDB em disco: namespace do chat não vê o global, `delete_namespace` e `delete_by_doc` removem só o alvo, busca em base vazia devolve lista vazia. Banco real do usuário migrado até `user_version = 5` sem perder dados.
**Não verificado:** importar um documento clicando na UI e ver o progresso até "ready".

### AD-024: M7 (embedded-runtime) implementado — 16/16 tasks (2026-07-25)

**Decision:** Executado o `tasks.md` completo de `embedded-runtime` (T1-T16). Módulo `runtime/` novo (`release`, `download`, `detect`, `model`, `process`, `store`), `providers::embedded::EmbeddedClient`, `embedded_commands.rs`, conexão `embedded` semeada sempre, e `EmbeddedRuntimeCard` na aba Conexões.
**Reason:** Segundo item da fila, confirmado pelo usuário no escopo "M3.1 + M7".
**Verificação real (não só compilação):**
- Release `b10107` resolvido ao vivo pela API do GitHub; os 4 sufixos que o `pick_asset` casa existem de fato no release.
- URL do GGUF do Phi-3.5 (única incerteza declarada do design) confirmada: 200 + `content-length` 2.393.232.672 (~2,39 GB).
- Binário Vulkan baixado e extraído; `llama-server --list-devices` respondeu `Vulkan0: NVIDIA GeForce RTX 3060 (12329 MiB, 11550 MiB free)` — formato idêntico ao que o `classify_output` parseia.
- Sidecar subido com as flags exatas que o app monta (`-m`, `--host 127.0.0.1`, `--port`, `-ngl -1`): `/health` devolveu `{"status":"ok"}`, `/v1/models` listou o modelo (o que o `CustomClient` parseia) e `/v1/chat/completions` gerou resposta. 152 tok/s de geração e 498 tok/s de prompt confirmam que o offload de GPU funcionou (a 1ª chamada é lenta por compilação de pipeline Vulkan — não confundir com CPU).
**Trade-off/Notas:**
- `runtime/store.rs` não estava no design: a linha singleton `embedded_runtime` é lida tanto pelo comando quanto pelo autostart do boot, então o SQL ficou num módulo só (SPEC_DEVIATION no commit).
- `ConnectionManager` deixou de ser unit struct e passou a carregar um `EmbeddedContext` (porta + models_dir), porque a URL da conexão embutida só existe depois que o processo escolhe a porta. Todos os comandos que criam provider passaram a receber `AppHandle` e a construir o manager via `embedded_commands::manager`.
- T2 pedia baixar o timeout do client de 5s para 2s; feito **por requisição de health check**, porque o mesmo client serve downloads de vários GB.
- **Não verificado**: setup disparado pelo card na UI, fechar o app e confirmar que o `llama-server` sumiu, e reabrir com a conexão ativa para ver o autostart. O mecanismo (`RunEvent::ExitRequested` → `kill`) está no código e o `kill` também roda no `Drop`, mas nenhum dos três foi exercitado clicando.
**Correção pós-auditoria (mesmo dia):** uma revisão requisito-a-requisito encontrou dois itens marcados como prontos que não estavam:
- **EMBED-12 estava quebrado**: `configure_model` gravava contexto/GPU em `model_configs`, mas o sidecar inicia a partir da linha `embedded_runtime` — a configuração era persistida e ignorada. Corrigido: o provider embutido grava também na própria linha e reinicia o servidor se estiver rodando. Offload de GPU é tudo-ou-nada (`-ngl` quer contagem de camadas, que não dá pra saber sem ler o GGUF; fração vira "off", nunca "max" silencioso).
- **EMBED-04 AC4 incompleto**: o setup terminava em "pronto para iniciar" e exigia um segundo clique. Agora sobe o sidecar ao fim da instalação.
- **Desvio consciente mantido (EMBED-02)**: o AC diz que *ativar* a conexão dispara o download; a UI exige clique explícito em "Baixar e instalar", porque ativar por rádio não deveria começar um download de 2,4 GB. Se o comportamento literal for desejado, é uma mudança pequena no card.
**Impact:** M7 ✅ no ROADMAP; C-01, C-02 e C-07 do CONCERNS.md resolvidos; C-05 parcialmente (este é o primeiro provider exercitado contra um servidor real).

### AD-023: M3.1 (single-active-connection) implementado — 10/10 tasks (2026-07-25)

**Decision:** Executado o `tasks.md` completo de `single-active-connection` (T1-T10). `db.rs` passou a aplicar migrações versionadas por `PRAGMA user_version` (migração 1 = schema antigo, migração 2 = `enabled` → `is_active` + normalização); `toggle_connection` saiu do backend, do `lib.rs`, da API e da store; `get_active_model` virou `get_active_pair`; a UI trocou checkbox por radio exclusivo e passou a listar modelos de toda conexão disponível.
**Reason:** Primeiro item da fila de Todos, confirmado pelo usuário como escopo "M3.1 + M7".
**Trade-off/Notas:**
- **T3 e T4 num só commit**: o gate de T3 é `cargo test connections::`, que não passa enquanto `connection_commands.rs` ainda chama a função removida — os callers tiveram que ir junto (SPEC_DEVIATION registrada no commit).
- `create_connection` perdeu o parâmetro `enabled`: mantê-lo permitiria criar uma segunda conexão ativa e furar justamente a invariante da feature. Ativação agora só existe via `set_active_connection`/`set_active_model`.
- `set_active_connection` foi partida em `apply_active_connection` (sem transação própria) + wrapper: o SQLite rejeita `BEGIN` aninhado e `set_active_model` já abre a sua para ativar o par atomicamente.
- `list_installed_models` já aceitava qualquer `connection_id` e não exigia conexão ativa — ACTIVE-08 não precisou de mudança no backend, só na store/UI.
- **Não verificado**: a UI não foi exercitada clicando (ativar Ollama → ativar LM Studio → trocar modelo). O app sobe (`Finished` + `Running`) e o build é limpo, mas o fluxo visual continua na lista de Todos.
**Impact:** M3.1 ✅ no ROADMAP; `single-active-connection/{spec,tasks}.md` marcados; AD-016 revogada também no `chat-messaging/{design,tasks}.md` (T10). C-01 do CONCERNS.md resolvido.

### AD-022: Runtime embutido usa Vulkan (não CUDA), e o próprio binário detecta a GPU (2026-07-25)

**Decision:** O M7 baixa o build **Vulkan** do llama.cpp (mais o build CPU só como fallback se o Vulkan nem executar). CUDA fica fora. A detecção de GPU é feita rodando `llama-server --list-devices` e lendo a saída — sem `wgpu`, `ash` ou `nvml`.
**Reason:** Um binário Vulkan cobre NVIDIA/AMD/Intel sem exigir toolkit instalado. CUDA obrigaria escolher entre `cuda-12.4` e `cuda-13.3`, casar com versão de driver e dobrar a matriz de download. Sobre a detecção: o binário já sabe responder a pergunta, e uma lib de GPU responderia "existe Vulkan", não "o llama.cpp consegue usar".
**Trade-off:** Usuário com NVIDIA de ponta perde ~35% em prompt processing (benchmarks 2026: RTX 5090 ~14.073 vs ~10.382 pp512); geração de token fica praticamente empatada (290 vs 264 tg128). Registrado como Deferred Idea.
**Impact:** `embedded-runtime/design.md` (Tech Decisions) e tasks T4/T6.

### AD-021: Uma conexão ativa e um modelo ativo, globais — revoga a AD-016 (2026-07-25)

**Decision:** Existe no máximo **uma** conexão ativa e **um** modelo ativo no app inteiro, e o modelo ativo sempre pertence à conexão ativa. Escolher um modelo ativa a conexão dona dele na mesma ação. Conexões inativas continuam listadas com status e com modelos inspecionáveis.
**Reason:** Pedido literal do usuário — *"conexão e modelo deve ter somente um único ativo, que é ele que deve ser usado na hora do chat"*. O M3 tinha deixado uma assimetria: modelo já era único, mas `connections.enabled` permitia várias habilitadas, sem resposta para "qual delas responde?".
**Trade-off:** **Revoga a AD-016** (modelo por chat com fallback global) — perguntado explicitamente, o usuário escolheu matar o override por chat. Perde-se flexibilidade de usar modelos diferentes em chats diferentes; ganha-se um modelo mental sem ambiguidade.
**Impact:** `connections.enabled` vira `is_active`; `toggle_connection` sai e entram `set_active_connection`/`clear_active_connection`; `get_active_model` vira `get_active_pair`. `chat-messaging/design.md` precisa perder `chats.model_config_id` (task T10 de `single-active-connection`).

### AD-020: Migração de schema versionada com `PRAGMA user_version` (2026-07-25)

**Decision:** `db.rs` passa de um `execute_batch(SCHEMA)` único para uma lista ordenada de migrações aplicadas conforme o `PRAGMA user_version`, cada uma em transação.
**Reason:** O schema atual é só `CREATE TABLE IF NOT EXISTS` — funciona para adicionar tabela, mas vira **no-op silencioso** para mudança de coluna em banco já existente (C-01 no CONCERNS.md). A AD-021 precisa justamente renomear `connections.enabled`, e o M7 adiciona `embedded_runtime` — as duas próximas features batem nesse limite.
**Trade-off:** ~40 linhas de infra a mais; nenhuma dependência nova (é recurso nativo do SQLite).
**Impact:** `single-active-connection` T1/T2; `embedded-runtime` T3 entra como migração 3.

---

## Recent Decisions (Last 60 days)

### AD-019: M3 (connections-models) implementado — 15/15 tasks (2026-07-25)

**Decision:** Executado o `tasks.md` completo de `connections-models` (T1-T15), do zero até `ConnectionsPanel` funcional no `App.tsx`. Repositório git inicializado nesta sessão (não existia antes) especificamente para viabilizar 1 commit atômico por task.
**Reason:** Próximo passo registrado em Todos desta mesma STATE.md; usuário confirmou escopo "feature inteira T1-T15 autônomo" e "inicializar git agora" via pergunta direta no início da sessão.
**Trade-off/Notas:**
- Nenhum Ollama/LM Studio rodando neste ambiente — `OllamaClient`/`LmStudioClient`/`CustomClient` foram verificados por `cargo check`/`cargo test` e pelos payloads exatos documentados (pesquisa web durante T5/T6), não por chamada real a um servidor. Endpoint fields para LM Studio (`context_length` snake_case, `offload_kv_cache_to_gpu` boolean) divergiam do que o `design.md` original supunha (`contextLength`/`gpuOffload` camelCase graduado) — corrigido, documentado como SPEC_DEVIATION no código.
- `models.rs` virou `models/mod.rs` (Chat/Message preservados) para caber `models::catalog`/`models::memory_estimate` no path exato do design.
- `tasks.md` tinha algumas lacunas de integração não explícitas em nenhuma task individual, preenchidas durante a execução (todas com nota SPEC_DEVIATION no commit correspondente): provider "custom" sem client (`providers::custom::CustomClient`, T7), `set_active_model`/`configure_model` descritos como recebendo um `model_config_id` que nem sempre existe ainda (resolvido com find-or-create por `connection_id`+`model_name`, T9), nenhum getter para "qual é o modelo ativo" (`get_active_model`, addendum pós-T9), e nenhuma task listada para colocar `ModelsList`/`ModelConfigForm` dentro do `ConnectionsPanel` (feito nos próprios commits de T13/T14, já que design.md só permite um lugar pra isso).
**Impact:** M3 completo no ROADMAP; `connections-models/tasks.md` e `spec.md` atualizados (checkboxes + tabela de rastreabilidade). 9 commits no backend Rust, 6 no frontend React, todos atômicos.

---

## Recent Decisions (Last 60 days)

### AD-018: Streaming de chat via evento Tauri, não retorno de comando (2026-07-25)

**Decision:** `send_message` retorna o `message_id` do usuário imediatamente; os tokens da resposta chegam via evento (`chat-stream-chunk`), não como retorno do comando.
**Reason:** Comandos Tauri são request/response; token-a-token exige push. Mesmo padrão já usado para progresso de download (M3) e indexação de documentos (M5) — consistência entre as três features planejadas nesta sessão.
**Trade-off:** Frontend precisa gerenciar estado de "mensagem sendo montada" via listener, não só via return de promise.
**Impact:** `chat-messaging/design.md` define `ChatStreamChunk`; `CancellationRegistry` (por `chat_id`) permite parar no meio.

### AD-017: RAG com namespace único reusado por Documentos e Chat (2026-07-25)

**Decision:** Uma única abstração `VectorStore` (LanceDB, coluna `namespace`) atende tanto a base global (`namespace="global"`, M5) quanto os anexos por chat (`namespace="chat:<id>"`, M4) — mesmo código, sem duplicar orquestração de parse→chunk→embed→store.
**Reason:** `chat-messaging` (M4) precisava do mesmíssimo pipeline de `documents-rag` (M5), só trocando o namespace; construir dois pipelines seria retrabalho e um risco de os dois divergirem.
**Trade-off:** `chat-messaging` tem dependência de implementação direta em `documents-rag` (não só de arquitetura) — a ordem de execução importa de verdade, não é só preferência de roadmap.
**Impact:** `tasks.md` de `chat-messaging` referencia tasks de `documents-rag` explicitamente como "Externo" nas dependências (T5, T6 dependem de documents-rag T3/T4/T5/T6).

### AD-016: ~~Modelo ativo é por chat, com fallback pro modelo global~~ — **REVOGADA em 2026-07-25 pela AD-021**

> Revogada no mesmo dia em que foi escrita, antes de qualquer código depender dela. O usuário decidiu que existe um único par ativo global (conexão + modelo) e que não há override por chat. `chats.model_config_id` **não deve ser implementado**. Texto original preservado abaixo apenas como histórico da decisão.



**Decision:** `chats.model_config_id` (nullable) — quando `NULL`, usa o "modelo ativo" marcado globalmente em Conexões (`model_configs.is_active`).
**Reason:** O spec de `connections-models` fala em "modelo ativo" (singular), mas o ROADMAP original (antes desta sessão) já previa "seleção de modelo por chat". O fallback satisfaz os dois sem contradição.
**Trade-off:** Nenhum real — é estritamente mais flexível que só-global.
**Impact:** Fechado no design de `chat-messaging`, não no de `connections-models` (que só define o conceito de "modelo ativo global").

### AD-015: Catálogo de modelos para download é curado, não uma API de catálogo (2026-07-25)

**Decision:** Nem Ollama nem LM Studio expõem API pública para listar "todos os modelos disponíveis para baixar" com tamanho (confirmado via pesquisa web nesta sessão). v1 usa uma lista curada embutida (JSON/const Rust com modelos populares publicamente conhecidos: Llama 3.1 8B, Qwen2.5 7B, Phi-3 mini, etc.) + campo de pull manual por nome (Ollama) ou link Hugging Face (LM Studio) para qualquer coisa fora da lista.
**Reason:** Sem essa decisão, "filtrar modelos para download por memória" (pedido do usuário) seria impossível de implementar de forma alguma — não há de onde vir a lista.
**Trade-off:** A lista curada precisa de manutenção manual ao longo do tempo (novos modelos populares não entram sozinhos); RAM estimada usa fórmula (`params × bytes/peso × 1.2`), rotulada como estimativa na UI, não medição real.
**Impact:** `connections-models/design.md` (`ModelCatalog`) e `tasks.md` T3. Pesquisa confirmou também que LM Studio TEM API de download nativa (`/api/v1/*`, LM Studio ≥0.4.0) — corrige uma suposição errada registrada antes nesta sessão (ver Todos removidos).

### AD-014: Padrão nav+painel para Configurações (2026-07-24)

**Decision:** A seção Configurações na sidebar virou só um item de navegação (ícone + label); os campos (tema, idioma, pasta) saíram do bloco inline da sidebar e passaram a um painel de tela cheia à direita (`SettingsPanel`), substituindo o `ChatPanel` enquanto ativo. Roteamento local via `uiStore` (`activeView: 'chat' | 'settings'`).
**Reason:** Pedido do usuário — a sidebar deve ter "somente a navegação"; os campos aparecem do lado direito ao clicar.
**Trade-off:** Precisa resetar `activeView` para `'chat'` ao criar/selecionar um chat (senão o usuário fica preso na tela de Configurações vendo a lista mudar atrás). Feito em `ChatList.handleCreateChat/handleSelectChat`.
**Impact:** Estabelece o padrão que Documentos e Conexões provavelmente vão seguir quando ganharem conteúdo real (M3/M5) — hoje eles continuam como blocos inline simples (placeholders), a decisão de convertê-los para nav+painel fica para quando tiverem campos de verdade.

### AD-013: 4º tema, paleta creme/terracota — **renomeado para `terracotta` em 2026-07-26** (2026-07-24)

> **Renomeado a pedido do usuário.** O id passou de `claude` para `terracotta` e os rótulos para "Terracotta"/"Terracota". A paleta é a mesma. Quem já tinha o tema antigo salvo (em `config.json` ou no `localStorage`) é migrado por `normalizeTheme`, e a config é regravada no primeiro boot para a migração não rodar de novo — descartar o id antigo teria parecido que o app esqueceu a escolha do usuário.

**Decision:** Adicionado um 4º tema (`claude`, hoje `terracotta`) — paleta creme/terracota usando `#da7756` como accent, fundo `#faf9f5`/`#ede9de`.
**Reason:** Pedido explícito do usuário.
**Trade-off:** Só o accent color é confirmado como "oficial"; os tons de fundo creme são uma composição razoável em torno dele, não uma cópia pixel-a-pixel da paleta completa da Anthropic (não tive acesso a um guia de marca oficial completo).
**Impact:** `SUPPORTED_THEMES` agora tem 4 valores; todo `Record<Theme, string>` (Wizard, SettingsPanel) precisa mapear as 4 chaves — TypeScript já força isso via erro de compilação se esquecer.

### AD-012: Config bootstrap fica fora da pasta-base configurável (2026-07-24)

**Decision:** Um arquivo pequeno `config.json` (base_path, theme, language, onboarding_completed) vive no `app_config_dir` padrão do SO (via Tauri), não dentro da pasta-base escolhida pelo usuário. A pasta-base guarda só os dados reais (`localmind.db`, `models/`, `documents/`, `vectors/`, `chats/`).
**Reason:** Ovo-e-galinha: o app precisa saber *onde* está a pasta-base antes de conseguir ler qualquer coisa de dentro dela. Um ponteiro fixo fora da pasta resolve isso e permite trocar a pasta-base livremente depois.
**Trade-off:** Duas localizações de config para o usuário entender (a pasta padrão do SO guarda só o ponteiro; a pasta escolhida guarda os dados). Documentado no README/spec.
**Impact:** `config.rs` implementa `bootstrap_file_path()` separado de `base_path`; `update_base_path` só move o `localmind.db`, nunca o bootstrap.

### AD-011: DbState vira `Mutex<Option<Connection>>` (2026-07-24)

**Decision:** O estado do SQLite no Tauri passou de `Mutex<Connection>` (M1) para `Mutex<Option<Connection>>`, já que agora o banco só existe depois que o usuário completa o wizard (ou quando `update_base_path` remonta a conexão).
**Reason:** Antes do wizard não existe pasta-base, logo não existe onde abrir o `.db`. Comandos de chat agora retornam erro amigável se chamados antes da configuração.
**Trade-off:** Todo comando de chat precisa de um `require_conn`/checagem de `None` a mais.
**Impact:** `commands.rs` (chat) e `config_commands.rs` (onboarding/troca de pasta) compartilham esse padrão; App.tsx só renderiza a Sidebar/ChatPanel depois que `status === "ready"` no configStore, então `list_chats` nunca é chamado com DB ausente na prática.

### AD-001: Framework desktop = Tauri 2 (2026-07-24)

**Decision:** Usar Tauri 2 (Rust + webview) em vez de Electron.
**Reason:** Instalador pequeno (~10-15MB), menor uso de RAM, e backend Rust permite rodar embeddings/banco vetorial nativos.
**Trade-off:** Curva de aprendizado de Rust no backend; menos libs de RAG prontas que no ecossistema JS.
**Impact:** RAG (fastembed, LanceDB) implementado em Rust; frontend em React consome comandos Tauri.

### AD-002: Estratégia de LLM = conectar + runtime embutido (2026-07-24)

**Decision:** Detectar/conectar a Ollama e LM Studio via API OpenAI-compatible E embutir llama.cpp como fallback.
**Reason:** Alinha com "tudo necessário ou comunicação com o necessário"; funciona do zero sem pré-requisitos.
**Trade-off:** Instalador maior e mais complexidade de empacotamento (sidecar por plataforma).
**Impact:** Connection Manager (M2) abstrai runtimes; sidecar llama.cpp isolado em M5 para não travar o MVP.

### AD-003: RAG = embeddings embutidos + vetor local (2026-07-24)

**Decision:** Embeddings com fastembed (ONNX, ex. bge-small) + banco vetorial LanceDB, ambos embutidos.
**Reason:** Indexação 100% offline, sem depender de Ollama estar rodando; nativo em Rust cabe no bundle.
**Trade-off:** Modelo de embedding adiciona ~100-150MB ao bundle.
**Impact:** Ingestão de documentos (M3) independe das conexões de LLM.

### AD-004: Modelo de contexto = chat isolado + docs globais (2026-07-24)

**Decision:** Cada chat é único e isolado (histórico + docs anexados só valem naquele chat). Só a base de documentos é global e compartilhada.
**Reason:** Requisito explícito do usuário — "cada chat é único, somente os documentos são globais".
**Trade-off:** Duas tabelas/namespaces vetoriais (global + por `chat_id`) e lógica de retrieval combinada.
**Impact:** Define o schema (M1) e a arquitetura de RAG em duas camadas (M3 global, M4 por chat).

### AD-010: Config inicial via wizard de primeiro uso, não no instalador (2026-07-24)

**Decision:** Caminho de armazenamento, tema e idioma são definidos por um wizard na 1ª abertura do app (e editáveis depois em Configurações), não durante a instalação.
**Reason:** Instaladores Tauri não suportam config interativa de forma confiável/cross-platform — AppImage (Linux) é portátil sem etapa de instalação, `.deb` instala sem interação; customizar NSIS (Windows) é frágil e inconsistente entre SOs.
**Trade-off:** Configuração acontece 1 clique depois de abrir, não "antes de concluir a instalação" como pedido originalmente.
**Impact:** M2 entrega o wizard de primeiro uso; página customizada no NSIS Windows fica como ideia futura (deferida). Instaladores permanecem padrão/simples em M8.

### AD-009: Memória de conversa = RAG híbrido (recentes verbatim + retrieval do histórico) (2026-07-24)

**Decision:** Contexto de cada mensagem = system prompt + últimas N mensagens verbatim + top-k turnos antigos relevantes (recuperados por embedding) + RAG docs globais + RAG docs do chat/anexos.
**Reason:** Requisito do usuário — conversa serializada funcionando "como memória"; híbrido preserva continuidade imediata E memória de longo prazo além do limite de contexto.
**Trade-off:** Cada turno é embeddado e armazenado num namespace vetorial da conversa (`chat_id`), somando custo/armazenamento.
**Impact:** M6 implementa; reusa o embedding engine do M5. Define 3 camadas de RAG: global (docs), chat (anexos), conversa (memória).

### AD-008: Layout de armazenamento configurável (2026-07-24)

**Decision:** Uma pasta-base escolhida pelo usuário contém `models/`, `documents/`, `vectors/` (LanceDB), `localmind.db` (SQLite) e `chats/<id>/tmp/` para anexos temporários de chat. Anexos de chat são apagados quando o chat é excluído.
**Reason:** Usuário quer escolher onde modelos e documentos ficam; anexos de chat são efêmeros e atrelados ao ciclo de vida do chat.
**Trade-off:** App precisa gerenciar caminhos configuráveis (não só `app_data_dir` do Tauri) e migrar/validar a pasta ao trocar.
**Impact:** M2 define o storage manager e persiste a pasta-base; M4 grava anexos em `chats/<id>/tmp/` e os remove no delete do chat (estende a lógica atual de `delete_chat`).

### AD-007: i18n (EN padrão + PT) e temas múltiplos (2026-07-24)

**Decision:** Interface internacionalizada com inglês como idioma padrão e português disponível; sistema de temas com claro, escuro e temas de cor extras via CSS variables.
**Reason:** Requisito explícito do usuário.
**Trade-off:** Todas as strings de UI precisam passar por camada i18n desde já (retrofit é caro).
**Impact:** M2 introduz i18n (ex.: i18next) e o theme system; textos em PT já escritos na UI do M1 serão movidos para chaves de tradução (EN default).

### AD-006: Tailwind CSS v4 (2026-07-24)

**Decision:** Usar Tailwind CSS v4 (`@tailwindcss/postcss` + `@import "tailwindcss";` no CSS), não v3.
**Reason:** `npm install tailwindcss` instalou a versão atual (4.x) por padrão; v4 não usa `tailwind.config.js`/`@tailwind base/components/utilities` — é config CSS-first com detecção automática de conteúdo.
**Trade-off:** Nenhum, mas qualquer código de exemplo v3 copiado da internet não se aplica diretamente.
**Impact:** `postcss.config.js` usa `@tailwindcss/postcss`; `src/index.css` usa `@import "tailwindcss"` + bloco `@theme`. Não existe `tailwind.config.js` no projeto — é esperado, não um arquivo faltando.

### AD-005: Scaffold via create-tauri-app (2026-07-24)

**Decision:** Projeto gerado com `npx create-tauri-app@latest . -m npm -t react-ts --identifier com.localmind.app -y -f` em vez de escrever tauri.conf.json/Cargo.toml manualmente.
**Reason:** Garante config válida e compatível com a versão atual do Tauri 2 (ícones, capabilities, build.rs corretos).
**Trade-off:** Nenhum.
**Impact:** Estrutura base em `src/`, `src-tauri/`, `package.json` na raiz do projeto.

---

## Active Blockers

_Nenhum._

### B-001 (RESOLVIDO 2026-07-24): Rust toolchain não instalado

**Resolution:** Instalado via `winget install Rustlang.Rustup` (rustc/cargo 1.97.1, toolchain `stable-x86_64-pc-windows-msvc`). MSVC Build Tools já presentes (VS 2019 BuildTools). `cargo check` e `npm run tauri dev` rodaram com sucesso.
**Nota p/ Bash tool:** a PATH do tool Bash não herda o `~/.cargo/bin`; para comandos cargo via Bash, prefixe `export PATH="/c/Users/rafae/.cargo/bin:$PATH"`. Via PowerShell, use `$env:USERPROFILE\.cargo\bin`.

---

## Lessons Learned

### L-004: `tauri signer generate` produz dois blobs base64 quase idênticos, e um deles é segredo (2026-07-26)

**Context:** Fechando a T2 do M8, o mantenedor gerou o par de chaves e colou o valor em `plugins.updater.pubkey` do `tauri.conf.json`.
**Problem:** O que foi colado era o conteúdo de `localmind.key` — a chave **privada**. Os dois arquivos (`.key` e `.key.pub`) são blobs base64 de tamanho parecido, sem nada no valor que denuncie qual é qual; a diferença só aparece **depois** de decodificar (`rsign encrypted secret key` vs `minisign public key`). O `tauri.conf.json` é versionado, então o passo seguinte natural — commitar — teria colocado a chave privada no repositório. E o modo de falha funcional era igualmente tardio: nem o plugin nem o nosso `decode_pubkey` reclamam na inicialização, só em `app.updater()` ou na hora de verificar um download.
**Solution:** Pego antes de qualquer commit (`HEAD` ainda era `9cf3fe7`, arquivo só modificado no working tree) porque o valor foi decodificado antes de seguir adiante. Substituído pelo `.key.pub` de verdade e validado: 2 linhas, `minisign public key`, 42 bytes. A chave estava cifrada com senha e nunca saiu da máquina — não houve exposição e não foi preciso rotacionar.
**Prevents:** Entrou o teste `update::signature::the_configured_public_key_is_a_public_key_and_parses`, que lê o `tauri.conf.json` via `include_str!`, decodifica a `pubkey` e falha o `cargo test` se ela estiver vazia, se contiver `secret key`, ou se não parsear. A regra geral: **valor opaco em arquivo versionado se decodifica e se confere antes de commitar** — "parece a chave certa" não é verificação, e aqui o custo do engano seria um segredo publicado.

### L-001: `create-tauri-app --force` apaga o conteúdo existente do diretório (2026-07-24)

**Context:** Rodei `npx create-tauri-app@latest . -f` dentro de `D:\chat-ia-local`, que já continha `.specs/` com PROJECT.md, ROADMAP.md, STATE.md e o spec do app-shell.
**Problem:** A flag `--force` ("Force create the directory even if it is not empty") apagou a pasta `.specs/` inteira durante o scaffold.
**Solution:** Conteúdo restaurado a partir do histórico da conversa (nenhuma perda real, mas exigiu recriação manual).
**Prevents:** Nunca rodar scaffolders com flag de "force/overwrite" em diretório não vazio sem antes mover/backupar conteúdo existente para fora do diretório alvo, mesmo quando o conteúdo "não deveria" conflitar.

### L-002: M1 verificado em execução (2026-07-24)

**Context:** Após instalar o Rust, `npm run tauri dev` compilou em ~1m34s e abriu a janela (`tauri-app.exe`); `localmind.db` foi criado em `%AppData%\com.localmind.app\`.
**Problem:** Nenhum — validação de que o walking skeleton (Tauri + React + SQLite) funciona ponta a ponta.
**Solution:** SHELL-08 (init DB + migrações) confirmado pela criação do .db. Verificação visual dos fluxos de CRUD de chat (SHELL-01..07) ainda depende de clicar na UI manualmente.
**Prevents:** Regressões futuras — temos baseline de que o backend Rust compila e o app sobe nesta máquina.

---

### L-003: Uma limitação de biblioteca só é limitação depois de comparada com outra implementação (2026-07-26)

**Context:** A AD-032 registrou que o `pdf-extract` perdia letras em partes do PDF do usuário e concluiu que não havia saída: o crate já estava na versão mais recente publicada, logo "não há bump disponível".
**Problem:** O raciocínio parou na versão do crate e nunca perguntou se *outro* leitor daria o mesmo resultado. Com isso, um defeito que destruía 51,3% do corpus ficou um dia inteiro registrado como limitação aceita, e a culpa foi para o modelo. Pior: o diagnóstico "o modelo é que é fraco" é do tipo que encerra a investigação, porque não sugere nada verificável.
**Solution:** Rodar um extrator independente (`pdftotext`, do poppler, já instalado na máquina) contra o mesmo arquivo levou dois minutos e devolveu o texto perfeito — provando na hora que o PDF era legível e o problema era do crate. Só depois disso a troca por pdfium virou uma decisão óbvia em vez de uma aposta.
**Prevents:** Antes de escrever "limitação sem correção" sobre qualquer dependência, gastar os minutos de rodar uma segunda implementação no mesmo insumo. E desconfiar de diagnóstico que termina em "a ferramenta é fraca" sem um número do lado — a AD-032 não tinha medido que fração do corpus estava corrompida, e a fração era metade.

## Quick Tasks Completed

| #   | Description | Date | Commit | Status |
| --- | ----------- | ---- | ------ | ------ |
| 1   | Chat em balões com lados: mensagens do usuário à direita (cor de destaque), respostas do modelo à esquerda. Rótulo de papel saiu — o lado já diz quem falou; `system` (se algum dia for persistido) fica centralizado e discreto, e `aria-label` mantém o papel para leitor de tela | 2026-07-25 | — | Feito, verificado na UI |

---

## Deferred Ideas

- [ ] Perfis de agente reutilizáveis (persona + modelo + docs) — Captured during: planejamento inicial
- [ ] Agentes com ferramentas / tool-calling — Captured during: planejamento inicial
- [ ] Suporte a macOS — Captured during: planejamento inicial
- [ ] OCR de documentos escaneados — Captured during: planejamento inicial
- [ ] Página customizada no instalador NSIS Windows (pasta de dados durante a instalação) — Captured during: replanejamento (ver AD-010); wizard de 1º uso cobre isso no v1
- [ ] Detecção de VRAM por GPU para filtragem de modelos mais precisa — Captured during: replanejamento (M3 começa só com RAM)
- [ ] Build CUDA do llama.cpp embutido, para quem tem NVIDIA de ponta (~35% mais rápido em prompt processing que Vulkan) — Captured during: design do M7 (ver AD-022)
- [ ] Atualizar o binário do llama.cpp embutido para releases mais novos (v1 fixa o tag resolvido no primeiro download) — Captured during: spec do M7
- [ ] Code signing de verdade (certificado Authenticode / notarização) — Captured during: planejamento do M8 (AD-034); é custo e burocracia externa, não código
- [ ] Canal beta / pré-releases (`0.4.0-beta.1`) para testar o auto-update antes de soltar estável — Captured during: planejamento do M8; recusado em favor de `master` puro
- [ ] Delta updates (baixar só o diff em vez do bundle inteiro de ~226 MB) — Captured during: planejamento do M8
- [ ] Rollback para a versão anterior pela UI (o `.old` da troca portátil já dá um caminho manual de emergência) — Captured during: planejamento do M8
- [ ] `cargo clippy -D warnings` e `cargo fmt --check` no CI — Captured during: planejamento do M8; o código atual não passa hoje, então entra depois de pagar as dívidas da AD-033

---

## Todos

- [x] ~~**Lacuna do M2 encontrada na auditoria de 2026-07-25**: pasta-base que some entre sessões deixava o app abrir com todo comando quebrado~~ — **implementado em 2026-07-26** (ver AD-036): `config::evaluate_storage` + `get_storage_status` + wizard reaberto nomeando a pasta perdida. **Falta verificar clicando**: renomear a pasta-base com o app fechado, abrir, e conferir que o wizard aparece com o aviso e o caminho antigo preenchido
- [ ] Verificar manualmente na UI os fluxos de CRUD de chat do M1 (criar/renomear/excluir/persistir após reiniciar) — SHELL-01..07
- [ ] Verificar `connections-models` (M3) com Ollama e/ou LM Studio rodando de verdade nesta máquina — implementado e com `tauri dev` subindo limpo, mas `OllamaClient`/`LmStudioClient`/download real/`configure_model` nunca foram exercitados contra um servidor real (nenhum estava rodando durante a execução) — ver AD-019
- [x] ~~**1º — Executar `single-active-connection` tasks.md** (10 tasks)~~ — feito em 2026-07-25 (ver AD-023)
- [ ] Verificar manualmente na UI o fluxo do par ativo: ativar Ollama → ativar LM Studio → só a última marcada; escolher modelo da outra conexão → conexão ativa acompanha (T9 do `single-active-connection`, único item não verificado)
- [x] ~~**2º — Executar `embedded-runtime` tasks.md** (16 tasks)~~ — feito em 2026-07-25 (ver AD-024); URL do Phi-3.5 verificada ao vivo, C-07/C-02 pagos
- [x] ~~Verificar na UI o fluxo do runtime embutido (T16, EMBED-06/07)~~ — **fechado em 2026-07-25**: instalação pelo card baixou o Phi-3.5 e o Qwen2.5 1.5B; o autostart subiu o sidecar em todo reinício do `tauri dev` (`embedded runtime listening on 127.0.0.1:<porta>` no log, várias vezes); e ao encerrar o app o `tasklist` não achou nem `tauri-app.exe` nem `llama-server.exe` — inclusive com uma geração em andamento no momento do fechamento
- [x] ~~**3º — Executar `documents-rag` tasks.md** (11 tasks)~~ — feito em 2026-07-25 (ver AD-025)
- [x] ~~**4º — Executar `chat-messaging` tasks.md** (12 tasks)~~ — feito em 2026-07-25 (ver AD-026)
- [x] ~~Pesquisa obrigatória de crates/modelos em `documents-rag` T3/T4/T5~~ — feita e registrada na AD-025
- [ ] **Verificar na UI o fluxo do M5/M4**: importar um documento e ver chegar a "ready"; enviar mensagem e ver streaming; anexar um `.txt` com um fato inventado e perguntar sobre ele; repetir a pergunta em outro chat e confirmar que NÃO usa o contexto (CHAT-11); excluir o chat e confirmar que `chats/<id>/tmp/` sumiu (CHAT-12)
- [ ] **Verificar na UI as correções da AD-027**: desligar "usar meus documentos" no chat A, ir ao B e voltar (o estado tem que acompanhar cada chat); enviar no A, trocar para o B durante a resposta e confirmar que o B não mostra as mensagens do A; anexar um `.zip` (tem que ser recusado antes do envio) e um `.pdf` só com imagem (tem que virar aviso no chat); importar 2 arquivos sendo 1 inválido e confirmar que o válido entra; conferir que a resposta cita `[fonte: <arquivo>]`
- [x] ~~**Dívidas de RAG achadas na revisão da AD-033**: (a) `distance`/`chunk_index` descartados; (b) falha de retrieval invisível; (c) `RESPONSE_RESERVE_TOKENS` × `answer_token_budget`; (d) `SYSTEM_PROMPT` brigando com "continue este texto"~~ — **as quatro pagas em 2026-07-26** (ver AD-036), com 5 testes novos para o ranqueamento. **Falta medir se melhorou de verdade**: repetir a pergunta do Art. 968 contra o mesmo documento e comparar com o resultado da AD-033 — o efeito da expansão de vizinho e do piso relativo só aparece contra o corpus real
- [ ] Qualquer PDF importado **antes de 2026-07-26** está indexado com o texto corrompido do `pdf-extract` e precisa ser apagado e reimportado (o `Código Civil 2 ed.pdf` já foi)
- [ ] **Pré-requisito de build novo**: `protoc` (instalado via winget nesta máquina) é obrigatório para compilar o `lancedb` — documentar no README/STACK antes de qualquer outra pessoa clonar o repo
- [ ] O `onnxruntime.dll` é baixado em runtime na primeira indexação (~79 MB); nunca foi exercitado pelo caminho do app (só por teste com a DLL apontada à mão) — confirmar que `rag::onnxruntime::ensure_dylib` baixa e extrai certo
- [ ] Encarar os itens de `.specs/codebase/CONCERNS.md` não cobertos pelas features planejadas: C-03 (espelhamento manual de tipos Rust↔TS), C-04 (zero teste no frontend), C-06 (polling de download do LM Studio sem timeout), C-10, C-11. **C-09 saiu da lista pela metade**: o M8 trouxe CI (`ci.yml` valida build + testes + Conventional Commits), mas linter e formatter continuam fora — o código atual não passa em `clippy -D warnings`
- [x] ~~**Executar `release-distribution` tasks.md** (24 tasks, M8)~~ — **23 de 24 feitas em 2026-07-26** (ver AD-035 e a correção na AD-036). A T2 foi concluída pelo mantenedor; resta só a T24
- [ ] **Planejar o M6 (memória de conversa)** — é o único milestone sem `.specs/features/` nenhum: existe a AD-009 (decisão de RAG híbrido) e três bullets no ROADMAP, mais nada. Precisa de uma passada de Specify antes de qualquer código
- [ ] **T24 do M8 é verificação real, não build**: publicar release de verdade, instalar o `-setup.exe` numa conta **sem** direitos de administrador (zero prompts de UAC), rodar o zip portátil e confirmar que nada foi escrito em `%APPDATA%`, e aplicar uma atualização de verdade nos dois modos
- [ ] Confirmar na execução do M8 as Open Questions do design: flag `--bundles` da versão corrente do `tauri-action`; se o `tauri-plugin-updater` ignora mesmo a chave `windows-x86_64-portable` no `latest.json` (plano B: manifesto separado); comando que atualiza a versão no `Cargo.lock` sem tocar em mais nada; e os nomes exatos dos artefatos (o `patch-latest-json.mjs` deve **ler** os assets da release, não presumir os nomes)
- [ ] Avaliar assinatura de código dos instaladores (Windows) — fora do escopo do M8 por decisão (AD-034); sem certificado, o SmartScreen avisa na 1ª execução
- [ ] Depois do M1, avaliar excluir os ícones padrão do template (`Square*.png`, `StoreLogo.png`) não usados no bundle final

---

## Preferences

**Model Guidance Shown:** never
