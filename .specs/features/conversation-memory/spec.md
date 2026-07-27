# Memória de conversa (RAG híbrido) — Specification

**Milestone:** M6
**Contexto das decisões:** `.specs/features/conversation-memory/context.md`

## Problem Statement

A conversa esquece tudo o que não cabe na janela de contexto. Hoje o `context_assembler` leva no
máximo as 20 últimas mensagens e, quando o orçamento aperta, `fit_history` descarta as mais antigas
— o que sai do prompt deixa de existir para o modelo, mesmo estando gravado no SQLite. Numa conversa
longa, o usuário precisa repetir o que já disse.

As outras duas camadas de RAG (documentos globais e anexos do chat) já existem e funcionam. Falta a
terceira, que a AD-009 decidiu em 2026-07-24 e nunca foi construída: a própria conversa como fonte
recuperável.

## Goals

- [ ] Perguntar sobre algo dito no começo de uma conversa longa e receber a resposta certa, com o
      turno original fora da janela verbatim — verificado numa conversa cujo histórico não cabe no
      orçamento
- [ ] A memória de uma conversa **nunca** aparece em outra — provado por teste, não por construção
- [ ] A camada nova não desloca nem documento nem turno recente do prompt: ela ocupa o que sobra

## Out of Scope

| Item | Motivo |
| --- | --- |
| Resumir a conversa com o LLM (summarization memory) | Custa uma geração extra por turno num app que roda o modelo na máquina do usuário. A recuperação por embedding reusa o motor que já está carregado |
| Memória entre conversas ("o que falamos ontem no outro chat") | Contradiz diretamente o confinamento pedido pelo usuário (MEM-07) |
| Editar ou apagar um turno específico da memória | A unidade de gestão é a conversa: apagar o chat apaga a memória. Curadoria turno a turno é UI para um problema que ainda não apareceu |
| Backfill automático no boot | Recusado na decisão 2 do `context.md`: CPU de embedding no primeiro boot depois do update se parece com travamento |
| Memória de anexos e documentos | Já existem, são as camadas 1 e 2 |

---

## User Stories

### P1: A conversa lembra do que saiu da janela ⭐ MVP

**User Story**: Como usuário, quero que o chat lembre do que foi dito muito antes na mesma conversa,
para não precisar repetir o contexto a cada mensagem.

**Why P1**: É a feature. Sem isto, o M6 não existe.

**Acceptance Criteria**:

1. WHEN uma geração termina com uma resposta completa THEN o sistema SHALL serializar o par
   (mensagem do usuário + resposta do assistente) e indexá-lo na memória daquela conversa
2. WHEN o par é indexado THEN isso SHALL acontecer **depois** de a resposta ser persistida e sem
   atrasar o retorno de `send_message`
3. WHEN a geração é cancelada ou falha THEN o sistema SHALL **não** gravar nada na memória — um
   turno pela metade recuperado depois vira uma resposta truncada apresentada como fato
4. WHEN uma mensagem é enviada THEN o sistema SHALL recuperar os turnos antigos mais relevantes da
   memória daquela conversa e incluí-los no contexto
5. WHEN um turno recuperado já está no histórico verbatim que vai no mesmo prompt THEN o sistema
   SHALL descartá-lo da recuperação, em vez de enviá-lo duas vezes
6. WHEN um turno de memória entra no prompt THEN ele SHALL ser rotulado como conversa anterior, e
   não como documento — o modelo não pode citá-lo como fonte de arquivo

**Independent Test**: numa conversa com mais turnos do que o orçamento comporta, perguntar sobre algo
dito no primeiro turno e receber a informação correta.

---

### P1: A memória de uma conversa não sai dela ⭐ MVP

**User Story**: Como usuário, quero que o que eu disse num chat fique naquele chat, para poder tratar
conversas diferentes como assuntos separados.

**Why P1**: Restrição explícita do usuário (`context.md`, decisão 1). É também a classe de defeito
que a AD-040 encontrou na CHAT-11: um isolamento que parecia garantido pelo namespace e não estava.

**Acceptance Criteria**:

1. WHEN a memória de um chat é gravada THEN ela SHALL usar um namespace vetorial exclusivo daquela
   conversa, distinto do namespace dos anexos do mesmo chat e do namespace global
2. WHEN qualquer chat monta contexto THEN a recuperação de memória SHALL consultar **apenas** o
   namespace da própria conversa
3. WHEN um chat é excluído THEN o sistema SHALL apagar a memória dele junto com os anexos, sem
   tocar em nenhuma outra conversa

**Independent Test**: gravar memória em dois chats, buscar por um termo que só existe no primeiro e
confirmar que o segundo não o recupera; excluir o primeiro e confirmar que o segundo continua
inteiro.

---

### P1: Orçamento entre as três camadas ⭐ MVP

**User Story**: Como usuário, quero que a memória ajude sem prejudicar o que já funciona — os
documentos que importei e a continuidade da conversa recente.

**Why P1**: A AD-033 mediu o que acontece quando o prompt é montado na ordem errada: com o documento
a ~10 mil caracteres da pergunta, o modelo copiou as próprias respostas anteriores em vez da fonte.
Uma camada nova mal posicionada reintroduz exatamente esse defeito.

**Acceptance Criteria**:

1. WHEN o contexto é montado THEN a memória SHALL consumir apenas o orçamento que sobra depois dos
   trechos de documento/anexo e do histórico recente
2. WHEN o orçamento se esgota antes da memória THEN a mensagem SHALL ser enviada sem memória, sem
   erro — a camada é um acréscimo, não um pré-requisito
3. WHEN a memória tem muitos turnos relevantes THEN o sistema SHALL respeitar um teto próprio de
   turnos recuperados, independente do teto dos documentos
4. WHEN a recuperação de memória falha (banco vetorial ou modelo de embedding indisponível) THEN a
   resposta SHALL sair mesmo assim e o usuário SHALL ser avisado de que a memória não entrou

**Independent Test**: com um orçamento apertado, confirmar que os trechos de documento e os turnos
recentes continuam no prompt e que a memória é o que fica de fora.

---

### P1: Ligar e desligar por conversa ⭐ MVP

**User Story**: Como usuário, quero desligar a memória num chat específico, para pedir versões
diferentes da mesma coisa sem o modelo se prender ao que já respondeu.

**Why P1**: Pedido do usuário (`context.md`, decisão 1), e a contrapartida honesta do risco medido na
AD-033 — o modelo desta base **tem** a tendência de imitar as próprias respostas anteriores.

**Acceptance Criteria**:

1. WHEN o usuário desliga a memória num chat THEN o sistema SHALL parar de recuperar **e** de gravar
   naquela conversa
2. WHEN um chat é criado THEN a memória SHALL vir ligada
3. WHEN o app é reaberto THEN o estado do toggle SHALL ser o que o usuário deixou, por conversa

**Independent Test**: desligar num chat, trocar de chat e voltar; o interruptor continua desligado e
nenhum turno novo foi indexado nele.

---

### P2: Indexar o histórico existente, sob demanda

**User Story**: Como usuário, quero mandar o app indexar as conversas que já tenho, para que a
memória valha também para elas.

**Why P2**: A conversa começa a ganhar memória sozinha a partir da próxima mensagem; sem isto o
histórico antigo fica de fora para sempre, mas o app funciona.

**Acceptance Criteria**:

1. WHEN o usuário pede para indexar o histórico de uma conversa THEN o sistema SHALL embeddar os
   pares pergunta+resposta já gravados naquele chat
2. WHEN o backfill roda THEN o progresso SHALL chegar à UI por evento, como o de indexação de
   documento
3. WHEN o backfill roda uma segunda vez no mesmo chat THEN ele SHALL substituir o que já indexou, e
   não duplicar
4. WHEN a conversa não tem nenhum par completo THEN o backfill SHALL terminar informando isso, sem
   erro

**Independent Test**: rodar o backfill numa conversa antiga, perguntar sobre o primeiro turno dela e
receber a resposta certa.

---

## Edge Cases

- WHEN um turno é maior que o limite do modelo de embedding THEN o sistema SHALL fatiá-lo com o
  mesmo chunking dos documentos, em vez de descartá-lo ou truncá-lo em silêncio
- WHEN o chat é excluído durante o backfill THEN o processamento SHALL parar sem gravar nada e sem
  erro visível
- WHEN a conversa está na primeira mensagem THEN não há memória a recuperar e o prompt SHALL sair
  igual ao de hoje
- WHEN a resposta foi cancelada mas deixou texto parcial na tela THEN esse texto SHALL ficar no
  histórico (CHAT-04, comportamento atual) e **fora** da memória
- WHEN o mesmo par é indexado de novo (backfill depois da gravação automática) THEN o resultado SHALL
  ser um único registro, não dois
- WHEN a memória está desligada e o usuário roda o backfill THEN o comando SHALL recusar nomeando o
  toggle, em vez de indexar dados que nunca serão lidos

---

## Requirement Traceability

Atualizada em 2026-07-27, depois da execução das T1–T8. **"Verificado" aqui significa exercitado
contra um recurso real**; "Implementado" significa escrito e coberto por teste unitário, mas nunca
exercitado num app aberto — a diferença é o que a T9 fecha.

| Requirement ID | Story | Tasks | Status |
| --- | --- | --- | --- |
| MEM-01 | P1: Lembra do que saiu da janela | T2, T3 | ✅ **Verificado no app (2026-07-27)** — 9 turnos reais gravados numa conversa, `vectors/` crescendo ~9,5 KB por turno; com o toggle desligado o crescimento para em zero, o que prova que era a memória escrevendo |
| MEM-02 | P1: Lembra do que saiu da janela | T3 | ⚠️ Implementado — roda em `spawn`; o efeito na latência **não foi medido** |
| MEM-03 | P1: Lembra do que saiu da janela | T2, T3 | ✅ Implementado — `should_record_turn` e `pair_turns` cobrem as quatro vias, 9 testes |
| MEM-04 | P1: Lembra do que saiu da janela | T4 | ✅ **Verificado no app (2026-07-27)** — numa conversa de 32 mensagens (fora das 20 do `RECENT_HISTORY_LIMIT`, ou seja, o turno plantado não estava no prompt verbatim), a pergunta *"com que apelido eu batizei o trabalho, e quanto dinheiro eu disse que tinha sido liberado?"* — sem uma palavra em comum com o turno guardado — foi respondida com **"Pantera Cinzenta"** e **47 mil reais**. A mesma pergunta falhara 3× antes das correções da AD-047 |
| MEM-05 | P1: Lembra do que saiu da janela | T4 | ✅ **Verificado** — `recall_blocks`, 4 testes. A dedup estava **correta e mortal**: aplicada depois do corte para 1 candidato, descartava a única vaga sempre que o vizinho mais próximo era a própria pergunta repetida (AD-047). Hoje o filtro roda antes do `take` |
| MEM-06 | P1: Lembra do que saiu da janela | T4 | ✅ Implementado — preâmbulo e marcador próprios, 1 teste |
| MEM-07 | P1: Não sai da conversa | T2, T5 | ✅ **Verificado** contra LanceDB real |
| MEM-08 | P1: Não sai da conversa | T2, T4 | ✅ **Verificado** contra LanceDB real |
| MEM-09 | P1: Não sai da conversa | T5 | ⚠️ **Parcial** — as duas chamadas de `delete_namespace` foram verificadas contra LanceDB real; que o `delete_chat` faça as duas é código sem teste (precisa de `AppHandle`) |
| MEM-10 | P1: Orçamento entre as camadas | T4 | ✅ Implementado — a memória é servida depois de `fit_history`, mas com **15% do orçamento reservado** antes dele e devolvido em seguida, para que "o que sobra" nunca seja zero (AD-047). 4 testes |
| MEM-11 | P1: Orçamento entre as camadas | T4 | ✅ Implementado — orçamento zerado devolve lista vazia, sem erro |
| MEM-12 | P1: Orçamento entre as camadas | T4 | ✅ Implementado — `MEMORY_TOP_K` separado do `TOP_K`, e **reduzido de 2 para 1** depois de medir que o piso de relevância não filtra nada nesta camada (Open Question #1 do design) |
| MEM-13 | P1: Orçamento entre as camadas | T4 | ⚠️ Implementado — reusa o `retrieval_error`, que já existia; o caminho de falha **não foi provocado** |
| MEM-14 | P1: Ligar e desligar por conversa | T1, T3, T7 | ✅ Implementado — desligado não grava (teste) e não recupera (código) |
| MEM-15 | P1: Ligar e desligar por conversa | T1 | ✅ Implementado — default da coluna, 2 testes de migração |
| MEM-16 | P1: Ligar e desligar por conversa | T1, T3, T7 | ✅ **Verificado no app (2026-07-27)** — o interruptor foi clicado numa conversa real e a gravação parou: `vectors/` ficou em 5.748.117 bytes antes e depois do turno seguinte, byte a byte |
| MEM-17 | P2: Backfill sob demanda | T2, T6 | ⚠️ Implementado — `pair_turns` testado; o comando nunca rodou |
| MEM-18 | P2: Backfill sob demanda | T6, T7 | ⚠️ Implementado — evento emitido e ouvido; **nenhum clique** |
| MEM-19 | P2: Backfill sob demanda | T2 | ✅ **Verificado** contra LanceDB real — reindexar deixa um registro |
| MEM-20 | P2: Backfill sob demanda | T2, T7 | ✅ Implementado — conversa sem par completo devolve 0, 2 testes |

**Mapa ID → critério:**

| ID | O que afirma |
| --- | --- |
| MEM-01 | Turno completo vira memória ao fim da geração |
| MEM-02 | A indexação não atrasa a resposta |
| MEM-03 | Geração cancelada ou com erro não vira memória |
| MEM-04 | Recuperação dos turnos antigos relevantes da própria conversa |
| MEM-05 | Turno já presente no histórico verbatim não é recuperado de novo |
| MEM-06 | Bloco de memória é rotulado como conversa, não como documento |
| MEM-07 | Namespace exclusivo por conversa, distinto do de anexos e do global |
| MEM-08 | A recuperação de memória só consulta a própria conversa |
| MEM-09 | Excluir o chat apaga a memória junto |
| MEM-10 | A memória consome só o que sobra depois de documentos e histórico |
| MEM-11 | Orçamento esgotado antes da memória não é erro |
| MEM-12 | Teto próprio de turnos recuperados |
| MEM-13 | Falha de memória avisa e não bloqueia a resposta |
| MEM-14 | Desligado para de recuperar **e** de gravar |
| MEM-15 | Chat novo nasce com memória ligada |
| MEM-16 | O toggle persiste por conversa |
| MEM-17 | Backfill embedda os pares já gravados de um chat |
| MEM-18 | Progresso do backfill por evento |
| MEM-19 | Backfill repetido substitui, não duplica |
| MEM-20 | Conversa sem par completo termina o backfill sem erro |

**Status values:** Pending → In Design → In Tasks → Implementing → Verified
**Coverage:** 20 no total, 20 mapeados para tasks, 0 sem task. **3 verificados contra recurso real,
16 implementados com teste unitário, 1 parcial.** Nenhum verificado clicando — é o que a T9 responde.

---

## Success Criteria

- [ ] Numa conversa cujo histórico **não cabe** no orçamento, uma pergunta sobre o primeiro turno é
      respondida corretamente — e o número de turnos e o tamanho do orçamento são registrados, não
      estimados
- [ ] Um termo exclusivo do chat A não é recuperado pelo chat B, em teste automatizado
- [ ] `cargo test` acima da linha de base de 150, com cada teste perdido justificado
- [ ] Com a memória desligada, o prompt montado é byte a byte o mesmo de hoje
- [ ] O backfill de uma conversa real termina e a pergunta sobre o turno mais antigo é respondida
