# Conexões & Modelos — Specification

> ## ⚠️ PARCIALMENTE REVOGADA por `self-contained-runtime` (M9) — 2026-07-27
>
> **Decisões: AD-039 (planejamento) e AD-042 (execução).**
>
> O app não fala mais com Ollama nem com LM Studio, não aceita conexão por URL
> manual, e as tabelas `connections` e `model_configs` foram derrubadas pela
> migração 7. **Tudo neste documento que descreve "uma conexão" descreve um app
> que não existe mais.**
>
> O que sobreviveu não foi a gestão de conexões — foi a **gestão de modelos**:
> detecção de RAM, catálogo curado com estimativa, download com progresso e
> configuração de contexto/GPU continuam valendo, agora sobre o runtime único.
> Ver a tabela de rastreabilidade no fim para o desfecho de cada requisito.
>
> **A spec vigente é `.specs/features/self-contained-runtime/spec.md`.**

## Problem Statement

O app precisa saber com quais runtimes locais (Ollama, LM Studio) conversar, quais modelos usar dentro deles, e — quando o usuário quiser — baixar novos modelos sem sugerir algo que vai travar a máquina por falta de memória. Hoje a sidebar só tem um placeholder "Nenhuma conexão configurada ainda". Esta feature entrega a gestão real de conexões e modelos, pré-requisito para o chat conversar de verdade (M4).

## Goals

> Os checkboxes abaixo estavam **todos desmarcados** apesar de a feature ter sido
> entregue em 2026-07-25 — leitura enganosa que ficou do template original.
> Marcados agora com o desfecho real; ❌ é o que o M9 revogou.

- [x] ❌ Detectar Ollama e LM Studio automaticamente e mostrar status de saúde — entregue, depois revogado (AD-042)
- [x] ❌ Usuário marca quais conexões estão habilitadas para uso — entregue, depois revogado (AD-042)
- [x] Listar modelos já instalados/carregados ~~por conexão habilitada~~, para escolher qual usar — hoje lê os `.gguf` da pasta de modelos
- [x] Listar modelos candidatos a download, ocultando/avisando sobre os que não cabem na RAM disponível
- [x] Baixar modelo com barra de progresso real
- [x] Configurar tamanho de contexto e CPU/GPU por modelo

## Out of Scope

| Feature | Reason |
| --- | --- |
| Enviar mensagem/streaming de fato | M4 — esta feature só prepara conexões e modelos |
| Runtime embutido llama.cpp (fallback) | M7 — cenário "nenhuma conexão disponível" aqui só mostra estado vazio |
| Catálogo completo navegável de todos os modelos do Ollama/LM Studio | Nenhum dos dois expõe API pública de catálogo (confirmado via pesquisa) — v1 usa lista curada + pull manual por nome/link |
| Detecção de VRAM | Decisão do usuário: só RAM do sistema entra no filtro (ver DECISION abaixo) |

## Research Findings (Knowledge Verification Chain)

Confirmado via busca (não fabricado):

- **Ollama** `/api/generate` e `/api/chat` aceitam `options.num_ctx` (tamanho de contexto) e `options.num_gpu` (camadas offloaded pra GPU) **por requisição** — não precisa reconfigurar o servidor. `/api/pull` retorna NDJSON com `{status, digest, total, completed}` por download, dá pra barra de progresso real. [docs.ollama.com/api]
- **Ollama NÃO tem API pública de catálogo** — não dá pra listar programaticamente "todos os modelos disponíveis para baixar" com tamanho. `ollama.com/library` é só a interface web.
- **LM Studio** tem REST API nativa v1 (`/api/v1/*`, LM Studio ≥0.4.0) com endpoint de **download** (aceita identificador do catálogo LM Studio ou link direto do Hugging Face) e endpoint de **load** que aceita `contextLength` e `gpuOffload` (0-1, `"off"`, `"max"`) no carregamento do modelo. [lmstudio.ai/docs/developer/rest]
- **Detecção de VRAM não é confiável** entre fabricantes (NVIDIA/AMD/Intel) sem SDKs proprietários — RAM do sistema via crate `sysinfo` é a única detecção universal e confiável em Rust.

**DECISÃO DO USUÁRIO:** filtro de memória usa só RAM do sistema (detecção automática via `sysinfo`), estimando necessidade de RAM por `params(B) × bytes-por-peso-do-quant × 1.2` (overhead). Modelos que não cabem são ocultados/avisados, mas o usuário pode sempre baixar por nome/link manual mesmo sem garantia de caber.

---

## User Stories

### P1: Ver conexões disponíveis e habilitar quais usar ⭐ MVP

**User Story**: Como usuário, quero ver quais runtimes locais o app encontrou (Ollama, LM Studio) e marcar quais devo usar, para controlar de onde vêm as respostas.

**Why P1**: Sem isso não há para onde mandar mensagens no M4.

**Acceptance Criteria**:

1. WHEN o app verifica conexões THEN o sistema SHALL testar Ollama (`http://localhost:11434`) e LM Studio (`http://localhost:1234`) e mostrar status: disponível / indisponível
2. WHEN uma conexão está disponível THEN o usuário SHALL poder marcá-la como habilitada ou desabilitada
3. WHEN uma conexão está desabilitada THEN seus modelos NÃO SHALL aparecer como opção de uso no chat (M4)
4. WHEN o usuário adiciona uma URL customizada (outro servidor OpenAI-compatible) THEN o sistema SHALL testá-la e tratá-la como uma conexão adicional
5. WHEN nenhuma conexão está disponível THEN o sistema SHALL mostrar um estado vazio claro com botão de "tentar novamente"

**Independent Test**: Com Ollama rodando, abrir a tela de Conexões e ver status "disponível"; desligar o Ollama e clicar em atualizar — status vira "indisponível".

---

### P1: Listar e escolher modelo instalado ⭐ MVP

**User Story**: Como usuário, quero ver os modelos já instalados em cada conexão habilitada e escolher qual usar, para conversar com o modelo certo.

**Why P1**: É o que o chat (M4) vai consumir para saber qual modelo chamar.

**Acceptance Criteria**:

1. WHEN uma conexão está habilitada THEN o sistema SHALL listar os modelos instalados nela (via API de listagem do provedor)
2. WHEN o usuário seleciona um modelo THEN essa escolha SHALL ficar disponível para uso no chat
3. WHEN uma conexão não tem nenhum modelo instalado THEN o sistema SHALL indicar isso e sugerir ir para a aba de download

**Independent Test**: Com um modelo instalado no Ollama, ver ele listado; selecioná-lo e confirmar que fica marcado como ativo.

---

### P2: Ver modelos para baixar, filtrados por memória

**User Story**: Como usuário, quero ver uma lista de modelos que posso baixar, sem me mostrar os que certamente não cabem na minha RAM, para não perder tempo baixando algo inutilizável.

**Why P2**: Importante para a experiência de descoberta, mas o chat já funciona sem isso (usando modelos já instalados).

**Acceptance Criteria**:

1. WHEN o app inicia THEN o sistema SHALL detectar a RAM total do sistema automaticamente (sem input do usuário)
2. WHEN a lista de modelos candidatos é exibida THEN cada item SHALL mostrar o tamanho estimado (RAM necessária) e se cabe ou não na RAM detectada
3. WHEN um modelo não cabe na RAM detectada THEN o sistema SHALL ocultá-lo por padrão ou marcá-lo claramente como "não recomendado" (com opção de mostrar mesmo assim)
4. WHEN o usuário quer um modelo fora da lista curada THEN o sistema SHALL permitir baixar por nome (Ollama) ou link Hugging Face (LM Studio) manualmente, sem garantia de checagem de memória

**Independent Test**: Em uma máquina com RAM detectada = X GB, confirmar que modelos estimados acima de X GB aparecem ocultos/marcados, e os que cabem aparecem normalmente.

---

### P2: Baixar modelo com progresso

**User Story**: Como usuário, quero ver uma barra de progresso ao baixar um modelo, para saber quanto falta.

**Why P2**: UX necessária mas o fluxo de "usar modelo já instalado" (P1) não depende disso.

**Acceptance Criteria**:

1. WHEN o usuário inicia um download THEN o sistema SHALL mostrar progresso (bytes baixados / total) atualizado em tempo real
2. WHEN o download termina THEN o modelo SHALL aparecer automaticamente na lista de "instalados" (P1)
3. WHEN o download falha ou é cancelado THEN o sistema SHALL mostrar erro e permitir tentar novamente
4. WHEN o usuário fecha a tela de Conexões durante um download THEN o download SHALL continuar em background

**Independent Test**: Baixar um modelo pequeno, observar a barra progredir até 100%, e ver o modelo aparecer na lista de instalados.

---

### P2: Configurar contexto e CPU/GPU por modelo

**User Story**: Como usuário, quero configurar o tamanho de contexto e se o modelo deve rodar em CPU ou GPU, para controlar performance e uso de memória.

**Why P2**: Afeta qualidade/performance das respostas, mas o chat funciona com os defaults do provedor sem isso.

**Acceptance Criteria**:

1. WHEN o usuário configura o tamanho de contexto de um modelo THEN essa configuração SHALL ser aplicada nas próximas chamadas àquele modelo (via `num_ctx` no Ollama, `contextLength` no load do LM Studio)
2. WHEN o usuário escolhe CPU ou GPU THEN essa preferência SHALL ser aplicada (via `num_gpu` no Ollama, `gpuOffload` no LM Studio)
3. WHEN o provedor não suporta um dos dois campos por API THEN o sistema SHALL indicar isso claramente em vez de fingir que aplicou

**Independent Test**: Reduzir o tamanho de contexto de um modelo e confirmar (via log/inspeção da requisição) que o valor configurado é enviado nas chamadas.

---

## Edge Cases

- WHEN Ollama e LM Studio estão ambos disponíveis e habilitados com o "mesmo" modelo por nomes diferentes THEN o sistema SHALL tratá-los como entradas separadas (namespaced por conexão), sem tentar deduplicar
- WHEN a RAM do sistema muda entre sessões (ex.: notebook trocado) THEN o sistema SHALL redetectar a cada abertura, não cachear indefinidamente
- WHEN um download é interrompido por queda de conexão/energia THEN o sistema SHALL permitir retomar (Ollama já resuma pulls parciais nativamente)
- WHEN o usuário desabilita a única conexão habilitada enquanto ela está selecionada no chat ativo THEN o sistema SHALL avisar que aquele chat ficará sem modelo disponível
- WHEN nenhum modelo curado cabe na RAM detectada THEN o sistema SHALL ainda mostrar a opção de pull manual, com aviso

---

## Requirement Traceability

> **Atualizada em 2026-07-27.** Cada desfecho abaixo foi conferido contra o
> código atual por `grep`, não deduzido do que o M9 dizia que ia fazer.

| Requirement ID | Story | Desfecho |
| --- | --- | --- |
| CONN-01 | P1: Detectar conexões + status | ❌ **Revogado** por SELF-02 (AD-042) — `providers/ollama.rs` e `providers/lmstudio.rs` não existem mais |
| CONN-02 | P1: Habilitar/desabilitar conexão | ❌ **Revogado** por SELF-01 (AD-042) — a tabela `connections` caiu na migração 7 |
| CONN-03 | P1: Conexão manual (URL customizada) | ❌ **Revogado** por SELF-01 (AD-039) — escolha explícita do usuário: nenhum provedor externo sobra |
| CONN-04 | P1: Estado vazio (nenhuma conexão) | ❌ **Revogado** por SELF-01 — substituído pelo estágio `NoModel`, que é o normal de uma instalação nova (AD-043) |
| CONN-05 | P1: Listar modelos instalados por conexão | ⚠️ **Vive, reformulado** — `list_installed_models` lê os `.gguf` da pasta de modelos; não há mais "por conexão" |
| CONN-06 | P1: Selecionar modelo ativo | ✅ **Vive** como SELF-07 — `set_active_model` sobre a linha `embedded_runtime` |
| CONN-07 | P2: Detectar RAM do sistema | ✅ **Vive** — `system_info::total_ram_gb`, com teste |
| CONN-08 | P2: Lista curada de modelos + estimativa de RAM | ✅ **Vive** — `models/catalog.rs`; o campo `provider` saiu do catálogo (AD-043) e 8 entradas do Ollama foram removidas, sobrando os 6 GGUF |
| CONN-09 | P2: Ocultar/avisar modelos que não cabem | ✅ **Vive** — `ModelsList.tsx` filtra por `fits_ram`, com "mostrar todos" |
| CONN-10 | P2: Pull manual por nome/link | ⚠️ **Vive, estreitado** — o campo de URL continua (`manualUrl` em `ModelsList.tsx`), mas só aceita link direto de `.gguf`; o `pull` por nome saiu junto com o Ollama |
| CONN-11 | P2: Download com progresso | ✅ **Vive** — evento `model-download-progress`, com canal próprio separado do preparo do motor (AD-043) |
| CONN-12 | P2: Configurar contexto por modelo | ✅ **Vive** como SELF-08 — `configure_model` |
| CONN-13 | P2: Configurar CPU/GPU por modelo | ✅ **Vive** como SELF-08 — `configure_model` |

**ID format:** `CONN-[NUMBER]`
**Coverage:** 13 no total — **5 revogados, 2 reformulados, 6 vivos.** Nenhum
apagado: o histórico do "por quê" fica.

**Onde os vivos são mantidos hoje:** os requisitos que sobreviveram passaram a
ser rastreados pela spec de `self-contained-runtime` (SELF-01, SELF-07, SELF-08).
Ao mexer no catálogo, na detecção de RAM ou no download de modelo, é **lá** que a
rastreabilidade se atualiza — não aqui.

---

## Success Criteria

> **Revogados junto com as conexões.** Os dois primeiros critérios abaixo
> dependiam de um Ollama rodando; os dois últimos migraram para os critérios de
> `self-contained-runtime`. Ficam registrados como o que a feature *pretendia*
> provar em 2026-07-25.

- [x] ~~Com Ollama rodando, o app detecta e lista seus modelos instalados sem configuração manual~~ — revogado (AD-042)
- [x] Modelos curados que não cabem na RAM detectada não aparecem soltos na lista principal — continua valendo, agora sob SELF-01
- [ ] Um download real progride visivelmente até 100% e o modelo vira selecionável — **nunca verificado clicando**; migrou para a T22 de `self-contained-runtime`
- [ ] Configurar contexto/CPU-GPU de um modelo reflete nas chamadas subsequentes — migrou para SELF-08
