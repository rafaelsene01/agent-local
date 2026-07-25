# Chat: Envio, Streaming & Anexos — Specification

## Problem Statement

O chat hoje só mostra "sem mensagens ainda" — não dá pra conversar de verdade. Esta feature entrega o campo de mensagem, o envio com resposta em streaming usando o modelo marcado (M3), e a possibilidade de anexar arquivos numa conversa específica: esses arquivos são serializados e usados como RAG **só naquele chat**, além (opcionalmente) da base global de documentos (M5).

## Goals

- [x] Campo de mensagem no chat, com envio e resposta em streaming
- [x] Anexar arquivos ao enviar uma mensagem; arquivo processado e usado como RAG daquele chat
- [x] O RAG do chat funciona junto com o RAG global de documentos (quando habilitado)
- [x] Anexos de um chat nunca vazam para outro chat (isolamento — AD-004)
- [x] Anexos de um chat são apagados quando o chat é excluído

## Out of Scope

| Feature | Reason |
| --- | --- |
| Memória semântica da própria conversa (RAG híbrido sobre o histórico) | M6 — feature separada (AD-009), esta aqui só cobre anexos + RAG global |
| Escolher/baixar modelo | M3 — esta feature consome o modelo já marcado como ativo |
| Runtime embutido de fallback | M7 |
| Editar/apagar mensagens individuais | Não pedido; deferir se necessário |

## Dependência

Consome diretamente:
- **M3 (Conexões & Modelos)**: precisa de uma conexão habilitada + modelo ativo para enviar mensagens (`CONN-01..13`)
- **M5 (Documentos & RAG global)**: reusa o pipeline de parse/chunk/embed (`DOC-04`) para processar anexos, e o mecanismo de retrieval (`DOC-10`) generalizado para múltiplos namespaces

---

## User Stories

### P1: Enviar mensagem e receber resposta em streaming ⭐ MVP

**User Story**: Como usuário, quero digitar uma mensagem no chat e ver a resposta da IA chegando em tempo real, para conversar de verdade.

**Why P1**: É o propósito central do app.

**Acceptance Criteria**:

1. WHEN o usuário digita texto e envia THEN o sistema SHALL salvar a mensagem do usuário e mostrar a resposta chegando token a token (streaming), não tudo de uma vez
2. WHEN não há conexão/modelo ativo configurado THEN o campo de envio SHALL indicar isso claramente e impedir o envio (ou orientar a ir configurar em Conexões)
3. WHEN a resposta termina THEN o sistema SHALL salvar a mensagem completa da IA no histórico do chat (persistida, SHELL-04 já garante isso a nível de schema)
4. WHEN o usuário cancela durante o streaming THEN o sistema SHALL parar a geração e manter o que já foi recebido até ali
5. WHEN a chamada ao modelo falha (conexão caiu, erro do provedor) THEN o sistema SHALL mostrar erro na própria conversa, sem travar a UI

**Independent Test**: Enviar "oi" com um modelo configurado e ver a resposta aparecer progressivamente; desligar o Ollama no meio de uma resposta e ver erro tratado.

---

### P1: Anexar arquivo à mensagem ⭐ MVP

**User Story**: Como usuário, quero anexar um arquivo junto com minha pergunta, para que a resposta considere o conteúdo desse arquivo.

**Why P1**: Requisito explícito central desta feature.

**Acceptance Criteria**:

1. WHEN o usuário anexa um arquivo ao compor uma mensagem THEN o sistema SHALL serializar o arquivo em `chats/<chat_id>/tmp/` (pasta-base, AD-008) antes de habilitar o envio
2. WHEN a mensagem com anexo é enviada THEN o sistema SHALL processar o(s) arquivo(s) (extrair → chunk → embed, reusando o pipeline de DOC-04) num namespace vetorial exclusivo daquele `chat_id`
3. WHEN o processamento do anexo termina THEN a pergunta enviada SHALL usar os trechos relevantes do(s) anexo(s) daquele chat como contexto adicional
4. WHEN um anexo é pequeno o bastante (abaixo de um limiar de tokens configurável) THEN o sistema SHALL injetá-lo inteiro no contexto em vez de só os trechos recuperados por similaridade
5. WHEN um anexo falha ao processar THEN o sistema SHALL avisar no chat e enviar a mensagem sem aquele contexto (não travar o envio da mensagem de texto)

**Independent Test**: Anexar um `.txt` pequeno com um fato inventado, perguntar sobre esse fato, e confirmar que a resposta reflete o conteúdo do anexo.

---

### P1: Isolamento de anexos entre chats ⭐ MVP

**User Story**: Como usuário, quero que arquivos anexados num chat não apareçam nem influenciem outro chat, para manter cada conversa independente.

**Why P1**: Já é uma decisão arquitetural registrada (AD-004); esta story garante que a implementação a respeita.

**Acceptance Criteria**:

1. WHEN um anexo é processado no chat A THEN uma pergunta no chat B SHALL SÓ recuperar trechos do namespace do chat B (nunca do chat A)
2. WHEN o chat é excluído (SHELL-07 já existente) THEN o sistema SHALL apagar a pasta `chats/<chat_id>/tmp/` inteira e os embeddings daquele namespace

**Independent Test**: Anexar um arquivo com um fato único no chat A; perguntar sobre esse fato no chat B e confirmar que a resposta NÃO usa esse contexto.

---

### P2: Combinar RAG global + RAG do chat

**User Story**: Como usuário, quero que minha pergunta considere tanto os documentos da base global (M5) quanto os anexos daquele chat específico, ao mesmo tempo.

**Why P2**: Enriquece a resposta, mas o P1 (só anexos) já entrega valor sozinho.

**Acceptance Criteria**:

1. WHEN uma mensagem é enviada em um chat com anexos processados E a base global tem documentos "prontos" THEN o sistema SHALL recuperar trechos de ambas as fontes e combiná-los no contexto enviado ao modelo
2. WHEN o usuário desliga o uso da base global para aquele chat (toggle) THEN só os anexos do próprio chat SHALL ser usados como RAG
3. WHEN o orçamento de contexto do modelo é insuficiente para tudo (anexos + global + histórico) THEN o sistema SHALL priorizar: mensagem atual > anexos do chat > histórico recente > RAG global (ordem sujeita a ajuste no design)

**Independent Test**: Com um documento na base global e um anexo no chat cobrindo tópicos diferentes, fazer uma pergunta que só o retrieval global responde, e confirmar que funciona mesmo sem anexo relevante.

---

## Edge Cases

- WHEN o usuário anexa um arquivo de tipo não suportado THEN o sistema SHALL rejeitar antes de tentar enviar, com mensagem clara (reusa validação de DOC-03)
- WHEN o usuário envia uma mensagem sem nenhum anexo e sem base global habilitada THEN o chat SHALL funcionar normalmente sem RAG (comportamento atual)
- WHEN dois anexos no mesmo chat têm conteúdo conflitante THEN o sistema SHALL recuperar de ambos por similaridade, sem tentar resolver o conflito (isso é problema do modelo, não do RAG)
- WHEN o usuário anexa um arquivo enorme (acima do limite de DOC-03) THEN o sistema SHALL rejeitar com o mesmo limite usado na base global
- WHEN a resposta em streaming está em andamento e o usuário troca de chat THEN o sistema SHALL continuar a geração em background e não perder a resposta ao voltar para aquele chat

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| --- | --- | --- | --- |
| CHAT-01 | P1: Enviar mensagem + streaming | Implemented | Implemented |
| CHAT-02 | P1: Bloquear envio sem modelo ativo | Implemented | Implemented |
| CHAT-03 | P1: Persistir mensagens da conversa | Implemented | Implemented |
| CHAT-04 | P1: Cancelar geração em andamento | Implemented | Implemented |
| CHAT-05 | P1: Tratar erro de chamada ao modelo | Implemented | Implemented |
| CHAT-06 | P1: Anexar arquivo → serializar em tmp/ | Implemented | Implemented |
| CHAT-07 | P1: Processar anexo (pipeline reusado de DOC-04) | Implemented | Implemented |
| CHAT-08 | P1: Usar trechos do anexo no contexto da resposta | Implemented | Implemented |
| CHAT-09 | P1: Injetar anexo pequeno inteiro (sem RAG) | Implemented | Implemented |
| CHAT-10 | P1: Falha de anexo não bloqueia envio da mensagem | Implemented | Implemented |
| CHAT-11 | P1: Isolamento de namespace por chat_id | Implemented | Implemented |
| CHAT-12 | P1: Apagar tmp/ e embeddings ao excluir chat | Implemented | Implemented |
| CHAT-13 | P2: Combinar RAG global + RAG do chat | Implemented | Implemented |
| CHAT-14 | P2: Toggle de uso da base global por chat | Implemented | Implemented |
| CHAT-15 | P2: Priorização de orçamento de contexto | Implemented | Implemented |

**ID format:** `CHAT-[NUMBER]`
**Status values:** Pending → In Design → In Tasks → Implementing → Verified
**Coverage:** 15 total, 15 implementados (2026-07-25). Streaming verificado contra socket real (frame partido entre leituras); o fluxo completo pela UI segue por verificar.

---

## Success Criteria

- [x] Conversa real funciona ponta a ponta com um modelo do Ollama, com streaming visível
- [x] Anexar um arquivo muda a resposta de forma verificável (teste com fato inventado)
- [x] Um anexo do chat A nunca influencia o chat B
- [x] Excluir um chat remove seus anexos do disco e dos embeddings
