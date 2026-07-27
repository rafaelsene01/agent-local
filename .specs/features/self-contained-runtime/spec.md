# Runtime autossuficiente — Especificação

**Contexto:** `.specs/features/self-contained-runtime/context.md`
**Status:** Draft
**Milestone:** M9

---

## Problem Statement

O LocalMind foi desenhado para falar com três runtimes (Ollama, LM Studio e o embutido) mais uma URL manual, e paga por isso todos os dias: quatro implementações de `ProviderClient`, uma tabela `connections` cuja única função é responder "qual deles atende?", uma tela inteira de Conexões, e um modelo mental que o usuário precisa aprender antes de conversar. Na prática, o runtime embutido já resolve o caso inteiro — foi ele que o app usou em toda verificação real (AD-024, AD-028, AD-033).

Ao mesmo tempo, o app se anuncia como offline-first mas não roda sem internet na primeira vez: baixa o `llama-server`, o ONNX Runtime e o pdfium sob demanda. Numa máquina sem rede — ou atrás de um proxy corporativo que bloqueia o GitHub — a instalação simplesmente não conclui.

Esta feature elimina os dois problemas de uma vez: um runtime só, embutido no instalador junto com tudo que ele precisa.

## Goals

- [ ] O app não oferece, detecta nem fala com nenhum programa externo — o único runtime é o embutido
- [ ] Nenhum componente binário é baixado em tempo de execução: `llama-server` (Vulkan e CPU), ONNX Runtime e pdfium viajam dentro do instalador
- [ ] Instalar e conversar exige da internet **apenas** o download de um modelo GGUF escolhido pelo usuário
- [ ] Importar e consultar um PDF funciona com a máquina completamente offline
- [ ] O código perde a abstração de multi-provider: um cliente concreto, uma fonte de verdade para "qual modelo responde"

## Out of Scope

Explicitamente excluído. Documentado para impedir crescimento de escopo.

| Item | Motivo |
| --- | --- |
| Embutir um modelo GGUF no instalador | Decisão D3 — o usuário escolhe o modelo que cabe na máquina dele; o instalador não carrega 1–5 GB |
| Voltar Ollama/LM Studio atrás de uma flag ou modo avançado | Decisão D1 — sai inteiro, sem resíduo. Se voltar, volta como feature nova |
| Suporte a CUDA/ROCm | AD-022 segue valendo: Vulkan cobre NVIDIA/AMD/Intel com um binário só |
| macOS | Segue fora do escopo do projeto (PROJECT.md) |
| Atualizar o llama.cpp sem uma release do LocalMind | Consequência aceita de D2 — a versão passa a ser fixada em build |
| Migrar chats/documentos de um provedor externo para o embutido | Não há o que migrar: conversas e documentos nunca foram atrelados a um provedor |

---

## User Stories

### P1: Um runtime só, sem escolha a fazer ⭐ MVP

**User Story:** Como usuário, quero abrir o app e ter um único runtime já disponível, sem precisar entender o que são Ollama, LM Studio ou "conexão", para poder ir direto ao ponto: escolher um modelo e conversar.

**Why P1:** É o pedido literal do usuário e o que remove a maior parte da complexidade acidental. Tudo o mais depende desta decisão estar aplicada no backend.

**Acceptance Criteria:**

1. WHEN o usuário abre a área de runtime THEN o sistema SHALL mostrar apenas o runtime embutido, sem lista de conexões e sem formulário de adicionar URL
2. WHEN o app inicia THEN o sistema SHALL nunca fazer requisição a `localhost:11434` nem a `localhost:1234`
3. WHEN uma mensagem é enviada THEN o sistema SHALL usar o runtime embutido e o modelo ativo, sem consultar nenhuma tabela de conexões
4. WHEN não há modelo ativo THEN o sistema SHALL recusar o envio com uma mensagem que nomeia a ação que falta ("escolha um modelo em Runtime"), não com um erro genérico
5. WHEN o código é compilado THEN o sistema SHALL não conter `providers/ollama.rs` nem `providers/lmstudio.rs`, e nenhuma string "ollama"/"lmstudio" na UI ou nos textos de i18n

**Independent Test:** rodar o app, abrir a área de runtime, ver um único card; enviar mensagem e receber resposta; `grep -ri "ollama\|lmstudio" src src-tauri/src` volta vazio.

---

### P1: Banco com uma fonte de verdade ⭐ MVP

**User Story:** Como mantenedor, quero que "qual modelo responde, com qual contexto e com qual GPU" more num lugar só, para que configurar não volte a gravar num lugar e ler de outro.

**Why P1:** As tabelas `connections` e `model_configs` ficam sem sentido com um runtime só, e a duplicação entre `model_configs` e `embedded_runtime` já causou um bug real (EMBED-12: configuração persistida e ignorada, AD-024).

**Acceptance Criteria:**

1. WHEN um banco existente é aberto THEN o sistema SHALL aplicar uma migração que remove `connections` e `model_configs` e preserva `chats`, `messages`, `documents`, `chat_attachments` e `embedded_runtime`
2. WHEN a migração roda num banco que tinha Ollama ativo THEN o sistema SHALL abrir normalmente, com o runtime embutido como única opção
3. WHEN a migração roda duas vezes THEN o sistema SHALL ser idempotente (`PRAGMA user_version` já no alvo, nada reexecuta)
4. WHEN o modelo ativo é trocado ou configurado THEN o sistema SHALL gravar em `embedded_runtime` e em nenhum outro lugar
5. WHEN o app reinicia THEN o sistema SHALL retomar o mesmo modelo, contexto e escolha de GPU da sessão anterior

**Independent Test:** copiar um `localmind.db` com conexões Ollama, abrir o app, confirmar que os chats continuam lá e que o runtime embutido responde.

---

### P1: Componentes dentro do instalador ⭐ MVP

**User Story:** Como usuário numa máquina sem internet (ou atrás de um proxy que bloqueia o GitHub), quero que o app instale e funcione com o que veio no instalador, para não depender de um download que não vai acontecer.

**Why P1:** É a segunda metade do pedido ("auto suficiente"). Sem isso o app continua exigindo rede para existir.

**Acceptance Criteria:**

1. WHEN o instalador é gerado THEN o sistema SHALL conter os binários `llama-server` Vulkan **e** CPU, o ONNX Runtime e o pdfium para aquele SO
2. WHEN o runtime é preparado pela primeira vez THEN o sistema SHALL resolver o `llama-server` dentro dos recursos do app, sem nenhuma requisição de rede
3. WHEN o binário Vulkan não executa na máquina THEN o sistema SHALL cair para o binário CPU embutido, também sem download
4. WHEN um PDF é importado com a máquina offline THEN o sistema SHALL parsear, chunkar, embeddar e deixar o documento `ready`
5. WHEN o app roda no Linux (`.deb` ou `.AppImage`) THEN o sistema SHALL conseguir executar o `llama-server` mesmo que o empacotador não preserve o bit de execução
6. WHEN o setup é executado THEN o sistema SHALL não exibir mais a etapa "baixando o runtime" — só a de modelo, quando houver

**Independent Test:** desligar a rede, instalar do zero, copiar um `.gguf` na pasta de modelos à mão, abrir o app e conversar; importar um PDF e ver "pronto".

---

### P2: Versões fixadas e reprodutíveis

**User Story:** Como mantenedor, quero que as versões do llama.cpp, do ONNX Runtime e do pdfium fiquem declaradas num arquivo, para que build local e CI produzam o mesmo app e uma atualização seja um commit revisável.

**Why P2:** O app funciona sem isso (dá para chumbar as URLs no script), mas sem um manifesto a versão embutida vira folclore e o "por que na minha máquina é diferente" fica sem resposta.

**Acceptance Criteria:**

1. WHEN o projeto é buildado THEN o sistema SHALL ler as versões de um único manifesto versionado no git
2. WHEN um artefato declarado não existe no servidor THEN o build SHALL falhar com o nome do arquivo procurado, nunca seguir sem ele
3. WHEN o `tauri build` ou o `tauri dev` é disparado THEN o sistema SHALL garantir os artefatos antes de compilar, sem passo manual
4. WHEN os artefatos já estão presentes e íntegros THEN o passo SHALL ser um no-op rápido, sem rebaixar nada

**Independent Test:** apagar a pasta de recursos, rodar `npm run tauri build`, ver os três componentes serem trazidos e o bundle sair completo.

---

### P2: Distribuição consistente nos dois modos

**User Story:** Como usuário do bundle portátil, quero que a atualização traga também os componentes novos, para não ficar com um executável novo apontando para um `llama-server` velho.

**Why P2:** O M8 entregou dois caminhos de atualização; ambos passam a carregar recursos, e o portátil é montado à mão (`make-portable.mjs` hoje copia só o `.exe`).

**Acceptance Criteria:**

1. WHEN o zip portátil é montado THEN o sistema SHALL incluir a pasta de recursos ao lado do executável
2. WHEN uma atualização portátil é aplicada THEN o sistema SHALL substituir também os arquivos de recursos, mantendo o rollback existente em caso de falha
3. WHEN o app portátil roda depois da atualização THEN o sistema SHALL resolver os componentes na pasta nova

**Independent Test:** gerar o zip, extrair, conferir que a pasta de recursos veio junto e que o app sobe a partir dela.

---

### P3: Faxina dos downloads antigos

**User Story:** Como usuário que já usava o LocalMind, quero que os ~150 MB baixados pela versão anterior sumam sozinhos, para não guardar arquivos que nada mais lê.

**Why P3:** É higiene, não funcionalidade — o app funciona igual com o lixo lá.

**Acceptance Criteria:**

1. WHEN o app inicia depois da atualização THEN o sistema SHALL remover `<pasta-base>/runtime/{vulkan,cpu,onnxruntime,pdfium}` se existirem
2. WHEN a remoção falha (arquivo em uso, permissão) THEN o sistema SHALL ignorar silenciosamente e abrir normalmente
3. WHEN a pasta de modelos existe THEN o sistema SHALL nunca tocá-la — os `.gguf` baixados continuam valendo

**Independent Test:** criar as pastas à mão com um arquivo dentro, abrir o app, conferir que sumiram e que os modelos continuam listados.

---

## Edge Cases

- WHEN a pasta de recursos do app não existe ou está incompleta (instalação corrompida) THEN o sistema SHALL exibir um erro que nomeia o arquivo faltando e sugere reinstalar, em vez de tentar baixar
- WHEN o `llama-server` embutido não executa em nenhum dos dois backends THEN o sistema SHALL reportar o motivo do último erro e deixar o app aberto, sem travar o boot
- WHEN a migração encontra um banco já sem `connections` (instalação nova) THEN o sistema SHALL seguir sem erro
- WHEN o usuário tem um `.gguf` na pasta de modelos mas nenhum ativo THEN o sistema SHALL listar os arquivos e pedir uma escolha, sem escolher sozinho
- WHEN o disco não tem espaço para o modelo THEN o sistema SHALL recusar antes de começar o download (comportamento atual de `ensure_free_space`, preservado)
- WHEN o app roda num SO fora de Windows/Linux THEN o sistema SHALL reportar plataforma não suportada, como hoje

---

## Requirement Traceability

| ID | História | Tasks | Status |
| --- | --- | --- | --- |
| SELF-01 | P1: Um runtime só | T4, T7, T8, T9, T10, T11 | In Tasks |
| SELF-02 | P1: Um runtime só | T4, T5 | In Tasks |
| SELF-03 | P1: Um runtime só | T1, T5 | In Tasks |
| SELF-04 | P1: Um runtime só | T3 | In Tasks |
| SELF-05 | P1: Um runtime só | T3 | In Tasks |
| SELF-06 | P1: Fonte de verdade | T6 | In Tasks |
| SELF-07 | P1: Fonte de verdade | T2 | In Tasks |
| SELF-08 | P1: Fonte de verdade | T2 | In Tasks |
| SELF-09 | P1: Componentes no instalador | T13, T22 | In Tasks |
| SELF-10 | P1: Componentes no instalador | T14, T17 | In Tasks |
| SELF-11 | P1: Componentes no instalador | T17, T22 | In Tasks |
| SELF-12 | P1: Componentes no instalador | T15, T16, T22 | In Tasks |
| SELF-13 | P1: Componentes no instalador | T14, T20 | In Tasks |
| SELF-14 | P2: Versões fixadas | T12, T13 | In Tasks |
| SELF-15 | P2: Versões fixadas | T12, T17 | In Tasks |
| SELF-16 | P2: Distribuição | T19 | In Tasks |
| SELF-17 | P2: Distribuição | T19 | In Tasks |
| SELF-18 | P3: Faxina | T18 | In Tasks |
| SELF-19 | Transversal | T7, T21 | In Tasks |

**Descrição de cada ID:**

| ID | Requisito |
| --- | --- |
| SELF-01 | A UI expõe um único runtime; não há lista de conexões nem formulário de URL manual |
| SELF-02 | Nenhuma requisição a `:11434` ou `:1234`; `providers/ollama.rs` e `providers/lmstudio.rs` deixam de existir |
| SELF-03 | Um cliente concreto substitui o trait `ProviderClient` com `Box<dyn>` e o `match` de provedor |
| SELF-04 | O chat resolve modelo e configuração pelo runtime, não por conexão ativa |
| SELF-05 | Sem modelo ativo, o erro nomeia a ação que falta |
| SELF-06 | Migração remove `connections` e `model_configs` preservando todo o resto |
| SELF-07 | `embedded_runtime` é a única fonte de verdade de modelo/contexto/GPU |
| SELF-08 | Modelo, contexto e GPU sobrevivem ao restart do app |
| SELF-09 | Instalador contém `llama-server` Vulkan + CPU, ONNX Runtime e pdfium |
| SELF-10 | O runtime resolve o binário nos recursos do app, sem rede |
| SELF-11 | Fallback Vulkan → CPU usa o binário embutido, sem download |
| SELF-12 | Parsing de PDF e embeddings funcionam offline |
| SELF-13 | O bit de execução é garantido no Linux mesmo se o empacotador não o preservar |
| SELF-14 | Versões dos três componentes num manifesto único, versionado |
| SELF-15 | Artefato ausente falha o build nomeando o arquivo |
| SELF-16 | O zip portátil inclui a pasta de recursos |
| SELF-17 | A atualização portátil substitui os recursos, com o rollback existente |
| SELF-18 | Restos de download de versões anteriores são apagados no boot |
| SELF-19 | PROJECT.md, ROADMAP.md, README e os docs de `.specs/codebase/` deixam de descrever multi-provider |

**Status values:** Pending → In Design → In Tasks → Implementing → Verified
**Coverage:** 19 no total, **19 mapeados** para tasks, 0 sem cobertura ✅

---

## Success Criteria

- [ ] Numa máquina limpa **sem internet**, com um `.gguf` copiado à mão para a pasta de modelos: instalar → abrir → escolher o modelo → conversar, tudo funciona
- [ ] Numa máquina limpa **com internet**: instalar → abrir → baixar um modelo do catálogo → conversar, sem nenhuma outra etapa de download
- [ ] Importar um PDF offline chega a `ready` e as respostas citam o documento
- [ ] `grep -ri "ollama\|lmstudio" src src-tauri/src` não retorna nada
- [ ] `cargo test` verde com contagem **maior ou igual** à atual (123), sem testes deletados em silêncio
- [ ] `npm run build` limpo e i18n com paridade EN/PT
- [ ] O tamanho do instalador é **medido** e registrado (não estimado) para os dois SOs
