# Memória de conversa (RAG híbrido) — Tasks

**Design:** `.specs/features/conversation-memory/design.md`
**Spec:** `.specs/features/conversation-memory/spec.md`
**Status:** **T1–T8 concluídas; a T9 rodou, reprovou o código, e o código corrigido passou
(2026-07-27).** A conversa lembrou do primeiro turno depois de 16 turnos. Sobram dois itens da T9
que exigem clique: o backfill numa conversa real e o efeito de desligar o toggle.
A conversa aconteceu no app e a memória **não recuperava nada** — duas causas, ambas registradas na
AD-047: a busca pedia exatamente um candidato e o filtro de duplicata rodava depois do corte
(`MEMORY_CANDIDATES = 8` e o filtro antes do `take`), e o histórico podia consumir todo o orçamento
antes da memória (reserva de 15%). **Depois das correções, a mesma pergunta que falhara 3× foi
respondida com "Pantera Cinzenta" e "47 mil reais".** Gates atuais: `cargo test` **174 passando /
12 ignorados** (eram 169/12), `npm run build` limpo, i18n **148/148**.

**Ainda aberto na T9:** o backfill numa conversa real, e o critério *"desligar o toggle faz o modelo
parar de se prender ao que já respondeu"* — desligar foi verificado só do lado da **gravação**
(`vectors/` para de crescer), não do efeito na resposta.

---

## Execution Log (2026-07-27)

| Task | Status | Evidência |
| --- | --- | --- |
| T1 | ✅ | Migração **8**, número conferido contra `MIGRATIONS` por teste em vez de presumido. Um banco em `user_version = 7` com chat e mensagem sobe com `use_memory = 1` e nada perdido. 2 testes novos; a idempotência já era coberta por `applying_migrations_twice_is_idempotent`, que roda a lista inteira duas vezes — e é justamente o que falharia com "duplicate column" se o guard de versão não funcionasse |
| T2 | ✅ | `chat/memory.rs`: namespace, serialização, pareamento, gravação, busca e backfill. 6 testes puros + 2 `#[ignore]` contra LanceDB real |
| T3 | ✅ | `should_record_turn` como função pura, 5 testes. A gravação roda em `spawn` depois do `insert_message` da resposta |
| T4 | ✅ | Vetor da pergunta calculado **uma vez** para as duas camadas; `recall_blocks` extraída para tornar dedup e orçamento testáveis sem `AppHandle`; 6 testes novos |
| T5 | ✅ | `delete_chat` apaga os dois namespaces. O isolamento inteiro (MEM-07/08/09) provado contra LanceDB real |
| T6 | ✅ | `index_chat_history` + evento `memory-backfill-progress`. **Escrito na T2 junto do módulo** — ver o desvio abaixo |
| T7 | ✅ | `use_memory` no `Chat` dos dois lados, segundo interruptor, botão de indexar com progresso, 5 chaves novas em cada idioma (**147/147**) |
| T8 | ✅ | ROADMAP, STATE (AD-044), a rastreabilidade acima, e o prefixo `MEM` na regra `spec-driven-changes.md`, que listava dez e não incluía este |
| T9 | ⏳ Quase | **A pergunta central foi respondida em conversa real (2026-07-27):** numa conversa de **16 turnos completos / 35.220 caracteres**, a pergunta sobre o **primeiro** turno foi respondida **inteira** — codinome *e* valor. As cinco tentativas anteriores da mesma pergunta tinham sido recusadas. Falta o backfill e o efeito de desligar o toggle, que exigem clicar |

### O que foi verificado contra um recurso real, e não deduzido

- **O confinamento por conversa** (a restrição que o usuário acrescentou): dois chats com memória, o
  termo exclusivo de um não chega ao outro, a memória não enxerga os anexos do próprio chat, e
  apagar uma conversa deixa a outra inteira. Rodado com `-- --ignored` contra LanceDB.
- **A idempotência do `doc_id`**: gravar o mesmo turno duas vezes deixa **um** registro. É uma
  afirmação sobre o `upsert` do LanceDB, não sobre o nosso código, então foi medida lá.

### Desvios do plano, com o motivo

1. **O `backfill` foi escrito na T2, não na T6.** Ele e a gravação automática dependem das mesmas
   funções puras e da mesma leitura de mensagens; separá-los teria duplicado código para respeitar
   uma fronteira de task. A T6 ficou com o comando Tauri e o evento, que é o que ela realmente
   acrescenta.
2. **O teste de isolamento ficou em `chat/memory.rs`, não em `commands.rs`.** É o módulo que define
   o namespace, e `commands.rs` não tem módulo de teste. O teste exercita as duas chamadas que o
   `delete_chat` faz — que ele as faça continua sendo código sem teste, e está registrado como tal
   na rastreabilidade (MEM-09 parcial).

### A medição que mudou uma constante (2026-07-27)

A Open Question #1 do design — *"um turno rotulado é bom material de embedding?"* — foi respondida
**antes** da T9, contra o modelo real, por um teste `#[ignore]` que toma os caminhos por variável de
ambiente e roda sobre uma **cópia** do cache de modelos do usuário:

```
LOCALMIND_EMBED_CACHE=<cópia> LOCALMIND_ORT_DYLIB=<onnxruntime.dll> \
  cargo test --lib memory_quality -- --ignored --nocapture
```

| O que foi medido | Resultado |
| --- | --- |
| O turno certo é recuperado? | **Sim** — 0,2484 contra 0,3413 e 0,3805 |
| Os rótulos `Usuário:`/`Assistente:` atrapalham? | **Não** — sem eles a separação **piora** (1,33× contra 1,37×). O plano B do design está descartado |
| O piso relativo filtra irrelevantes? | **Não** — numa pergunta sem relação com a conversa, **os 3 turnos passam o corte** |

**`MEMORY_TOP_K` caiu de 2 para 1** por causa da terceira linha: sem filtro que funcione, o teto é o
único filtro, e um turno irrelevante colado na pergunta é o modo de falha que a AD-033 mediu.
**Nenhum limiar absoluto foi inventado** a partir de três turnos sintéticos — essa decisão é da T9,
com uma conversa real.

### Contagem de testes

De **150 para 169**, com **12 ignorados** contra 9. **+19 novos, nenhum perdido** — nenhuma
justificativa de remoção era necessária desta vez. Dos 3 `#[ignore]` novos, 2 rodaram contra um
LanceDB real e 1 contra o modelo de embedding real; todos foram executados, nenhum ficou só escrito.

**Linha de base a preservar:** `cargo test` **150 passando / 9 ignorados**, `npm run test:scripts`
**43**, `npm run build` limpo. Toda task que reduzir esses números precisa justificar cada teste
perdido — a única justificativa aceita é "o código que ele testava foi removido".

---

## Execution Plan

```
T1 [P] ─┐
        ├─→ T3 ──┬─→ T6 ──┐
T2 [P] ─┼─→ T4 ──┤        ├─→ T7 ──→ T8 ──→ T9
        └─→ T5 ──┘        │
                          │
(T7 depende de T3 e T6; T8 depende de tudo que mudou comportamento)
```

`T1` (migração) e `T2` (módulo novo) não se tocam. `T3`, `T4` e `T5` dependem do módulo; só a `T3`
depende também da coluna. `T7` é o frontend inteiro, que precisa dos dois comandos novos.

---

## Task Breakdown

### T1: Migração 8 — a coluna do toggle [P]

**What:** Oitava entrada da lista de migrações versionadas, adicionando `chats.use_memory`.
**Where:** `src-tauri/src/db.rs`
**Depends on:** nenhuma
**Reuses:** o mecanismo de `PRAGMA user_version` (AD-020) e o molde da migração 6, que também
adicionou coluna com default
**Requirement:** MEM-15, MEM-16

**Done when:**

- [ ] `MIGRATION_8_CHAT_MEMORY` adiciona `use_memory INTEGER NOT NULL DEFAULT 1` em `chats`
- [ ] Um banco `user_version = 7` com chats, mensagens, documentos e anexos sobrevive inteiro, e
      cada chat existente passa a ter `use_memory = 1`
- [ ] Migrar duas vezes é no-op
- [ ] O número é **8** — conferido contra `MIGRATIONS`, não presumido
- [ ] Gate check passa: `cd src-tauri && cargo test db::`
- [ ] Contagem de testes: no mínimo 3 novos (coluna existe com o default certo, dados preservados,
      idempotência)

**Tests:** unit · **Gate:** quick
**Commit:** `feat(db): add the per-chat conversation memory toggle`

---

### T2: O módulo de memória [P]

**What:** `chat::memory` com o namespace, a serialização do turno, o pareamento de mensagens e a
gravação de um turno no banco vetorial.
**Where:** `src-tauri/src/chat/memory.rs` (novo), `src-tauri/src/chat/mod.rs`
**Depends on:** nenhuma
**Reuses:** `rag::store::VectorStore` (sem alteração), `rag::chunking::chunk_text`,
`rag::embedding::embed_passages`, `rag::pipeline::vectors_dir`
**Requirement:** MEM-01, MEM-07, MEM-17, MEM-19, MEM-20

**Done when:**

- [ ] `memory_namespace(chat_id)` devolve `memory:<id>` e **não colide** com `chat:<id>` nem com
      `global` — afirmado por teste, que é o formato do MEM-07
- [ ] `serialize_turn` produz o par rotulado numa string só
- [ ] `pair_turns` reduz uma lista de mensagens aos pares completos user→assistant e **ignora**
      pergunta sem resposta, resposta órfã e dois `user` seguidos
- [ ] `record_turn` fatia com `chunk_text`, embedda e faz `upsert` com `doc_id` = id da mensagem do
      assistente
- [ ] Gravar o mesmo par duas vezes deixa **um** registro (o `upsert` apaga por `doc_id` antes de
      escrever) — verificado no teste `#[ignore]` de LanceDB, não por leitura do código
- [ ] Gate check passa: `cd src-tauri && cargo test chat::memory`
- [ ] Contagem de testes: no mínimo 5 novos (namespace disjunto, serialização, três formatos de
      pareamento), mais 1 `#[ignore]` de idempotência contra LanceDB real

**Tests:** unit · **Gate:** quick
**Commit:** `feat(chat): add the conversation memory module`

---

### T3: Gravar o turno ao fim da geração

**What:** `send_message` passa a registrar o par quando a geração termina bem, e o toggle ganha seu
comando.
**Where:** `src-tauri/src/chat_commands.rs`
**Depends on:** T1, T2
**Reuses:** `set_chat_use_global_rag` (molde exato do comando novo), `CancellationRegistry`,
`rag::pipeline::still_exists` (a ideia, aplicada a `chats`)
**Requirement:** MEM-01, MEM-02, MEM-03, MEM-14, MEM-16

**Done when:**

- [ ] `should_record_turn` existe como **função pura** — recebe (houve texto, houve erro, foi
      cancelado, toggle ligado) e devolve o booleano. É o que torna o MEM-03 testável sem um
      `AppHandle`
- [ ] A gravação roda em `spawn`, depois do `insert_message` da resposta, e o retorno de
      `send_message` não espera por ela (MEM-02)
- [ ] Um chat apagado no meio da geração **não** recebe memória — a existência é conferida antes do
      `upsert` (é o C-14 do CONCERNS, que esta feature não resolve mas não pode piorar)
- [ ] `set_chat_use_memory` registrado no `invoke_handler`
- [ ] Gate check passa: `cd src-tauri && cargo test chat::` e `cargo check`
- [ ] Contagem de testes: no mínimo 4 novos, todos sobre `should_record_turn` (sucesso grava;
      cancelado não; erro não; toggle desligado não)

**Tests:** unit · **Gate:** quick
**Commit:** `feat(chat): record each completed turn as conversation memory`

---

### T4: Recuperar a memória na montagem do contexto

**What:** A terceira camada entra no prompt, por último no orçamento e acima dos documentos.
**Where:** `src-tauri/src/chat/context_assembler.rs`
**Depends on:** T2
**Reuses:** `rank_candidates` (o mesmo piso relativo), `Budget`, `question_with_context`,
`recent_history`
**Requirement:** MEM-04, MEM-05, MEM-06, MEM-08, MEM-10, MEM-11, MEM-12, MEM-13

**Done when:**

- [ ] O vetor da pergunta é calculado **uma vez** e usado nas duas buscas — a memória não paga um
      segundo `embed_query`
- [ ] `recent_history` devolve também o id de cada mensagem, e os turnos de memória cujo `doc_id`
      está nesse conjunto são descartados (MEM-05)
- [ ] A busca de memória consulta **apenas** `memory:<chat_id>` (MEM-08)
- [ ] A memória consome orçamento **depois** de `fit_history` — com orçamento apertado, o que falta
      no prompt é a memória, não o documento nem o turno recente (MEM-10)
- [ ] Teto próprio de turnos, separado do `TOP_K` dos documentos (MEM-12)
- [ ] O bloco de memória entra sob preâmbulo próprio, que **não** manda citar arquivo (MEM-06)
- [ ] Falha na busca de memória vira `retrieval_error`, e a resposta sai mesmo assim (MEM-13)
- [ ] Com o toggle desligado, o prompt montado é **idêntico** ao de hoje — afirmado por teste
- [ ] Gate check passa: `cd src-tauri && cargo test chat::`
- [ ] Contagem de testes: no mínimo 5 novos (dedup pelo histórico verbatim, ordem dos blocos no
      turno final, preâmbulo de memória separado do de documento, orçamento sacrifica a memória
      primeiro, desligado não muda nada)

**Tests:** unit · **Gate:** quick
**Commit:** `feat(chat): retrieve conversation memory when assembling the prompt`

---

### T5: Excluir o chat leva a memória junto

**What:** `delete_chat` passa a apagar dois namespaces.
**Where:** `src-tauri/src/commands.rs`
**Depends on:** T2
**Reuses:** a limpeza de namespace que já existe ali para os anexos
**Requirement:** MEM-09

**Done when:**

- [ ] `delete_chat` apaga `chat:<id>` **e** `memory:<id>`
- [ ] Falha ao apagar o vetorial não impede o chat de sair do banco (comportamento atual preservado)
- [ ] Um teste `#[ignore]` contra LanceDB real prova o isolamento inteiro do MEM-07/08/09: dois
      chats com memória, um termo exclusivo do primeiro não é recuperado pelo segundo, e apagar o
      primeiro deixa o segundo intacto
- [ ] Gate check passa: `cd src-tauri && cargo test` e o `#[ignore]` roda com `-- --ignored`
- [ ] Contagem de testes: 1 novo `#[ignore]`

**Tests:** unit · **Gate:** quick
**Commit:** `feat(chat): drop the conversation memory when a chat is deleted`

---

### T6: Backfill do histórico existente

**What:** Comando que reindexa os pares já gravados de uma conversa, com progresso.
**Where:** `src-tauri/src/chat/memory.rs`, `src-tauri/src/chat_commands.rs`
**Depends on:** T2, T3
**Reuses:** `pair_turns` (T2), `record_turn` (T2), o molde de evento de `DocumentStatusEvent`
**Requirement:** MEM-17, MEM-18, MEM-19, MEM-20

**Done when:**

- [ ] `index_chat_history(chat_id)` lê as mensagens em ordem, pareia e grava cada par
- [ ] O progresso chega por evento `memory-backfill-progress` com `{chat_id, done, total}`
- [ ] Rodar duas vezes no mesmo chat não duplica (garantido pelo `doc_id` da T2)
- [ ] Conversa sem nenhum par completo termina informando isso, sem erro
- [ ] Com a memória desligada, o comando recusa nomeando o toggle
- [ ] O chat apagado no meio interrompe o backfill sem erro
- [ ] Gate check passa: `cd src-tauri && cargo check` e `cargo test chat::`
- [ ] Contagem de testes: os de `pair_turns` da T2 já cobrem a parte pura; nenhum novo obrigatório
      (camada de comando — a matriz da TESTING.md diz "none")

**Tests:** none (o núcleo puro foi testado na T2) · **Gate:** build
**Commit:** `feat(chat): index the existing conversation history on demand`

---

### T7: Frontend — toggle, botão e textos

**What:** O segundo interruptor no chat, o botão de indexar histórico com progresso, e as chaves de
i18n nos dois idiomas.
**Where:** `src/types.ts`, `src/lib/chatApi.ts`, `src/store/chatStore.ts`,
`src/components/Chat/ChatPanel.tsx`, `src/i18n/locales/{en,pt}.json`
**Depends on:** T3, T6
**Reuses:** o toggle de `use_global_rag` inteiro — mesma forma, mesmo store, mesmo padrão otimista
**Requirement:** MEM-14, MEM-16, MEM-18

**Done when:**

- [ ] `Chat` ganha `use_memory: boolean` em `types.ts` (espelho manual — C-03)
- [ ] O interruptor reflete e grava o estado, com o mesmo update otimista do outro
- [ ] O botão de indexar histórico mostra progresso e volta ao estado normal ao terminar
- [ ] Chaves novas em EN **e** PT, com contagem igual entre os dois arquivos
- [ ] Gate check passa: `npm run build`, e a contagem de chaves EN é igual à de PT

**Tests:** none (componentes React e TS — a matriz diz "none") · **Gate:** build
**Commit:** `feat(ui): add the conversation memory toggle and history indexing`

---

### T8: Documentação

**What:** ROADMAP, STATE e os documentos de codebase param de descrever o M6 como inexistente.
**Where:** `.specs/project/ROADMAP.md`, `.specs/project/STATE.md`,
`.specs/codebase/{ARCHITECTURE,STRUCTURE,TESTING}.md`, `.claude/rules/spec-driven-changes.md`
**Depends on:** T1–T7
**Requirement:** —

**Done when:**

- [ ] O M6 deixa de ser "📭 SEM SPEC, SEM CÓDIGO" no ROADMAP
- [ ] O prefixo `MEM` entra na lista de prefixos em uso da regra `spec-driven-changes.md` — hoje ela
      lista dez e não inclui este
- [ ] A tabela de rastreabilidade da spec reflete o que ficou **verificado** e o que ficou pendente
- [ ] Uma AD nova em `STATE.md` registra as três decisões do `context.md` e os desvios de plano
- [ ] Gate check: revisão manual (documentação não tem gate automatizado)

**Tests:** none · **Gate:** none
**Commit:** `docs: describe the conversation memory milestone`

---

### T9: Verificação de ponta a ponta ⚠️ EXIGE O USUÁRIO

**What:** O gate real da feature. Nenhum teste automatizado responde "a conversa lembrou?".
**Where:** — (verificação manual)
**Depends on:** T7, T8
**Requirement:** todos os critérios de sucesso

**Done when:**

- [x] Numa conversa com mais turnos do que o orçamento comporta, uma pergunta sobre o primeiro turno
      é respondida corretamente — **com o número de turnos e o orçamento registrados**.
      **Feito em 2026-07-27** — ver "A recuperação em conversa real", abaixo
- [x] O backfill roda numa conversa real e a pergunta sobre o turno mais antigo é respondida —
      **feito em 2026-07-27**, ver "O backfill e o toggle, medidos em A/B" abaixo
- [x] Desligar o toggle faz o modelo parar de se prender ao que já respondeu — **feito em
      2026-07-27**, na mesma sessão e com a mesma pergunta
- [x] O custo de armazenamento é **medido** contra o `vectors/` real (Open Question #3 do design) —
      **~9,5 KB por turno** (AD-047); confirmado de novo pelo backfill de 2026-07-27, que gravou
      **+133.963 bytes para 12 turnos (≈11,2 KB/turno)**
- [x] A Open Question #1 (o turno rotulado é bom material de embedding?) recebe uma resposta medida —
      **sim**, e os rótulos ajudam (1,37× com eles contra 1,33× sem). Rodado de novo em 2026-07-27
- [x] Gate check passa: `cargo test` **174 / 0 falhas / 13 ignorados**, `npm run build` limpo,
      `npm run test:scripts` **49**

### O backfill e o toggle, medidos em A/B (2026-07-27)

Os dois critérios que faltavam foram fechados **dirigindo a UI de verdade** — o app aberto com o
debug remoto do WebView2 exposto, e cada ação despachada como evento DOM na página que o usuário vê,
não por `invoke` direto. Um `invoke` provaria o backend e não provaria a tela.

**A primeira tentativa reprovou por erro meu de método, e o registro fica porque o erro é
instrutivo.** Eu fiz a mesma pergunta duas vezes na mesma conversa — uma com a memória desligada,
outra com ela ligada — e a resposta errada da primeira virou o turno imediatamente anterior à
segunda. O modelo repetiu a si mesmo: *"Flor do Abacate"* virou *"Flor do Abacão"*. É a AD-033
acontecendo dentro do próprio experimento. A pergunta passou a ser feita **uma única vez por
conversa**, e as duas leituras foram para conversas separadas.

Desenho final: 12 turnos, o fato plantado no turno 1 e empurrado para fora da janela verbatim pelos
11 seguintes (`RECENT_HISTORY_LIMIT` são **20 mensagens**; com 24 na conversa, o turno 1 está fora).
A pergunta é a mesma nas duas, palavra por palavra, e não repete nenhum termo da resposta guardada.

| Conversa | Memória durante | Toggle na pergunta | `vectors/` | Resposta |
| --- | --- | --- | --- | --- |
| A | **desligada** | ligada + backfill | +0 B em 12 turnos, depois **+133.963 B** no backfill | **"Falcão Azul" … "82 mil reais"** ✅ |
| B | **ligada** | **desligada** | **+190.814 B** durante a conversa | *"não tenho a capacidade de lembrar interações ou conversas anteriores"* ✅ |

Os dois lados são fortes pelo mesmo motivo: em B a memória **existia** no banco vetorial — foi
gravada turno a turno — e mesmo assim não foi recuperada. O que suprimiu foi o toggle, não a
ausência de dado. E em A o `vectors/` não cresceu **um byte** ao longo de 12 turnos com o toggle
desligado, o que fecha o MEM-14 pelo lado da gravação sem depender de leitura de código.

O backfill em si: **12 turnos em ~1,6 s**, com o progresso lido na tela (`Indexando histórico…
(3/15)` numa execução de 15 turnos) — o que fecha o MEM-18 por observação e não por "o evento é
emitido". A linha de resultado apareceu como `12 turno(s) adicionado(s) à memória desta conversa`.

**Um defeito real saiu daqui, e ele não é do M6:** as duas conversas acima rodaram com *"usar meus
documentos"* **desligado**. Com ele ligado — e com um único PDF do Código Civil na base, que nada
tem a ver com a pergunta — a mesma pergunta na mesma forma de conversa foi respondida com
*"Projeto de Código Civil Brasileiro 115 … R$ 250.000,00"*. Ver AD-050.

### A recuperação em conversa real (2026-07-27)

O gate desta feature era "a conversa lembrou?", e a resposta agora é **sim**, lida do banco e não
inferida. A conversa `7e0ec8bc` tem **16 turnos completos** e **35.220 caracteres** de histórico —
o orçamento do prompt é 78.848 caracteres com `n_ctx_slot = 21760`, então o histórico **cabia**, e
a memória estava sendo exercitada pelo motivo certo (relevância), não por transbordo.

O primeiro turno plantou o dado:

> *"Guarde este dado: o codinome do meu projeto e Pantera Cinzenta e o valor liberado foi 47 mil
> reais."*

A mesma pergunta foi feita **seis vezes**. As cinco primeiras (16:14 → 16:42) foram recusadas — a
falha que a AD-047 diagnosticou. A sexta, às **16:51:52**:

> *"Com base nas informações que você forneceu anteriormente em nossa conversa, você batizou o seu
> projeto com o apelido "Pantera Cinzenta" e mencionou que 47 mil reais foram liberados."*

**Os dois fatos, corretos.** Isso é o desfecho; o mecanismo por trás dele foi medido à parte, pelo
`a_rephrased_question_still_reaches_the_turn_it_is_asking_about` contra o modelo real:

```
#0 0.3032  Usuário: voltando ao início: com que apelido eu batizei o projeto? <- isca (já citada verbatim)
#1 0.3158  Usuário: vou te dar um codinome pra esse projeto: chama ele de Albatroz daqui pra frente <- plantado
posição do plantado: #1 · da isca: #0 · MEMORY_CANDIDATES = 8, MEMORY_TOP_K = 1
```

A isca fica em **#0**. Com o funil antigo — `search` pedindo exatamente `MEMORY_TOP_K = 1` e o
filtro de verbatim rodando **depois** do corte — ela consumia a única vaga e era descartada,
sobrando zero. É a razão pela qual reformular a pergunta de forma natural *piorava* a recuperação.

**O que continua aberto, e não é detalhe:** o backfill nunca rodou numa conversa de verdade, e o
efeito de **desligar** o toggle sobre a resposta não foi observado. Os dois exigem cliques na UI.
O `MEMORY_TOP_K = 1` também segue sem justificativa medida — uma conversa em que dois turnos
antigos importam ao mesmo tempo continua sem ter sido testada.

**Tests:** none (UAT) · **Gate:** full
**Commit:** —

---

## Task Granularity Check

| Task | Escopo | Status |
| --- | --- | --- |
| T1 | 1 migração | ✅ Granular |
| T2 | 1 módulo novo | ✅ Granular |
| T3 | 1 comando + 1 função pura | ✅ Granular |
| T4 | 1 arquivo, uma camada nova no prompt | ⚠️ OK — coeso: as 3 mudanças servem a mesma recuperação |
| T5 | 1 função | ✅ Granular |
| T6 | 1 comando | ✅ Granular |
| T7 | 1 tela + tipos + textos | ⚠️ OK — uma tela é a unidade demonstrável |
| T8 | Só documentação | ⚠️ OK — indivisível na prática |
| T9 | Verificação manual | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends on (corpo) | Diagrama mostra | Status |
| --- | --- | --- | --- |
| T1 | — | sem seta de entrada | ✅ |
| T2 | — | sem seta de entrada | ✅ |
| T3 | T1, T2 | T1→T3, T2→T3 | ✅ |
| T4 | T2 | T2→T4 | ✅ |
| T5 | T2 | T2→T5 | ✅ |
| T6 | T2, T3 | T3→T6 | ✅ |
| T7 | T3, T6 | T6→T7 | ✅ |
| T8 | T1–T7 | T7→T8 | ✅ |
| T9 | T7, T8 | T8→T9 | ✅ |

---

## Test Co-location Validation

| Task | Camada criada/modificada | Matriz exige | Task diz | Status |
| --- | --- | --- | --- | --- |
| T1 | Migração de schema | unit | unit | ✅ |
| T2 | Funções puras + I/O vetorial | unit | unit (+1 `#[ignore]`) | ✅ |
| T3 | Comando Tauri + 1 função pura | unit (a mais alta das duas) | unit | ✅ |
| T4 | Montagem de contexto (pura) | unit | unit | ✅ |
| T5 | Comando + I/O vetorial | unit onde há função; `#[ignore]` para o recurso real | unit | ✅ |
| T6 | Comando Tauri (orquestração) | none | none | ✅ |
| T7 | React + TS | none | none | ✅ |
| T8 | Documentação | none | none | ✅ |
| T9 | UAT | none | none | ✅ |

---

## Gate Check Commands

| Gate | Comando |
| --- | --- |
| quick (Rust) | `cd src-tauri && cargo test <módulo>::` |
| build (Rust) | `cd src-tauri && cargo check` |
| build (frontend) | `npm run build` |
| full | `cargo test` completo + o app aberto, conversando |
