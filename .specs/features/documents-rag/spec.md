# Base de Conhecimento & RAG Global — Specification

## Problem Statement

O usuário quer importar documentos para uma base de conhecimento global e ter o chat respondendo com base neles (RAG). Hoje a aba Documentos é só um placeholder. Esta feature entrega a importação com feedback de progresso — só documentos totalmente processados entram no RAG — e a recuperação (retrieval) que o chat (M4) vai consumir como uma das camadas de contexto.

## Goals

- [x] Importar documentos (PDF, DOCX, TXT, MD) pela aba Documentos
- [x] Mostrar progresso de processamento por documento (fila → processando → pronto/erro)
- [x] Só documentos com status "pronto" entram na busca RAG
- [x] Buscar trechos relevantes por similaridade e expor isso para o chat injetar no contexto

## Out of Scope

| Feature | Reason |
| --- | --- |
| Anexos dentro de um chat específico | M4 — RAG "de chat" é isolado, esta feature é só a base global |
| OCR de documentos escaneados/imagens | Deferido (ver STATE.md Deferred Ideas) |
| Reindexar tudo ao trocar de modelo de embedding | Deferido — v1 assume o modelo de embedding é fixo por instalação |
| Edição de conteúdo do documento pelo app | Fora de escopo — app só lê/indexa o que foi importado |

---

## User Stories

### P1: Importar documento ⭐ MVP

**User Story**: Como usuário, quero clicar em "importar" na aba Documentos e escolher um arquivo do meu computador, para adicioná-lo à base de conhecimento.

**Why P1**: Ação central da feature — sem importar, não há o que indexar.

**Acceptance Criteria**:

1. WHEN o usuário clica em importar THEN o sistema SHALL abrir um seletor de arquivo nativo filtrado para PDF/DOCX/TXT/MD
2. WHEN um ou mais arquivos são escolhidos THEN o sistema SHALL copiá-los para `documents/` (na pasta-base) e registrá-los com status inicial "na fila"
3. WHEN o arquivo é maior que um limite configurável (ex.: 200MB) OU tem extensão não suportada THEN o sistema SHALL rejeitar com mensagem clara antes de copiar

**Independent Test**: Importar um PDF pequeno e ver ele aparecer na lista da aba Documentos imediatamente com status "na fila".

---

### P1: Ver progresso de processamento ⭐ MVP

**User Story**: Como usuário, quero ver o progresso de processamento de cada documento importado, para saber quando ele já pode ser usado.

**Why P1**: Requisito explícito — só documentos processados devem valer como RAG, então o usuário precisa saber o status.

**Acceptance Criteria**:

1. WHEN um documento está na fila THEN o sistema SHALL processá-lo em background pelas etapas: extrair texto → dividir em chunks → gerar embeddings → indexar
2. WHEN o documento está em qualquer etapa de processamento THEN a UI SHALL mostrar isso visualmente (ex.: "processando", com indicador)
3. WHEN o processamento termina com sucesso THEN o status SHALL virar "pronto" e o documento passa a entrar nas buscas RAG
4. WHEN o processamento falha (arquivo corrompido, texto vazio, erro de parsing) THEN o status SHALL virar "erro" com mensagem, e o documento NÃO SHALL entrar no RAG
5. WHEN vários documentos são importados de uma vez THEN o sistema SHALL processá-los (em paralelo ou fila, à escolha do design) sem travar a UI

**Independent Test**: Importar um documento grande, observar o status mudando de "na fila" → "processando" → "pronto"; importar um arquivo corrompido e ver status "erro".

---

### P1: Listar e remover documentos ⭐ MVP

**User Story**: Como usuário, quero ver todos os documentos importados com seu status, e poder remover os que não quero mais.

**Why P1**: Gestão básica da base — sem isso, a base só cresce e não há como corrigir erros.

**Acceptance Criteria**:

1. WHEN a aba Documentos é aberta THEN o sistema SHALL listar todos os documentos com nome, status e tamanho
2. WHEN o usuário remove um documento THEN o sistema SHALL apagar o arquivo de `documents/`, seus embeddings da tabela global, e o registro
3. WHEN um documento com status "erro" é removido e reimportado THEN o sistema SHALL tentar processar novamente do zero

**Independent Test**: Remover um documento "pronto" e confirmar que uma pergunta que antes recuperava esse trecho não recupera mais.

---

### P2: Retrieval usado pelo chat

**User Story**: Como usuário, quero que minhas perguntas no chat considerem os documentos prontos da base, para receber respostas fundamentadas no meu conteúdo.

**Why P2**: É o "porquê" de tudo isso existir, mas depende do M4 (chat) para ser demonstrável ponta a ponta — esta feature entrega a capacidade de recuperação; o consumo real acontece em M4.

**Acceptance Criteria**:

1. WHEN uma pergunta é feita no chat THEN o sistema SHALL embeddar a pergunta e buscar os top-k trechos mais similares entre os documentos com status "pronto"
2. WHEN nenhum documento está "pronto" THEN a busca RAG global SHALL retornar vazio sem erro (chat funciona normalmente sem contexto extra)
3. WHEN trechos são recuperados e usados THEN o sistema SHALL expor de qual documento cada trecho veio (para citação futura no M4)

**Independent Test**: Com um documento "pronto" contendo um fato específico, fazer uma pergunta relacionada e confirmar que os trechos recuperados vêm daquele documento.

---

## Edge Cases

- WHEN um documento é removido enquanto está "processando" THEN o sistema SHALL cancelar o processamento em andamento e limpar qualquer chunk parcial já indexado
- WHEN dois documentos com o mesmo nome de arquivo são importados THEN o sistema SHALL tratá-los como registros distintos (IDs únicos), sem sobrescrever
- WHEN o texto extraído de um documento está vazio (ex.: PDF só com imagens, sem OCR) THEN o status SHALL virar "erro" com mensagem específica ("nenhum texto encontrado")
- WHEN a pasta-base é trocada (ver feature de Configurações) THEN os documentos já indexados na pasta antiga NÃO SHALL aparecer na nova pasta (consistente com a decisão de não migrar documentos automaticamente)
- WHEN o app é fechado com documentos "na fila"/"processando" THEN ao reabrir THEN o sistema SHALL retomar ou reenfileirar o processamento pendente

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| --- | --- | --- | --- |
| DOC-01 | P1: Importar documento (seletor nativo) | Implemented | ✅ **Verificado clicando (2026-07-27)** — o seletor nativo do Windows foi aberto pelo botão e respondido; o arquivo escolhido apareceu na lista em 517 ms |
| DOC-02 | P1: Copiar para pasta + registrar "na fila" | Implemented | Implemented |
| DOC-03 | P1: Rejeitar arquivo inválido/grande demais | Implemented | Implemented |
| DOC-04 | P1: Pipeline extrair→chunk→embed→indexar | Implemented | Implemented |
| DOC-05 | P1: UI de progresso por documento | Implemented | ✅ **Verificado na tela (2026-07-27)** — a linha do documento passou por `Indexando` (+5,8 s) e chegou a `Pronto` em 16,6 s, num TXT de 134 KB. `Na fila`/`Lendo`/`Dividindo` **não foram capturados**: passam em menos que o intervalo de leitura de 120 ms |
| DOC-06 | P1: Status "erro" com mensagem | Implemented | Implemented |
| DOC-07 | P1: Processar múltiplos sem travar UI | Implemented | Implemented |
| DOC-08 | P1: Listar documentos com status | Implemented | Implemented |
| DOC-09 | P1: Remover documento (arquivo + embeddings) | Implemented | ✅ **Verificado clicando (2026-07-27)** pelo lado da lista — o botão Remover tirou o documento da tela. Que os embeddings saem junto continua provado por teste contra LanceDB real, não pela UI |
| DOC-10 | P2: Retrieval top-k por similaridade | Implemented | Implemented (revisto em 2026-07-26 — AD-036: ranqueamento entre namespaces, piso de relevância relativo e expansão para o chunk seguinte). ⚠️ **Revisto de novo em 2026-07-27 pela AD-050**: o pool passou a incluir a memória da conversa, o teto de 4 é compartilhado, e o documento só fica colado na pergunta enquanto for o acerto mais próximo |
| DOC-11 | P2: Retrieval vazio sem erro quando base vazia | Implemented | Implemented |
| DOC-12 | P2: Expor origem/citação dos trechos | Implemented | ✅ **Verificado no app (2026-07-27)** — resposta de um chat com anexo saiu como `3,72 unidades [fonte: relatorio-anexo.txt]`, com o nome do arquivo certo |

**ID format:** `DOC-[NUMBER]`
**Status values:** Pending → In Design → In Tasks → Implementing → Verified
**Coverage:** 12 total, 12 implementados. Verificado por teste real: embeddings via ONNX Runtime, isolamento de namespace e deletes no LanceDB. **A importação foi exercitada clicando em 2026-07-27** (DOC-01, DOC-05, DOC-09, DOC-12), o que fecha a pendência que esta linha registrava desde 2026-07-25.

⚠️ **DOC-10 mudou de sentido pela AD-050** — ver a linha na tabela. A medição que motivou a mudança está em `chat::context_assembler::retrieval_quality`: contra a base real do usuário, a pior pergunta que o corpus **responde** fica a 0,3077 e a melhor que ele **não** responde a 0,3150. As duas populações se separam por 0,0073, o que descarta um limiar absoluto e é o motivo de a correção ter sido comparativa em vez de um corte.

---

## Success Criteria

- [x] Importar um PDF/DOCX/TXT/MD real e vê-lo chegar a "pronto" sem intervenção manual
- [x] Um documento "erro" nunca aparece nos resultados de retrieval
- [x] Remover um documento reflete imediatamente na busca (não aparece mais em resultados novos)
- [x] Retrieval retorna trechos com referência de qual documento vieram
