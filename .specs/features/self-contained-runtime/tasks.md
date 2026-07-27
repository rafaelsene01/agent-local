# Runtime autossuficiente — Tasks

**Design:** `.specs/features/self-contained-runtime/design.md`
**Spec:** `.specs/features/self-contained-runtime/spec.md`
**Status:** **T1–T21 de 22 concluídas; a T22 está parcial (2026-07-27).** Os
instaladores foram **regerados** a partir da árvore corrigida da AD-046 e
medidos, e desta vez o `llama-server.exe` foi **executado a partir do zip
portátil extraído** — a conferência que faltava. Ver as medições abaixo. O que
continua aberto na T22 é o que exige uma pessoa: instalar sem rede, conversar,
importar um PDF offline e abrir o portátil como app. Gates atuais:
`cargo test` **174 passando / 0 falhas / 13 ignorados**,
`npm run test:scripts` **49**, `npm run build` limpo.

**Atualização de 2026-07-27 — o app foi aberto de novo, e desta vez o runtime
subiu.** Depois da correção da AD-046, o `npm run tauri dev` levou o autostart a
iniciar o sidecar sozinho: Phi-3.5 carregado a partir da árvore vendorizada,
`n_ctx_slot = 21760`, escutando em `127.0.0.1:53773`. Isso fecha, em ambiente de
desenvolvimento, a dúvida que a AD-046 tinha aberto — o `llama-server`
empacotado não só executa, como serve o modelo. **Continua sem ter sido testado
a partir de um app instalado, e sem a rede desligada.**

⚠️ **A release `v0.2.0` publicada não contém nada deste milestone.** A tag foi
cortada de um commit anterior ao vendoring: `git ls-tree v0.2.0 scripts/` não
tem `vendor-runtime.mjs` nem `vendor.json`, o `tauri.conf.json` da tag não tem
`bundle.resources`, `runtime/bundled.rs` não existe e o frontend ainda é
`src/components/Connections`. É por isso que o zip portátil publicado tem
**3 arquivos** (`.portable`, `LocalMind.exe`, `README.txt`) e nenhum recurso — o
que é coerente com o código daquela tag, não um defeito do empacotador. Pior:
naquela tag o backend já tinha perdido `list_connections`/`get_active_pair`/
`pull_model` (Fase 1, AD-042) enquanto o frontend ainda os chamava, então
**a release publicada é o estado quebrado em runtime que a AD-042 descreve**.
Uma release nova, a partir de `master`, é o que resolve — e disparar release é
decisão do mantenedor.

---

## Execution Log — Fase 1 (2026-07-27)

| Task | Status | Evidência |
| --- | --- | --- |
| T1 | ✅ | `providers/llama_server.rs`: um cliente concreto, sem trait. 6 testes — janela treinada vs alocada, casamento por sufixo de caminho, servidor sem `meta`, runtime parado devolvendo `Unavailable` |
| T2 | ✅ | `runtime::store` ganhou `ActiveModel`, `active_model`, `set_active_model`, `set_config`. 5 testes — inclusive que escolher modelo **não** zera contexto/GPU e que um arquivo apagado do disco lê como "sem modelo ativo", não como ativo quebrado |
| T3 | ✅ | `chat_commands` não resolve mais par ativo: pede o modelo ao runtime. `ConnectionManager`, `provider_for` e `get_active_pair` sumiram do arquivo |
| T4 | ✅ | `embedded_commands.rs` → `runtime_commands.rs` (via `git mv`, histórico preservado), com `client(&app)` no lugar de `manager(&app)`; 6 comandos novos sem `connection_id` |
| T5 | ✅ | 7 arquivos apagados; `trait ProviderClient`, `ConnectionManager` e `ConfigApplied` não existem mais; `async-trait` sem nenhum uso restante. `grep -ri "ollama\|lmstudio"` só acha comentários explicando a remoção **e o catálogo**, de onde saíram as 8 entradas que só o Ollama sabia baixar |
| T6 | ✅ | **Migração 7, não 6** — o número 6 já tinha sido gasto pela coluna `documents.namespace` (AD-040). Derruba `model_configs` antes de `connections`, porque a FK agora é aplicada de verdade |

**Verificação que importa:** a migração foi ensaiada contra uma **cópia** do banco real do usuário. `user_version` 6 → 7, `chats` 2, `messages` 6, `documents` 1 e `chat_attachments` 0 — todas preservadas; `connections` deixou de existir. O original não foi tocado.

**Testes perdidos, com justificativa (o gate da T5 exige):** 146 contra 148 antes da T6. Dois foram removidos porque o assunto deixou de existir — `fresh_database_uses_is_active_column` (a coluna estava numa tabela derrubada) e `deleting_a_connection_now_takes_its_model_configs_with_it` (provava o CASCADE de `model_configs`; a aplicação de FK continua coberta por `open_enables_foreign_keys` e pelo teste de mensagem órfã). Outros quatro foram **reescritos** em vez de apagados, para continuarem afirmando algo verdadeiro sobre o novo estado. Os de `ollama`/`lmstudio` sumiram junto com o código que testavam.

---

## Execution Log — Fases 2, 3 e 4 (2026-07-27)

| Task | Status | Evidência |
| --- | --- | --- |
| T4 (fechamento) | ✅ | A T4 tinha sido dada como concluída **com os nomes antigos** (`setup_embedded_runtime` etc.). Renomeada para o que o design manda: `prepare_runtime`, `start_runtime`, `stop_runtime`, `runtime_status`, `download_model`. Eventos: `connections-changed` → `runtime-changed`, `embedded-setup-progress` → `runtime-progress` |
| T7 | ✅ | `connections.*` virou `runtime.*`; **142 chaves em EN e 142 em PT**, conferidas por script; `grep -i "ollama\|lm studio"` nos locales volta vazio |
| T8 | ✅ | `connectionsApi.ts` → `runtimeApi.ts`; saíram `Connection`, `ConnectionProvider`, `ConnectionStatus`, `ActivePair`, `ConfigApplied`; cada função corresponde a um comando do `invoke_handler` |
| T9 | ✅ | `connectionsStore.ts` → `runtimeStore.ts`; progresso de download indexado pela URL do `.gguf` |
| T10 | ✅ | `src/components/Runtime/{RuntimePanel,RuntimeCard,ModelsList,ModelDownloadCard,ModelConfigForm}.tsx`; `src/components/Connections/` apagado |
| T11 | ✅ | `RuntimeSection.tsx`; view `connections` → `runtime`; `MessageInput` passou a olhar `activeModel` |
| T12 | ✅ | `scripts/vendor.json` + `vendor-runtime.mjs` + 11 testes. **Rodado de verdade**: 120,5 MB baixados, extraídos e podados; segunda execução é no-op pelo stamp |
| T13 | ✅ | `bundle.resources: ["resources/"]`, `beforeBuildCommand`/`beforeDevCommand` com `npm run vendor`, `cargo:rerun-if-changed=resources` no `build.rs`, `src-tauri/resources/` no `.gitignore` |
| T14 | ✅ | `runtime/bundled.rs`: `resource_root`, `find_file`, `llama_server`, `onnxruntime_dylib`, `pdfium_library`, `ensure_executable`. 6 testes (3 só compilam em Unix) |
| T15 | ✅ | `rag/pdfium.rs` perdeu `asset_url`, `RELEASE` e o download |
| T16 | ✅ | `rag/onnxruntime.rs` perdeu `asset_url`, `ORT_VERSION` e o download |
| T17 | ✅ | `choose_backend` escolhe entre os dois binários embutidos por probe local; `runtime/release.rs` apagado; `download::extract` removido; `tar` e `flate2` saíram do `Cargo.toml` |
| T18 | ✅ | `remove_legacy_downloads` no boot; teste prova que o `llama-server.log` e a pasta `models/` sobrevivem |
| T19 | ✅ | `stageBundle` extraída de `main` para poder ser testada; recursos ausentes falham o empacotamento |
| T20 | ✅ | `check-linux-bundle.mjs` + 4 testes; passo novo no `release.yml`, depois do bundling do Linux |
| T21 | ✅ | PROJECT, ROADMAP, README, INTEGRATIONS, ARCHITECTURE, STACK, STRUCTURE, CONCERNS, CONVENTIONS, TESTING |
| T22 | 🔶 | **Parcial (2026-07-27, regerada).** A metade que não exige uma pessoa está feita **e agora vale**: instaladores gerados e medidos a partir da árvore corrigida, zip portátil extraído e o `llama-server` **executado de dentro dele**. Continua aberto tudo que precisa de alguém: instalar sem rede, conversar, importar um PDF offline, baixar um modelo pelo catálogo, rodar o portátil como app |

### O que a implementação mediu (e que a spec pedia como número, não estimativa)

- **Árvore vendorizada: 120,5 MB** no Windows x64 — llama Vulkan 73,8 MB, llama CPU 23,1 MB, ONNX Runtime 16,2 MB, pdfium 7,4 MB.
- **O ONNX Runtime extrai 425,9 MB cru, e 408 MB disso é `onnxruntime.pdb`.** A poda de `.pdb`/`.lib`/`.exp`/headers foi o que trouxe o componente a 16,2 MB. O design previa risco no `lib/` inteiro (Open Question #3); o problema real era um arquivo só.
- **`tar` não é o mesmo programa em toda máquina.** Medido aqui: a partir do Git Bash, `tar -xf` num `.zip` falha com *"This does not look like a tar archive"* (é o GNU tar do MSYS, não o bsdtar do Windows). O script passou a despachar por extensão.

### Medições do build de release (regeradas em 2026-07-27, Windows x64)

> **Os números anteriores foram descartados.** O primeiro build saiu da árvore de
> vendoring quebrada (AD-046) e produzia um `llama-server.exe` que não executava.
> A tabela abaixo é de um build novo, **depois** da correção da poda.

`npm run tauri build` em **8m44s** (contra 23m37s do primeiro — o resto do
`target/` já estava quente), seguido de
`node scripts/make-portable.mjs --version 0.1.1`:

| Artefato | Agora | Antes (inservível) | Δ |
| --- | --- | --- | --- |
| `LocalMind_0.1.1_x64-setup.exe` (NSIS) | **53,3 MiB** | 47,6 MiB | +5,7 |
| `LocalMind_0.1.1_x64_en-US.msi` | **91,4 MiB** | 83,8 MiB | +7,6 |
| `LocalMind_0.1.1_x64-portable.zip` | **107,2 MiB** | 92,0 MiB | +15,2 |
| `LocalMind.exe` | **159,2 MiB** | 159,2 MiB | 0 |
| `resources/` em `target/release/` | **150 MB, 83 arquivos** | 115,0 MiB, 79 arquivos | +4 arquivos |

**Os +4 arquivos são a prova aritmética da correção:** `llama-server-impl.dll` e
`llama-common.dll`, uma vez para cada backend (Vulkan e CPU). O `LocalMind.exe`
não mudou um byte, o que confirma que o crescimento está inteiro nos recursos e
não no binário.

**Desta vez o bundle foi executado, não só inspecionado.** É a diferença que a
AD-046 cobrou do SELF-16: o zip portátil foi extraído numa pasta limpa
(86 arquivos, `.portable`, `README.txt`, `LocalMind.exe`) e o `llama-server.exe`
**de dentro dele** foi rodado:

```
resources/llama/vulkan/llama-server.exe --list-devices
  Vulkan0: NVIDIA GeForce RTX 3060 (12329 MiB, 11548 MiB free)     exit 0
resources/llama/cpu/llama-server.exe --list-devices
  Available devices:                                                exit 0
```

Os tamanhos no bundle extraído mostram por que conferir o nome do arquivo nunca
bastou: `llama-server.exe` tem **9.216 bytes** e `llama-server-impl.dll`,
**9.898.496**.

Isso responde as Open Questions #2 e #3 do design com número medido. **O
instalador ficou muito abaixo do teto de ~450 MB** que dispararia uma poda mais
agressiva do ONNX Runtime — o payload comprime bem porque é código.

**A assinatura falhou, e é o esperado:** `A public key has been found, but no private key`. A chave privada é segredo do mantenedor e mora nos secrets do GitHub (T2 do M8, AD-035); nenhum agente a tem. Os bundles foram gerados; só os `.sig` não.

**Ainda não medido:** o **delta** em relação à versão anterior, que exigiria o instalador da v0.1.1 publicada para comparar, e **todos os números do Linux**, que exigem o runner do CI.

### Desvios do plano, com o motivo

1. **`prepare_runtime` deixou de baixar um modelo.** O plano herdado do M7 baixava o Phi-3.5 (2,4 GB) dentro do preparo. Isso torna o alvo do M9 — *"numa máquina sem rede, com um `.gguf` já na pasta de modelos: instalar → abrir → escolher o modelo → conversar"* — impossível de cumprir. O estágio `NoModel` foi criado para nomear o estado resultante, que é o normal de uma instalação nova.
2. **Os estágios do runtime mudaram de nome.** `DownloadingBinary`/`DownloadingModel` saíram; entraram `NotPrepared`, `Preparing` e `NoModel`. O progresso de download de modelo passou a ter canal próprio (`model-download-progress`), separado do preparo do motor.
3. **O campo `provider` saiu do catálogo de modelos.** Com um runtime só, ele não distinguia nada. O teste que filtrava por ele passou a valer para todas as entradas.
4. **A migração é a 7, não a 6** (registrado na AD-042).

### Testes perdidos, com justificativa

De 150 para 150 no total, mas com movimentação: **-5** de `runtime/release.rs` (o arquivo saiu junto com a API do GitHub), **-2** de `download::extract` (a extração migrou para o script de vendoring), **-1** de `pdfium::asset_url` (não há mais URL a montar). **+8** novos: 6 em `runtime::bundled`, 1 em `rag::onnxruntime`, 1 em `rag::pdfium`. Nos scripts: 27 → 43.

**Regra de ouro desta feature:** cada task compila e passa nos testes sozinha. Por isso a ordem é **criar → migrar chamadores → apagar**, nunca "apagar e consertar depois" — foi o que forçou T3+T4 num commit só na AD-023, e aqui a superfície removida é muito maior.

---

## Execution Plan

### Fase 1 — Backend: colapso do multi-provider (T1–T6)

```
T1 [P] ─┬─→ T3 ─┐
        │       ├─→ T5 ──→ T6
T2 [P] ─┴─→ T4 ─┘

(T3 e T4 dependem cada uma de T1 e T2; T5 depende de T3 e T4)
```

`T1` e `T2` são [P] (arquivos diferentes, nada em comum). `T3` e `T4` só entram quando os dois existirem. `T5` apaga o que ficou sem chamador; `T6` derruba as tabelas depois que ninguém mais as lê.

### Fase 2 — Frontend (T7–T11)

```
T7 ──────────────────┐
                     ├──→ T10 [P]
T4 ──→ T8 ──→ T9 ────┤
                     └──→ T11 [P]

(T10 e T11 dependem, cada uma, de T7 e de T9)
```

### Fase 3 — Vendoring e componentes embutidos (T12–T18)

```
                        ┌─→ T15 [P] ─┐
T12 ──→ T13 ──→ T14 ────┼─→ T16 [P] ─┼──→ T18
                        └─→ T17 ─────┘
                             ↑
                            T4
```

### Fase 4 — Distribuição, docs e verificação (T19–T22)

```
T13 ──→ T19 ─┐
             │
T14,T17 → T20┼──→ T22
             │
T5,T11,T17 → T21
```

---

## Task Breakdown

### T1: Criar o cliente único do llama-server [P]

**What:** Uma struct concreta `LlamaServerClient` que fala HTTP com o sidecar, fundindo `CustomClient` e `EmbeddedClient` sem trait e sem `Box<dyn>`.
**Where:** `src-tauri/src/providers/llama_server.rs` (novo)
**Depends on:** Nenhuma
**Reuses:** `providers/custom.rs` (corpo quase inteiro), `providers/embedded.rs` (`list_installed_models` lendo `.gguf` do disco, `gpu_layers_for`), `providers/openai_stream.rs`, `providers::http_client`
**Requirement:** SELF-03

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] `LlamaServerClient::new(port)` e os métodos `health_check`, `list_installed_models`, `model_limits`, `stream_chat` existem sem trait
- [ ] `list_installed_models` lê os `.gguf` da pasta de modelos (nome + tamanho), não o `/v1/models`
- [ ] `model_limits` continua lendo `meta.n_ctx_train` / `meta.n_ctx` (AD-029)
- [ ] Nada é removido nesta task — os módulos antigos continuam compilando
- [ ] Gate check passa: `cd src-tauri && cargo test providers::`
- [ ] Contagem de testes: os testes de `custom`/`embedded`/`openai_stream` continuam passando **mais** os novos do cliente (mínimo 3: health, limits, listagem de modelos)

**Tests:** unit · **Gate:** quick
**Commit:** `feat(providers): add single llama-server client`

---

### T2: Fazer do `embedded_runtime` a única fonte do modelo ativo [P]

**What:** `runtime::store` ganha os acessores que hoje moram em `model_configs`: ler/gravar modelo ativo, contexto e camadas de GPU.
**Where:** `src-tauri/src/runtime/store.rs`
**Depends on:** Nenhuma
**Reuses:** `store::load` / `store::save` (a linha singleton já tem todas as colunas necessárias — nenhuma coluna nova)
**Requirement:** SELF-07, SELF-08

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] `active_model(sql) -> Result<Option<ActiveModel>, String>` devolve nome do modelo, `context_length` e `gpu_layers` a partir da linha
- [ ] `set_active_model(sql, path)` e `set_config(sql, context_length, gpu_layers)` gravam só nessa linha
- [ ] Um `model_path` que não existe mais no disco é reportado como "sem modelo ativo", não como um ativo quebrado
- [ ] Gate check passa: `cd src-tauri && cargo test runtime::store`
- [ ] Contagem de testes: no mínimo 4 novos (ler vazio, gravar/reler, modelo sumido, config preservada)

**Tests:** unit · **Gate:** quick
**Commit:** `feat(runtime): make embedded_runtime the single source of the active model`

---

### T3: Apontar o chat para o runtime, não para a conexão ativa

**What:** `send_message` e a montagem de contexto param de resolver `ActivePair`/`ConnectionManager` e passam a pedir modelo e cliente ao runtime.
**Where:** `src-tauri/src/chat_commands.rs`, `src-tauri/src/chat/context_assembler.rs`
**Depends on:** T1, T2
**Reuses:** `chat::context_assembler::budget_context` (só muda quem responde `model_limits`), `chat::cancellation`
**Requirement:** SELF-04, SELF-05

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] Nenhuma referência a `ConnectionManager`, `provider_for` ou `get_active_pair` sobra nos dois arquivos
- [ ] Sem modelo ativo, o envio falha com mensagem que nomeia a ação ("escolha um modelo em Runtime")
- [ ] O orçamento de contexto continua consultando o limite real do modelo (AD-033), agora via `LlamaServerClient`
- [ ] Gate check passa: `cd src-tauri && cargo test chat::`
- [ ] Contagem de testes: todos os testes de `chat::` continuam passando (nenhum deletado) + 1 novo para o erro de "sem modelo ativo"

**Tests:** unit · **Gate:** quick
**Commit:** `refactor(chat): resolve the model from the runtime instead of the active connection`

---

### T4: Superfície de comandos do runtime

**What:** `embedded_commands.rs` vira `runtime_commands.rs` com a lista de comandos do design (sem `connection_id` em nenhum), e o `lib.rs` registra só ela.
**Where:** `src-tauri/src/runtime_commands.rs` (renomeado de `embedded_commands.rs`), `src-tauri/src/lib.rs`
**Depends on:** T1, T2
**Reuses:** `embedded_commands.rs` inteiro (setup/start/stop/status, `apply_runtime_config`, `apply_active_model`, `forward_progress`), `model_commands.rs` (`list_downloadable_models`, filtro de RAM)
**Requirement:** SELF-01, SELF-02

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] Comandos expostos: `prepare_runtime`, `start_runtime`, `stop_runtime`, `runtime_status`, `list_installed_models`, `list_downloadable_models`, `download_model`, `set_active_model`, `configure_model`, `model_limits`, `get_active_model`
- [ ] `list_connections`, `add_connection`, `set_active_connection`, `clear_active_connection`, `refresh_connection_status`, `pull_model` e `get_active_pair` saem do `invoke_handler`
- [ ] `autostart_sidecar` deixa de perguntar "a conexão embutida está ativa?" e passa a perguntar "há runtime pronto e modelo ativo?"
- [ ] Gate check passa: `cd src-tauri && cargo check`
- [ ] O app ainda sobe: `npm run tauri dev` chega em `Running` sem panic

**Tests:** none (camada de comandos Tauri — matriz de cobertura diz "none") · **Gate:** build
**Commit:** `refactor(runtime): collapse connection and model commands into one runtime surface`

---

### T5: Apagar os módulos que ficaram sem chamador

**What:** Remoção física de tudo que servia o multi-provider, mais a limpeza das dependências que ficarem órfãs.
**Where:** apagar `src-tauri/src/providers/{ollama,lmstudio,custom,embedded}.rs`, `src-tauri/src/connections.rs`, `src-tauri/src/connection_commands.rs`, `src-tauri/src/model_commands.rs`; editar `src-tauri/src/providers/mod.rs` (sai o trait `ProviderClient`), `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`
**Depends on:** T3, T4
**Reuses:** —
**Requirement:** SELF-02, SELF-03

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] Os 7 arquivos listados não existem mais e nenhum `mod` os declara
- [ ] `trait ProviderClient`, `ConnectionManager`, `EmbeddedContext` e `provider_for` não existem mais
- [ ] `async-trait` sai do `Cargo.toml` se não sobrar nenhum uso (conferir com `cargo check`, não por leitura)
- [ ] `grep -ri "ollama\|lmstudio" src-tauri/src` volta vazio
- [ ] Gate check passa: `cd src-tauri && cargo check` **sem nenhum warning de código morto novo** e `cargo test`
- [ ] Contagem de testes: registrar o número final e justificar cada teste perdido (os de `ollama`/`lmstudio` somem junto com o código que testavam — é remoção legítima, não deleção silenciosa)

**Tests:** none (remoção; a cobertura vem dos módulos que ficam) · **Gate:** full
**Commit:** `refactor(providers)!: drop Ollama, LM Studio and custom connection support`

---

### T6: Migração 6 — derrubar `connections` e `model_configs`

**What:** Sétima entrada da lista de migrações versionadas, removendo as duas tabelas agora que nada as lê.
**Where:** `src-tauri/src/db.rs`
**Depends on:** T5
**Reuses:** o mecanismo de `PRAGMA user_version` da AD-020 (nenhuma infraestrutura nova)
**Requirement:** SELF-06

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] `MIGRATION_6_SINGLE_RUNTIME` derruba `model_configs` e `connections`, nessa ordem (a FK aponta para `connections`)
- [ ] Um banco `user_version = 5` com chats, mensagens, documentos, anexos e uma linha de `embedded_runtime` sobrevive inteiro
- [ ] Um banco que tinha Ollama ativo migra sem erro
- [ ] Migrar duas vezes é no-op
- [ ] Gate check passa: `cd src-tauri && cargo test db::`
- [ ] Contagem de testes: no mínimo 3 novos (preservação de dados, banco com Ollama ativo, idempotência)

**Tests:** unit · **Gate:** quick
**Commit:** `feat(db): drop the connections and model_configs tables`

---

### T7: Textos em EN e PT para o modelo mental novo

**What:** As chaves de i18n perdem conexão/provedor e ganham runtime, nos dois idiomas, com paridade.
**Where:** `src/i18n/locales/en.json`, `src/i18n/locales/pt.json`
**Depends on:** Nenhuma
**Reuses:** as chaves de `connections.embedded.*` já existentes, promovidas para `runtime.*`
**Requirement:** SELF-01, SELF-19

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] Saem `providerOllama`, `providerLmStudio`, `providerCustom`, `addManual`, `baseUrlPlaceholder`, `add`, `noActiveConnection`, `activeUnavailable`, `clearActive`, `useConnection`, `tabConnections` e o texto de `manualPullPlaceholder` que cita Ollama/LM Studio
- [ ] Entram as chaves de `runtime.*` que T10/T11 vão consumir
- [ ] Nenhum valor em EN ou PT contém "Ollama" ou "LM Studio"
- [ ] Gate check passa: `npm run build`, e a contagem de chaves EN é **igual** à de PT

**Tests:** none (arquivos de tradução — sem camada na matriz) · **Gate:** build
**Commit:** `feat(i18n): replace connection wording with runtime wording`

---

### T8: Tipos e API do frontend

**What:** `types.ts` perde os tipos de conexão e `connectionsApi` vira `runtimeApi`, sem `connectionId`.
**Where:** `src/types.ts`, `src/lib/runtimeApi.ts` (renomeado de `connectionsApi.ts`)
**Depends on:** T4
**Reuses:** o arquivo atual quase inteiro — muda o nome dos comandos e some um parâmetro
**Requirement:** SELF-01

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] Saem `ConnectionProvider`, `ConnectionStatus`, `Connection`, `ActivePair`
- [ ] Entram `RuntimeStatus` (sem os estágios de download de binário) e `ActiveModel` sem `connection_id`
- [ ] Cada função de `runtimeApi` corresponde a um comando registrado no `lib.rs` da T4 (conferido nome a nome)
- [ ] Gate check passa: `npm run build`

**Tests:** none (camada TS — matriz diz "none") · **Gate:** build
**Commit:** `refactor(frontend): replace the connections API with a runtime API`

---

### T9: Store do runtime

**What:** `connectionsStore` vira `runtimeStore`: estado do runtime, modelos instalados, catálogo e modelo ativo — sem lista de conexões.
**Where:** `src/store/runtimeStore.ts` (renomeado de `connectionsStore.ts`)
**Depends on:** T8
**Reuses:** o store atual (listeners de progresso, carregamento de modelos, tratamento de erro)
**Requirement:** SELF-01

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] Some `connections`, `loadConnections`, `setActiveConnection`, `clearActiveConnection`, `refreshConnectionStatus`, `addConnection`
- [ ] O listener de `connections-changed` vira `runtime-changed` (backend e frontend com o mesmo nome — conferir os dois lados)
- [ ] O progresso de download continua chegando durante o download de um GGUF
- [ ] Gate check passa: `npm run build`

**Tests:** none (camada TS) · **Gate:** build
**Commit:** `refactor(frontend): turn the connections store into a runtime store`

---

### T10: Tela de Runtime [P]

**What:** `ConnectionsPanel` + `ConnectionsList` + `EmbeddedRuntimeCard` viram um `RuntimePanel` com um card de runtime e a lista de modelos.
**Where:** `src/components/Runtime/{RuntimePanel,RuntimeCard}.tsx`, `src/components/Runtime/ModelsList.tsx`; apagar `src/components/Connections/`
**Depends on:** T7, T9
**Reuses:** `EmbeddedRuntimeCard.tsx`, `ModelsList.tsx`, `ModelDownloadCard.tsx`, `ModelConfigForm.tsx` (tirando `connectionId`)
**Requirement:** SELF-01

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] Não existe mais formulário de adicionar conexão, seletor de provedor nem lista de conexões
- [ ] O card mostra estado (não preparado / pronto / rodando / erro), backend detectado e o modelo ativo
- [ ] A preparação não exibe etapa de download de binário (some com a T17; até lá o estágio simplesmente não aparece)
- [ ] `src/components/Connections/` não existe mais
- [ ] Gate check passa: `npm run build`

**Tests:** none (componentes React — matriz diz "none") · **Gate:** build
**Commit:** `feat(ui): replace the connections screen with a runtime screen`

---

### T11: Sidebar e roteamento de view [P]

**What:** A seção "Conexões" da sidebar vira "Runtime" e o `App.tsx` aponta para o painel novo.
**Where:** `src/components/Sidebar/RuntimeSection.tsx` (renomeado de `ConnectionsSection.tsx`), `src/components/Sidebar/Sidebar.tsx`, `src/App.tsx`, `src/store/uiStore.ts`
**Depends on:** T7, T9
**Reuses:** a seção atual inteira — muda rótulo, ícone opcional e o nome da view
**Requirement:** SELF-01

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] A view `connections` do `uiStore` vira `runtime` e não sobra nenhuma referência ao nome antigo
- [ ] A sidebar mostra o modelo ativo (ou "nenhum modelo") em vez de estado de conexão
- [ ] Gate check passa: `npm run build`

**Tests:** none (componentes React) · **Gate:** build
**Commit:** `feat(ui): rename the connections section to runtime`

---

### T12: Manifesto de versões e script de vendoring

**What:** `vendor.json` com as três versões fixadas e um script Node que baixa, extrai e poda os artefatos da plataforma hospedeira em `src-tauri/resources/`.
**Where:** `scripts/vendor.json`, `scripts/vendor-runtime.mjs`, `scripts/vendor-runtime.test.mjs`, `package.json` (script `vendor`)
**Depends on:** Nenhuma
**Reuses:** o estilo de `bump-version.mjs` (ESM, zero dependências, funções puras exportadas e testadas)
**Requirement:** SELF-14, SELF-15

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] `vendor.json` declara tag do llama.cpp, versão do ONNX Runtime, release do pdfium e **os nomes de asset por extenso** (não montados por template)
- [ ] O script baixa o asset da plataforma hospedeira, extrai para `resources/llama/<backend>/`, `resources/onnxruntime/`, `resources/pdfium/`
- [ ] A poda remove apenas os executáveis `llama-*` que não são o `llama-server`, e **preserva toda biblioteca compartilhada**
- [ ] Um asset ausente no servidor falha o script nomeando o arquivo procurado
- [ ] Com o `.vendor-stamp.json` batendo com o `vendor.json`, uma segunda execução é no-op
- [ ] Gate check passa: `npm run test:scripts`
- [ ] Contagem de testes: 27 atuais + no mínimo 5 novos (seleção de asset por plataforma, regra de poda, stamp bate, stamp desatualizado, asset ausente)

**Tests:** unit (`node --test`) · **Gate:** quick
**Commit:** `feat(build): vendor the runtime components from a pinned manifest`

---

### T13: Declarar os recursos no bundle

**What:** O Tauri passa a empacotar `src-tauri/resources/` e a rodar o vendoring antes de todo build e dev.
**Where:** `src-tauri/tauri.conf.json`, `src-tauri/build.rs`, `.gitignore`
**Depends on:** T12
**Reuses:** `beforeBuildCommand` / `beforeDevCommand` já existentes
**Requirement:** SELF-09, SELF-14

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] `bundle.resources` = `["resources/"]` (forma de array, barra final)
- [ ] `beforeBuildCommand` e `beforeDevCommand` rodam `npm run vendor` antes do que já rodavam
- [ ] `build.rs` declara `cargo:rerun-if-changed=resources` (é o contorno documentado para arquivo novo não ser copiado em dev)
- [ ] `src-tauri/resources/` está no `.gitignore` — artefato de build, não código
- [ ] Gate check passa: `cd src-tauri && cargo check` e `npm run tauri dev` chega em `Running`
- [ ] **Verificar e registrar:** em qual diretório os recursos aparecem depois de um `tauri dev` (Open Question #2 do design)

**Tests:** none (configuração) · **Gate:** build
**Commit:** `feat(build): ship the vendored runtime components as bundle resources`

---

### T14: Resolver componentes dentro do app

**What:** Módulo que responde "onde está o componente X" a partir do `resource_dir`, com garantia de bit de execução.
**Where:** `src-tauri/src/runtime/bundled.rs` (novo), `src-tauri/src/runtime/mod.rs`
**Depends on:** T13
**Reuses:** `find_server_binary` (de `embedded_commands.rs`), `find_dylib` (de `rag/onnxruntime.rs`), `find_library` (de `rag/pdfium.rs`) — as três viram uma `find_file` só
**Requirement:** SELF-10, SELF-13

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] `resource_root`, `llama_server(backend)`, `onnxruntime_dylib` e `pdfium_library` resolvem caminhos sem nenhuma chamada de rede
- [ ] Componente ausente devolve erro nomeando o arquivo e o diretório, e sugerindo reinstalar — nunca uma tentativa de download
- [ ] `ensure_executable`: no Unix, arquivo já executável é devolvido como está; sem o bit, tenta `chmod`; se o `chmod` falhar, copia a pasta do backend para a pasta-base e marca `0o755`
- [ ] No Windows `ensure_executable` é no-op
- [ ] Gate check passa: `cd src-tauri && cargo test runtime::bundled`
- [ ] Contagem de testes: no mínimo 5 novos (busca recursiva acha, busca falha com mensagem útil, `ensure_executable` com bit, sem bit + destino gravável, sem bit + destino só-leitura → cópia)

**Tests:** unit · **Gate:** quick
**Commit:** `feat(runtime): resolve bundled components from the app resources`

---

### T15: pdfium vem do bundle [P]

**What:** O leitor de PDF passa a apontar para o recurso embutido; todo o caminho de download sai.
**Where:** `src-tauri/src/rag/pdfium.rs`
**Depends on:** T14
**Reuses:** `LIBRARY_PATH` + `extract_text` (inalterados), `runtime::bundled::pdfium_library`
**Requirement:** SELF-12

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] `asset_url`, a constante `RELEASE` e a chamada a `download_with_progress` saem do arquivo
- [ ] `ensure_for` continua curto-circuitando em não-PDF (importar `.txt` não toca em nada)
- [ ] Erro de biblioteca ausente fala em reinstalar, não em rede
- [ ] Gate check passa: `cd src-tauri && cargo test rag::pdfium`
- [ ] Contagem de testes: os 2 atuais adaptados + 1 novo (biblioteca ausente → mensagem de reinstalação)

**Tests:** unit · **Gate:** quick
**Commit:** `feat(rag): load pdfium from the bundled resources`

---

### T16: ONNX Runtime vem do bundle [P]

**What:** `ORT_DYLIB_PATH` passa a apontar para o recurso embutido; o download sai.
**Where:** `src-tauri/src/rag/onnxruntime.rs`
**Depends on:** T14
**Reuses:** `set_dylib_path`, `runtime::bundled::onnxruntime_dylib`
**Requirement:** SELF-12

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] `asset_url`, `ORT_VERSION` e o download saem do arquivo (a versão agora mora no `vendor.json`)
- [ ] `ORT_DYLIB_PATH` é definido antes da primeira sessão de embedding, como hoje
- [ ] Gate check passa: `cd src-tauri && cargo test rag::`
- [ ] Contagem de testes: no mínimo 1 novo (dylib ausente → mensagem de reinstalação)

**Tests:** unit · **Gate:** quick
**Commit:** `feat(rag): load the ONNX Runtime from the bundled resources`

---

### T17: Preparar o runtime sem baixar nada

**What:** `prepare_runtime` deixa de resolver release e baixar binário: escolhe entre os dois backends embutidos por `probe_devices`.
**Where:** `src-tauri/src/runtime_commands.rs`, `src-tauri/src/runtime/mod.rs`; apagar `src-tauri/src/runtime/release.rs` e a função `download::extract`
**Depends on:** T14, T4
**Reuses:** `runtime::detect::probe_devices` e `classify_output` (inalterados), `runtime::process::spawn`
**Requirement:** SELF-10, SELF-11, SELF-15

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] Preparar o runtime não faz nenhuma requisição HTTP
- [ ] Vulkan é probado primeiro; se o binário não executa, o binário CPU embutido assume
- [ ] Se nenhum dos dois executa, o erro traz o motivo do último probe e o app segue aberto
- [ ] Os estágios `DownloadingBinary` e o campo de tag de release resolvida saem; `release_tag` passa a receber a tag do `vendor.json`
- [ ] `runtime/release.rs` não existe mais; `download::extract` sai e `tar`/`flate2` saem do `Cargo.toml` se ficarem sem uso (conferido por `cargo check`)
- [ ] Gate check passa: `cd src-tauri && cargo test runtime::` e `npm run tauri dev` sobe o sidecar
- [ ] Contagem de testes: os de `release.rs` somem com o arquivo (remoção legítima, registrar); no mínimo 2 novos para a escolha de backend

**Tests:** unit · **Gate:** full
**Commit:** `feat(runtime): start llama-server from the bundled binaries`

---

### T18: Apagar os downloads das versões anteriores

**What:** Faxina no boot dos diretórios que a versão antiga baixava e que ninguém mais lê.
**Where:** `src-tauri/src/runtime/bundled.rs` (ou módulo próprio `runtime::legacy`), chamada em `src-tauri/src/lib.rs`
**Depends on:** T15, T16, T17
**Reuses:** o padrão de `update_commands::cleanup_after_update` (best-effort, falha ignorada, roda cedo no boot)
**Requirement:** SELF-18

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] Remove `<base>/runtime/{vulkan,cpu,onnxruntime,pdfium}` quando existirem
- [ ] **Nunca** apaga `<base>/runtime/` inteiro: só os quatro subdiretórios nomeados. O M7.1 grava `<base>/runtime/llama-server.log` ali, e a faxina não pode levá-lo junto
- [ ] **Nunca** toca em `<base>/models` nem em nada fora de `<base>/runtime`
- [ ] Falha de remoção é ignorada e o app abre normalmente
- [ ] Gate check passa: `cd src-tauri && cargo test`
- [ ] Contagem de testes: no mínimo 3 novos (remove o que deve, preserva modelos, pasta inexistente é silenciosa)

**Tests:** unit · **Gate:** quick
**Commit:** `feat(runtime): remove runtime components downloaded by earlier versions`

---

### T19: Bundle portátil leva os recursos

**What:** `make-portable.mjs` copia a pasta de recursos junto do executável.
**Where:** `scripts/make-portable.mjs`, `scripts/make-portable.test.mjs`
**Depends on:** T13
**Reuses:** o script inteiro; `update::portable::move_tree` já é recursivo, então o lado da atualização não muda
**Requirement:** SELF-16, SELF-17

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] O zip contém `LocalMind.exe`, `.portable`, `README.txt` **e** a árvore de recursos
- [ ] Recursos ausentes no diretório de build falham o script com mensagem explícita (não geram um zip mudo e quebrado)
- [ ] Gate check passa: `npm run test:scripts`
- [ ] Contagem de testes: os atuais + no mínimo 2 novos (recursos copiados, ausência falha)

**Tests:** unit (`node --test`) · **Gate:** quick
**Commit:** `feat(release): include the bundled resources in the portable archive`

---

### T20: Provar o bit de execução no pacote Linux

**What:** Verificação automatizada de que o `llama-server` sai executável do `.deb` — a Open Question #1 do design, respondida com evidência.
**Where:** `scripts/check-linux-bundle.mjs`, `scripts/check-linux-bundle.test.mjs`, `.github/workflows/release.yml`
**Depends on:** T14, T17
**Reuses:** o padrão dos scripts existentes; `dpkg -c` já disponível no runner Ubuntu
**Requirement:** SELF-13

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] O script lê a saída de `dpkg -c` e reporta o modo do `llama-server` dentro do pacote
- [ ] Roda no job de build do Linux, depois do bundling
- [ ] O resultado é **registrado** no design (Open Question #1) — se o bit vier preservado, a cópia de fallback da T14 fica como rede de segurança; se não vier, o fallback é o caminho normal e isso passa a estar documentado
- [ ] O script falha o build se o `llama-server` não estiver no pacote (ausência é erro, modo sem bit não é)
- [ ] Gate check passa: `npm run test:scripts`
- [ ] Contagem de testes: no mínimo 2 novos (parse de uma saída de `dpkg -c` com bit, e sem bit)

**Tests:** unit (`node --test`) · **Gate:** quick
**Commit:** `test(release): verify the llama-server permissions inside the deb package`

---

### T21: Documentação para de descrever multi-provider

**What:** Todo documento que promete Ollama/LM Studio passa a descrever o app que existe.
**Where:** `.specs/project/PROJECT.md`, `.specs/project/ROADMAP.md`, `.specs/codebase/{STACK,ARCHITECTURE,INTEGRATIONS,STRUCTURE,CONCERNS}.md`, `README.md`
**Depends on:** T5, T11, T17
**Reuses:** —
**Requirement:** SELF-19

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [ ] `PROJECT.md` (visão, goals, stack, escopo) não cita Ollama, LM Studio nem "detecção automática"
- [ ] O diagrama de arquitetura do ROADMAP mostra um runtime só
- [ ] `INTEGRATIONS.md` deixa de listar integrações que não existem mais; `CONCERNS.md` marca o C-05 como resolvido por remoção
- [ ] `README.md` descreve a instalação real (nada a instalar além do LocalMind; um modelo a baixar)
- [ ] `grep -ri "ollama\|lmstudio\|lm studio" README.md .specs/project .specs/codebase` só retorna menções históricas datadas
- [ ] Gate check: revisão manual (documentação não tem gate automatizado)

**Tests:** none · **Gate:** none
**Commit:** `docs: describe LocalMind as a single self-contained runtime`

---

### T22: Verificação de ponta a ponta, offline

**What:** O gate real da feature: uma máquina sem rede instala e conversa. Exige o usuário — nenhum agente fecha esta task sozinho.
**Where:** — (verificação manual)
**Depends on:** T18, T19, T20, T21
**Reuses:** —
**Requirement:** SELF-09, SELF-11, SELF-12, todos os critérios de sucesso

**Tools:** MCP: NONE · Skill: NONE

**Done when:**

- [x] ⚠️ Instalador gerado e **tamanho medido** — **só Windows** (NSIS 53,3 · MSI 91,4 · zip 107,2 MiB, 2026-07-27, árvore corrigida). **Linux continua sem número**, e o delta contra a versão publicada segue sem base de comparação: não há v0.1.1 publicada para comparar
- [ ] Com a rede desligada e um `.gguf` copiado à mão: instalar → abrir → escolher o modelo → conversar
- [ ] Com a rede desligada: importar um PDF e vê-lo chegar a `ready`; perguntar sobre ele e receber citação
- [ ] Com rede: baixar um modelo do catálogo e conversar, sem nenhuma outra etapa de download
- [x] ⚠️ Bundle portátil extraído em pasta de usuário ~~sobe e conversa~~ — **meio feito**: extraído no scratchpad e o `llama-server.exe` de dentro dele executado (exit 0, Vulkan achando a RTX 3060). **O `LocalMind.exe` do bundle não foi aberto e ninguém conversou** — essa metade continua exigindo uma pessoa
- [x] Gate check passa: `cargo test` **174 passando / 0 falhas / 12 ignorados** (T17 tinha 150), `npm run test:scripts` **44**, `npm run build` limpo

**Tests:** none (UAT) · **Gate:** full
**Commit:** —

---

## Parallel Execution Map

```
Fase 1 (backend) — T1 e T2 simultâneas, o resto em fila:
  {T1 [P], T2 [P]} ──→ {T3, T4} ──→ T5 ──→ T6
  (T3 e T4 dependem ambas de T1+T2; T5 depende de T3+T4)

Fase 2 (frontend) — T10 e T11 simultâneas no fim:
  T7 ─────────────────┐
  T4 ──→ T8 ──→ T9 ───┴──→ {T10 [P], T11 [P]}

Fase 3 (bundle) — T15 e T16 simultâneas:
  T12 ──→ T13 ──→ T14 ──→ {T15 [P], T16 [P]} ──┐
                  T14 + T4 ──→ T17 ────────────┴──→ T18

Fase 4 (distribuição) — nada simultâneo, tudo converge em T22:
  T13 ────────────→ T19 ──┐
  T14 + T17 ──────→ T20 ──┼──→ T22
  T5 + T11 + T17 ──→ T21 ─┘
```

As fases 1 e 3 podem começar em paralelo até o ponto em que T17 precisa da T4 — na prática, um agente na Fase 1 e outro na T12/T13 não se atropelam (arquivos disjuntos: `providers/`+`chat/` de um lado, `scripts/`+`tauri.conf.json` do outro).

---

## Task Granularity Check

| Task | Escopo | Status |
| --- | --- | --- |
| T1 | 1 arquivo novo (1 struct) | ✅ Granular |
| T2 | 1 arquivo, 3 funções | ✅ Granular |
| T3 | 2 arquivos coesos (mesma troca de fonte) | ⚠️ OK — coeso |
| T4 | 1 módulo renomeado + registro | ✅ Granular |
| T5 | Só remoção — 7 arquivos, zero lógica nova | ⚠️ OK — remoção mecânica, verificável por `cargo check` |
| T6 | 1 migração | ✅ Granular |
| T7 | 2 arquivos espelhados | ✅ Granular |
| T8 | 2 arquivos (tipos + API) | ⚠️ OK — a API existe para os tipos |
| T9 | 1 arquivo | ✅ Granular |
| T10 | 1 tela (3 componentes reaproveitados) | ⚠️ OK — uma tela é a unidade demonstrável |
| T11 | 1 seção + roteamento | ✅ Granular |
| T12 | 1 script + 1 manifesto | ✅ Granular |
| T13 | 1 configuração | ✅ Granular |
| T14 | 1 módulo | ✅ Granular |
| T15 | 1 arquivo | ✅ Granular |
| T16 | 1 arquivo | ✅ Granular |
| T17 | 1 função + remoções ligadas a ela | ✅ Granular |
| T18 | 1 função | ✅ Granular |
| T19 | 1 script | ✅ Granular |
| T20 | 1 script + 1 passo de CI | ✅ Granular |
| T21 | Só documentação | ⚠️ OK — indivisível na prática |
| T22 | Verificação manual | ✅ Granular |

Nenhuma ❌: as quatro ⚠️ são casos de "2-3 coisas relacionadas no mesmo recorte", permitidos pela regra.

---

## Diagram-Definition Cross-Check

| Task | Depends on (corpo) | Diagrama mostra | Status |
| --- | --- | --- | --- |
| T1 | — | sem seta de entrada | ✅ |
| T2 | — | sem seta de entrada | ✅ |
| T3 | T1, T2 | T1→T3, T2→T3 | ✅ |
| T4 | T1, T2 | T1→T4, T2→T4 | ✅ |
| T5 | T3, T4 | T3→T5, T4→T5 | ✅ |
| T6 | T5 | T5→T6 | ✅ |
| T7 | — | sem seta de entrada | ✅ |
| T8 | T4 | T4→T8 | ✅ |
| T9 | T8 | T8→T9 | ✅ |
| T10 | T7, T9 | T7→T10, T9→T10 | ✅ |
| T11 | T7, T9 | T7→T11, T9→T11 | ✅ |
| T12 | — | sem seta de entrada | ✅ |
| T13 | T12 | T12→T13 | ✅ |
| T14 | T13 | T13→T14 | ✅ |
| T15 | T14 | T14→T15 | ✅ |
| T16 | T14 | T14→T16 | ✅ |
| T17 | T14, T4 | T14→T17, T4→T17 | ✅ |
| T18 | T15, T16, T17 | os três →T18 | ✅ |
| T19 | T13 | T13→T19 | ✅ |
| T20 | T14, T17 | T14→T20, T17→T20 | ✅ |
| T21 | T5, T11, T17 | os três →T21 | ✅ |
| T22 | T18, T19, T20, T21 | os quatro →T22 | ✅ |

Os `[P]` conferem: T1/T2 não dependem um do outro; T10/T11 também não; T15/T16 também não (arquivos disjuntos, e `cargo test` é paralelo-seguro pela avaliação da TESTING.md).

---

## Test Co-location Validation

| Task | Camada criada/modificada | Matriz exige | Task diz | Status |
| --- | --- | --- | --- | --- |
| T1 | Provider HTTP | unit com mocks | unit | ✅ |
| T2 | Função pura + SQL em memória | unit | unit | ✅ |
| T3 | Comando Tauri + montagem de contexto (pura) | unit (a mais alta das duas) | unit | ✅ |
| T4 | Comandos Tauri (só orquestração) | none | none | ✅ |
| T5 | Remoção | none | none | ✅ |
| T6 | Migração de schema | unit | unit | ✅ |
| T7 | Tradução (sem camada) | none | none | ✅ |
| T8 | TS/tipos | none | none | ✅ |
| T9 | Store React | none | none | ✅ |
| T10 | Componentes React | none | none | ✅ |
| T11 | Componentes React | none | none | ✅ |
| T12 | Script Node | unit (`node --test`) | unit | ✅ |
| T13 | Configuração | none | none | ✅ |
| T14 | Funções puras (caminho, permissão) | unit | unit | ✅ |
| T15 | Parser de documento | unit | unit | ✅ |
| T16 | Motor de embedding | unit | unit | ✅ |
| T17 | Lógica pura (escolha de backend) + comando | unit | unit | ✅ |
| T18 | Função pura de filesystem | unit | unit | ✅ |
| T19 | Script Node | unit | unit | ✅ |
| T20 | Script Node | unit | unit | ✅ |
| T21 | Documentação | none | none | ✅ |
| T22 | UAT | none | none | ✅ |

Nenhuma ❌ VIOLATION. Nenhuma task adia teste para outra: onde a matriz exige, o teste está na task que cria o código.

---

## Gate Check Commands (desta feature)

| Gate | Comando |
| --- | --- |
| quick (Rust) | `cd src-tauri && cargo test <módulo>::` |
| quick (scripts) | `npm run test:scripts` |
| build (Rust) | `cd src-tauri && cargo check` |
| build (frontend) | `npm run build` |
| full | `cargo test` completo + `npm run tauri dev` até `Running`, sidecar de pé |

**Linha de base a preservar:** 123 testes Rust e 27 de script passando hoje. Toda task que reduzir esses números tem de justificar cada teste perdido no commit — a única justificativa aceita é "o código que ele testava foi removido" (T5 e T17).
