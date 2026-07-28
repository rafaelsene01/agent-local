# Concerns

Riscos observados no código real (com caminho/arquivo como evidência), priorizados por impacto. Documentado após o M3 — nada aqui é bloqueante hoje, mas vários itens ficam mais caros quanto mais tarde forem resolvidos.

## Alto impacto

### C-01: Schema SQLite não tem versionamento — alterar coluna existente não migra

**Evidência:** `src-tauri/src/db.rs` — `const SCHEMA` é só uma sequência de `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`, executada em todo `db::open()`.
**Risco:** funciona perfeitamente pra **adicionar tabela nova** (foi o caso do M3), mas se alguma feature futura precisar adicionar/renomear/remover **coluna de tabela existente**, o `IF NOT EXISTS` faz o comando virar no-op silencioso em qualquer banco já criado. O usuário fica com schema antigo e erro de runtime em `row.get(N)`, sem nenhum aviso.
**Quando dói:** o M7 (`embedded-runtime`) e a mudança de "conexão única ativa" já mexem em `connections` — este é o momento natural de resolver.
**Fix sugerido:** adicionar `PRAGMA user_version` + um vetor de migrações aplicadas em ordem (`migrations: &[(u32, &str)]`), rodando só as acima da versão atual. ~40 linhas, sem dependência nova.

### C-02: ~~`list_connections` faz health checks sequenciais de 5s — trava a UI~~ — **RESOLVIDO POR REMOÇÃO (2026-07-27, M9)**

Não existe mais lista de conexões para checar. O `runtime_status` lê uma linha de banco e consulta o estado do processo filho em memória; nenhum health check HTTP acontece ao abrir a sidebar. `list_connections`, `ConnectionManager` e os três clients com timeout de 5s foram apagados na AD-042.

### C-03: `src/types.ts` espelha as structs Rust manualmente, sem geração

**Evidência:** `src/types.ts` replica à mão structs de `providers/mod.rs`, `runtime/store.rs`, `models/catalog.rs` e `runtime_commands.rs`. O M9 **reduziu** a superfície — `Connection`, `ConnectionProvider`, `ConnectionStatus`, `ActivePair` e `ConfigApplied` deixaram de existir dos dois lados —, mas não resolveu o problema. O caso mais frágil é `DownloadableModel`: no Rust é `struct DownloadableModel { #[serde(flatten)] info: CuratedModelInfo, fits_ram: bool }`, e no TS é uma interface **plana** com todos os campos — a correspondência só existe por causa do `flatten`.
**Risco:** renomear um campo no Rust compila normalmente e o TS também compila; a quebra só aparece em runtime, como `undefined` na tela. Nenhum teste pega isso hoje (ver C-04).
**Fix sugerido:** adotar `ts-rs` ou `specta`/`tauri-specta` pra gerar `types.ts` a partir das structs. Vale a pena quando o número de tipos crescer mais (hoje são 8; dobrar isso torna o manual insustentável).

### C-04: Zero cobertura de teste no frontend

**Evidência:** `package.json` não tem Vitest, Jest, Testing Library nem script `test`. `.specs/codebase/TESTING.md` já registra isso como "none (por ora)".
**Risco:** 12 componentes React e 4 stores Zustand, incluindo lógica não-trivial sem teste nenhum: o filtro `fits_ram` + toggle "mostrar todos" (`ModelsList.tsx`), o cálculo de percentual de download (`ModelDownloadCard.tsx`), o listener de evento que indexa progresso pela URL do `.gguf` (`runtimeStore.ts`) e, desde o M6, o listener de `memory-backfill-progress` que descarta eventos de outra conversa (`chatStore.ts`).
**Fix sugerido:** Vitest + RTL cobrindo primeiro os stores (lógica pura, sem DOM) — é onde está o maior risco por menor esforço.

## Médio impacto

### C-05: ~~Providers nunca exercitados contra servidor real~~ — **RESOLVIDO POR REMOÇÃO (2026-07-27, M9)**

Os dois clients que nunca tinham falado com um servidor de verdade (`OllamaClient`, `LmStudioClient`) são justamente os que saíram. O que restou — `LlamaServerClient` — fala com o sidecar que o próprio app sobe, e esse caminho já foi exercitado ao vivo na AD-028 e na AD-041.

### C-06: ~~Polling de download do LM Studio não tem timeout nem cancelamento~~ — **RESOLVIDO POR REMOÇÃO (2026-07-27, M9)**

O loop de polling saiu junto com `providers/lmstudio.rs`. Todo download agora é um GET direto de um `.gguf`, com progresso por bytes e sem estado de job para consultar.

### C-07: ~~`require_conn` duplicado em 3 arquivos~~ — **RESOLVIDO**

**Evidência:** a duplicação foi resolvida no caminho: `require_conn` vive em `db.rs` e é importada. Dois dos três arquivos que a copiavam (`connection_commands.rs`, `model_commands.rs`) nem existem mais.
**Status:** resolvido. Mantido aqui como registro.

### C-08: ~~Token de auth do LM Studio não é enviado~~ — **RESOLVIDO POR REMOÇÃO (2026-07-27, M9)**

Não há mais servidor externo a autenticar. O sidecar é filho do app, escuta em `127.0.0.1` numa porta efêmera e não usa credencial.

## Baixo impacto

### C-09: Sem linter nem formatter — CI resolvido pelo M8 (2026-07-26)

**Evidência:** não existem `.eslintrc*`, `.prettierrc*`, `rustfmt.toml` nem `clippy.toml`. `.github/workflows/` **passou a existir** com o M8: `ci.yml` roda `npm run build`, `cargo test` e valida Conventional Commits em todo push e PR. **Nunca foi executado no GitHub**, porém — o repositório ainda não teve um push que o dispare.
**Risco (o que sobrou):** estilo mantido só por disciplina manual. O build quebrado agora é pego pelo CI; o estilo divergente não.
**Fix sugerido:** `cargo clippy -D warnings` e `cargo fmt --check` foram deixados **de fora do M8 de propósito** (AD-034): o código atual não passa, e introduzi-los junto com o CI viraria uma refatoração disfarçada. Entram depois de pagar as dívidas — o `cargo check` de hoje ainda emite 5 warnings de dead code, incluindo o C-11.

### C-13: ~~Chaves estrangeiras declaradas mas nunca aplicadas~~ — RESOLVIDO (2026-07-26)

**Evidência (era):** `db::open` não executava `PRAGMA foreign_keys = ON`, e o SQLite deixa isso desligado por conexão. O `ON DELETE CASCADE` de `model_configs.connection_id` e a referência de `messages.chat_id` eram decorativos.
**Risco (era):** apagar um chat durante uma geração inseria a resposta num chat inexistente, como linha órfã silenciosa; e a primeira funcionalidade de apagar conexão herdaria `model_configs` órfãos confiando numa declaração que não valia.
**Resolução:** pragma ligado no `open`, com três testes — que o pragma está ativo, que o CASCADE dispara, e que uma mensagem órfã é recusada. Ver AD-040.

### C-14: ~~`delete_chat` não cancela a geração em andamento~~ — **RESOLVIDO (2026-07-27)**

**Evidência (era):** `commands.rs::delete_chat` apagava mensagens, anexos e o chat, mas não tocava no `CancellationRegistry`.
**Resolução:** `app.state::<CancellationRegistry>().cancel(&id)` como **primeira** linha do comando, antes da transação — a mesma via do `cancel_generation`. Sinalizar antes de apagar também estreita a janela que `chat::memory::record_turn` cobre com a checagem de existência: quanto antes o laço para, menos provável é ele chegar ao ponto de gravar memória.

**O que isto não tem:** teste automatizado. `delete_chat` é um comando Tauri que só orquestra I/O, e a matriz do `TESTING.md` põe isso explicitamente na coluna "nenhum teste" — não há runner de integração Tauri, e o comando precisa de um `AppHandle`. **A prova é de UAT e ainda não foi feita**: apagar um chat no meio de uma geração e observar o sidecar parar.

> **O M6 encostou nisto sem resolver (2026-07-27, AD-044).** A gravação de memória roda no fim da geração, então uma conversa apagada no meio poderia receber vetores num namespace que o `delete_chat` já limpou — órfãos que nada mais apagaria. `chat::memory::record_turn` confere que o chat ainda existe antes do `upsert`, o mesmo padrão do `still_exists` do pipeline. Isso fecha a janela nova; **a concern original continua aberta**.

### C-10: ~~Semeadura de conexão casa por `provider`, não por URL~~ — **RESOLVIDO POR REMOÇÃO (2026-07-27, M9)**

Não há semeadura: a tabela `connections` foi derrubada pela migração 7 e o runtime é um só, descoberto no `resource_dir` do próprio app.

### C-11: ~~Variantes `Quant::Q5/Q8/F16` sem uso (warning permanente no build)~~ — **RESOLVIDO (2026-07-27)**

**Evidência (era):** warning `variants Q5, Q8 and F16 are never constructed` em `cargo check` — os 6 modelos curados usam todos `Quant::Q4`.
**Resolução:** `#[allow(dead_code)]` explícito no enum, com o motivo escrito ao lado: a tabela descreve o **esquema de quantização**, não o catálogo atual, e apagar as variantes deixaria `estimate_ram_gb` especializada em Q4 continuando a se chamar como se fosse geral.

**Três warnings vizinhos foram varridos na mesma passada, e dois eram código morto de verdade:**

- `HEALTH_CHECK_TIMEOUT` e `LlamaServerClient::health_check` — sobras da tela de Conexões, que saiu com o M9 (AD-042). O único chamador restante era um teste, isto é, o método existia para que o teste tivesse o que chamar. Removidos os dois; o teste continua, exercitando `model_limits`.
- `PullStatus::Verifying` — a fase de checksum do `pull` do Ollama. Um GGUF baixado por um GET não tem essa fase. Removido no Rust **e** em `src/types.ts`, que espelha o enum à mão (C-03).
- Um `let mut` desnecessário num teste de `db.rs`.

**Estado:** `cargo check --lib` e `cargo check --lib --tests` passam com **zero warnings**. Isso é o pré-requisito que faltava para o C-09 poder ligar `clippy -D warnings` sem virar refatoração disfarçada — embora o `clippy` em si ainda não tenha sido rodado.

### C-12: Verificação só em Windows

**Evidência:** toda execução desta sessão foi `win32`; nunca houve build ou execução em Linux.
**Risco:** `tauri.conf.json` tem `"targets": "all"` e o roadmap promete `.AppImage`/`.deb` no M8, mas nada disso foi exercitado. Caminhos de filesystem usam `PathBuf`/`join` corretamente (portável), o que reduz o risco — mas é uma promessa não verificada.
**Fix sugerido:** entra naturalmente no M8 com a matrix do GitHub Actions.
