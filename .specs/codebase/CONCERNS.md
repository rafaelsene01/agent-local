# Concerns

Riscos observados no código real (com caminho/arquivo como evidência), priorizados por impacto. Documentado após o M3 — nada aqui é bloqueante hoje, mas vários itens ficam mais caros quanto mais tarde forem resolvidos.

## Alto impacto

### C-01: Schema SQLite não tem versionamento — alterar coluna existente não migra

**Evidência:** `src-tauri/src/db.rs` — `const SCHEMA` é só uma sequência de `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`, executada em todo `db::open()`.
**Risco:** funciona perfeitamente pra **adicionar tabela nova** (foi o caso do M3), mas se alguma feature futura precisar adicionar/renomear/remover **coluna de tabela existente**, o `IF NOT EXISTS` faz o comando virar no-op silencioso em qualquer banco já criado. O usuário fica com schema antigo e erro de runtime em `row.get(N)`, sem nenhum aviso.
**Quando dói:** o M7 (`embedded-runtime`) e a mudança de "conexão única ativa" já mexem em `connections` — este é o momento natural de resolver.
**Fix sugerido:** adicionar `PRAGMA user_version` + um vetor de migrações aplicadas em ordem (`migrations: &[(u32, &str)]`), rodando só as acima da versão atual. ~40 linhas, sem dependência nova.

### C-02: `list_connections` faz health checks sequenciais de 5s — trava a UI

**Evidência:** `src-tauri/src/connection_commands.rs` — o loop `for conn in base_list { manager.refresh_status(&conn).await }` é sequencial; cada client usa `timeout(Duration::from_secs(5))` (`providers/ollama.rs`, `lmstudio.rs`, `custom.rs`).
**Risco:** com Ollama e LM Studio ambos **desligados** (o cenário mais comum de primeira execução), abrir a sidebar leva **~10s** só esperando timeouts. Cada conexão custom adicionada soma mais 5s. `ConnectionsSection` chama isso no `useEffect` de montagem, então o app inteiro parece travado.
**Fix sugerido:** paralelizar com `futures_util::future::join_all` (o crate já está nas dependências) — passa de 10s pra ~5s; e/ou baixar o timeout de health check pra 1-2s (é `localhost`, não rede).

### C-03: `src/types.ts` espelha as structs Rust manualmente, sem geração

**Evidência:** `src/types.ts` tem 8 interfaces que replicam à mão structs de `providers/mod.rs`, `connections.rs`, `models/catalog.rs` e `model_commands.rs`. O caso mais frágil é `DownloadableModel`: no Rust é `struct DownloadableModel { #[serde(flatten)] info: CuratedModelInfo, fits_ram: bool }`, e no TS é uma interface **plana** com todos os campos — a correspondência só existe por causa do `flatten`.
**Risco:** renomear um campo no Rust compila normalmente e o TS também compila; a quebra só aparece em runtime, como `undefined` na tela. Nenhum teste pega isso hoje (ver C-04).
**Fix sugerido:** adotar `ts-rs` ou `specta`/`tauri-specta` pra gerar `types.ts` a partir das structs. Vale a pena quando o número de tipos crescer mais (hoje são 8; dobrar isso torna o manual insustentável).

### C-04: Zero cobertura de teste no frontend

**Evidência:** `package.json` não tem Vitest, Jest, Testing Library nem script `test`. `.specs/codebase/TESTING.md` já registra isso como "none (por ora)".
**Risco:** 12 componentes React e 4 stores Zustand, incluindo lógica não-trivial sem teste nenhum: o filtro `fits_ram` + toggle "mostrar todos" (`ModelsList.tsx`), o cálculo de percentual de download (`ModelDownloadCard.tsx`), o listener de evento que indexa progresso por chave composta (`connectionsStore.ts`).
**Fix sugerido:** Vitest + RTL cobrindo primeiro os stores (lógica pura, sem DOM) — é onde está o maior risco por menor esforço.

## Médio impacto

### C-05: Providers nunca exercitados contra servidor real

**Evidência:** registrado na AD-019 do STATE.md e nos commits de T5/T6 — nem Ollama nem LM Studio estavam rodando durante a implementação do M3. A verificação foi `cargo check` + payloads confirmados na documentação oficial.
**Risco:** parsing de resposta (`TagsResponse`, `ModelsResponse`, `DownloadStatusResponse`) e o loop NDJSON nunca rodaram com bytes de verdade. Campos opcionais/ausentes na prática podem quebrar a desserialização.
**Fix sugerido:** já está nos Todos do STATE.md — subir Ollama localmente e percorrer detectar → listar → baixar modelo pequeno → configurar. Alternativa complementar: testes unitários com fixtures JSON gravadas (o `TESTING.md` já prevê "unit com fixtures/mocks quando prático" pra essa camada).

### C-06: Polling de download do LM Studio não tem timeout nem cancelamento

**Evidência:** `src-tauri/src/providers/lmstudio.rs` — `loop { … tokio::time::sleep(750ms) }` só sai em `completed`, `failed` ou erro HTTP.
**Risco:** se o job ficar em `"paused"` (status documentado na API), o loop roda indefinidamente consumindo uma request a cada 750ms pelo resto da sessão. Não há como o usuário cancelar — não existe comando de cancelamento de download em `model_commands.rs`.
**Fix sugerido:** tratar `"paused"` como estado terminal reportável, e/ou adicionar um `CancellationToken` (o padrão de `CancellationRegistry` já está previsto no design de `chat-messaging`, dá pra reusar).

### C-07: `require_conn` duplicado em 3 arquivos

**Evidência:** função idêntica em `commands.rs:8`, `connection_commands.rs:7` e `model_commands.rs:11` — mesma assinatura, mesma mensagem de erro em português.
**Risco:** baixo em si, mas é o tipo de duplicação que diverge silenciosamente (alguém muda a mensagem em um lugar só). Vai virar 4 cópias assim que o M7 adicionar comandos.
**Fix sugerido:** mover pra `db.rs` como `pub fn require_conn(...)` e importar. Refactor de ~10 minutos, melhor fazer antes do M7.

### C-08: Token de auth do LM Studio não é enviado

**Evidência:** a doc oficial mostra `Authorization: Bearer $LM_API_TOKEN` nos exemplos; `LmStudioClient` não envia header nenhum.
**Risco:** usuários que ativarem autenticação no LM Studio verão a conexão como "indisponível" sem explicação — o erro vira `ProviderError::Unavailable` genérico.
**Fix sugerido:** campo opcional de token na conexão (a tabela `connections` precisaria de uma coluna — ver C-01), ou pelo menos distinguir 401/403 de "servidor offline" na mensagem.

## Baixo impacto

### C-09: Sem linter nem formatter — CI resolvido pelo M8 (2026-07-26)

**Evidência:** não existem `.eslintrc*`, `.prettierrc*`, `rustfmt.toml` nem `clippy.toml`. `.github/workflows/` **passou a existir** com o M8: `ci.yml` roda `npm run build`, `cargo test` e valida Conventional Commits em todo push e PR. **Nunca foi executado no GitHub**, porém — o repositório ainda não teve um push que o dispare.
**Risco (o que sobrou):** estilo mantido só por disciplina manual. O build quebrado agora é pego pelo CI; o estilo divergente não.
**Fix sugerido:** `cargo clippy -D warnings` e `cargo fmt --check` foram deixados **de fora do M8 de propósito** (AD-034): o código atual não passa, e introduzi-los junto com o CI viraria uma refatoração disfarçada. Entram depois de pagar as dívidas — o `cargo check` de hoje ainda emite 5 warnings de dead code, incluindo o C-11.

### C-13: ~~Chaves estrangeiras declaradas mas nunca aplicadas~~ — RESOLVIDO (2026-07-26)

**Evidência (era):** `db::open` não executava `PRAGMA foreign_keys = ON`, e o SQLite deixa isso desligado por conexão. O `ON DELETE CASCADE` de `model_configs.connection_id` e a referência de `messages.chat_id` eram decorativos.
**Risco (era):** apagar um chat durante uma geração inseria a resposta num chat inexistente, como linha órfã silenciosa; e a primeira funcionalidade de apagar conexão herdaria `model_configs` órfãos confiando numa declaração que não valia.
**Resolução:** pragma ligado no `open`, com três testes — que o pragma está ativo, que o CASCADE dispara, e que uma mensagem órfã é recusada. Ver AD-040.

### C-14: `delete_chat` não cancela a geração em andamento

**Evidência:** `commands.rs::delete_chat` apaga mensagens, anexos e o chat, mas não toca no `CancellationRegistry`.
**Risco:** apagar um chat que está gerando deixa o `send_message` rodando até o fim, gastando GPU/CPU para um resultado que agora é recusado pelo banco (desde a C-13). Desperdício visível como lentidão, não como erro.
**Fix sugerido:** chamar o cancelamento antes da transação, do mesmo jeito que `cancel_generation` faz.

### C-10: Semeadura de conexão casa por `provider`, não por URL

**Evidência:** `connection_commands::list_connections` — `existing.iter().any(|c| c.provider == candidate.provider)`.
**Risco:** se o usuário adicionar manualmente uma conexão com provider `ollama` numa porta diferente, o Ollama padrão de `:11434` nunca é semeado (o app assume que já existe). Cenário raro, mas confuso quando acontece.
**Fix sugerido:** comparar por `base_url`, não por `provider`.

### C-11: Variantes `Quant::Q5/Q8/F16` sem uso (warning permanente no build)

**Evidência:** warning `variants Q5, Q8 and F16 are never constructed` em todo `cargo build` — todos os 8 modelos curados usam `Quant::Q4`.
**Risco:** nenhum funcional, mas warning constante treina o olho a ignorar a saída do compilador, o que esconde warnings reais.
**Fix sugerido:** ou usar as variantes (adicionar modelos com outro quant ao catálogo), ou `#[allow(dead_code)]` explícito com comentário dizendo que existem pra completude da fórmula.

### C-12: Verificação só em Windows

**Evidência:** toda execução desta sessão foi `win32`; nunca houve build ou execução em Linux.
**Risco:** `tauri.conf.json` tem `"targets": "all"` e o roadmap promete `.AppImage`/`.deb` no M8, mas nada disso foi exercitado. Caminhos de filesystem usam `PathBuf`/`join` corretamente (portável), o que reduz o risco — mas é uma promessa não verificada.
**Fix sugerido:** entra naturalmente no M8 com a matrix do GitHub Actions.
