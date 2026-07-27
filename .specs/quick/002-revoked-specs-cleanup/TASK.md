# Quick Task 002: Três specs ainda descreviam Ollama e LM Studio

**Date:** 2026-07-27
**Status:** Done
**Features afetadas:** `connections-models`, `single-active-connection`,
`embedded-runtime` (revogadas em parte por `self-contained-runtime`)

## Description

O M9 removeu Ollama, LM Studio, a conexão por URL manual, o trait
`ProviderClient` e as tabelas `connections`/`model_configs` (AD-039, AD-042).
As três specs que descreviam esse mundo **nunca receberam nota de revogação**:
`grep -i "revogad\|superseded"` nelas só encontra menções à AD-016, de outro
assunto.

Isso quebra a regra 4 de `.claude/rules/spec-driven-changes.md` — *"`.specs/`
descreve o que o projeto oferece hoje"*. Hoje um leitor pode abrir
`connections-models/spec.md`, ler *"testar Ollama (`http://localhost:11434`)"* e
acreditar que o app faz isso. A `single-active-connection` chega a listar, como
verificação **pendente**, *"ativar Ollama → ativar LM Studio"* — um teste manual
de algo que não existe mais.

**Não é remoção de requisito em silêncio:** a regra pede o contrário — marcar o
que saiu, por qual spec e em que AD, preservando o histórico do "por quê".

## Escopo desta task (fora do guardrail dos 3 arquivos, deliberadamente)

São 6 arquivos, todos de documentação, sem uma linha de código. O guardrail de
3 arquivos do quick mode existe para conter risco de implementação; aqui não há
nenhum. Segue como quick task porque **não há decisão de design a tomar**: o que
cada requisito virou já está decidido e implementado desde o M9 — falta só
registrar.

## Files Changed

- `.specs/features/connections-models/spec.md`
- `.specs/features/connections-models/tasks.md`
- `.specs/features/single-active-connection/spec.md`
- `.specs/features/single-active-connection/tasks.md`
- `.specs/features/embedded-runtime/spec.md`
- `.specs/features/embedded-runtime/tasks.md`

## O mapa, conferido contra o código (não deduzido da spec)

Cada linha abaixo foi verificada por `grep` no código atual, não presumida a
partir do que o M9 dizia que ia fazer.

| Requisito | Desfecho | Evidência no código |
| --- | --- | --- |
| CONN-01 detectar Ollama/LM Studio | **Revogado** (SELF-02) | `providers/ollama.rs` e `lmstudio.rs` não existem |
| CONN-02 habilitar/desabilitar conexão | **Revogado** (SELF-01) | tabela `connections` derrubada na migração 7 |
| CONN-03 URL customizada | **Revogado** (SELF-01) | nenhum formulário de URL em `components/Runtime/` |
| CONN-04 estado vazio de conexões | **Revogado** (SELF-01) | substituído pelo estágio `NoModel` |
| CONN-05 listar modelos instalados | **Vive, reformulado** | `list_installed_models` lê os `.gguf` da pasta, não uma API de provedor |
| CONN-06 selecionar modelo ativo | **Vive** (SELF-07) | `set_active_model` sobre `embedded_runtime` |
| CONN-07 detectar RAM | **Vive** | `system_info::total_ram_gb` |
| CONN-08 catálogo + estimativa de RAM | **Vive** | `models/catalog.rs`, `estimate_ram_gb` |
| CONN-09 ocultar o que não cabe | **Vive** | `ModelsList.tsx` filtra por `fits_ram` |
| CONN-10 download manual por nome/link | **Vive, estreitado** | `ModelsList.tsx` tem `manualUrl`; só URL direta de `.gguf` — o `pull` por nome do Ollama saiu junto com o Ollama |
| CONN-11 download com progresso | **Vive** | evento `model-download-progress` |
| CONN-12/13 contexto e GPU | **Vive** (SELF-08) | `configure_model` |
| ACTIVE-01..08 | **Revogados** (SELF-01, SELF-07) | não há conexão para ativar; o "par ativo" virou uma linha só |
| ACTIVE-09/10 migração versionada | **Vive como infraestrutura** | a lista `MIGRATIONS` em `db.rs` está na 8 |
| EMBED-01 opção visível em Conexões | **Revogado** (SELF-01) | não há tela de Conexões |
| EMBED-02 baixar o binário | **Revogado** (SELF-09/10) | vem do bundle; `runtime/release.rs` apagado |
| EMBED-03 baixar o modelo padrão no setup | **Revogado** (AD-043) | `prepare_runtime` não baixa modelo |
| EMBED-04 sidecar via `ProviderClient` | **Revogado** (SELF-03) | `LlamaServerClient` concreto |
| EMBED-05 modelo embutido selecionável | **Revogado por fusão** (SELF-07) | não há "modelo embutido" contra outros |
| EMBED-06 auto-start | **Vive** | marcador `SPEC:` em `lib.rs` |
| EMBED-07 morre no quit | **Vive** | reforçado por SIDE-04/05 |
| EMBED-08 sidecar quebrado reporta indisponível | **Vive** | `runtime_status` |
| EMBED-09 desabilitar a conexão para o sidecar | **Revogado** (SELF-01) | virou `stop_runtime`, sem conexão no meio |
| EMBED-10/11 GPU/CPU | **Vive** (SELF-11) | `probe_devices` |
| EMBED-12 config por restart | **Vive** | `configure_model` reinicia o sidecar |
| EMBED-13 GGUF por link do Hugging Face | **Vive** | mesmo `manualUrl` do CONN-10 |
| EMBED-14 exibir a tag do llama.cpp | **Vive, reformulado** | `RuntimeStatus.release_tag`; a tag agora é fixa em `vendor.json`, não consultada na API do GitHub |

## Verification

- [x] **Nenhum item de UAT aberto** (`- [ ]`) nas specs de feature testa Ollama,
      LM Studio ou conexão por URL. As menções que restam em
      `self-contained-runtime/tasks.md` são critérios *internos* de tasks já
      concluídas, e vários deles são justamente `grep`s que **exigem** a palavra
      para provar a ausência dela no código
- [x] Cada requisito revogado nomeia a spec que o revogou (SELF-xx) e a AD
- [x] Nenhum requisito apagado — os 37 IDs das três specs continuam listados,
      com desfecho ao lado
- [x] Dois itens fora do escopo original, achados durante a verificação e
      corrigidos: `chat-messaging/spec.md` mandava *"desligar o Ollama no meio de
      uma resposta"* como teste independente, e dava como critério de sucesso
      *"conversa real com um modelo do Ollama"*
- [x] Gates rodados mesmo sem código tocado: `cargo test` **174 passando / 0
      falhas / 12 ignorados**, `npm run build` limpo, `npm run test:scripts`
      **44 passando**

## Contagem do saneamento

| Spec | Requisitos | Revogados | Reformulados | Vivos |
| --- | --- | --- | --- | --- |
| `connections-models` | 13 | 5 | 2 | 6 |
| `single-active-connection` | 10 | 8 | 1 | 1 |
| `embedded-runtime` | 14 | 4 | 4 | 6 |

**O que a contagem revela:** a `embedded-runtime` não era uma spec morta — 6 dos
14 requisitos são o coração do app hoje. Tratá-la como revogada por inteiro
teria apagado a origem do ciclo de vida do sidecar. Já a
`single-active-connection` perdeu 8 de 10, e o que sobrou (ACTIVE-09,
versionamento de migração) foi um efeito colateral da feature, não o objetivo
dela.

## Commit

pendente — `docs(specs): mark what the single-runtime milestone revoked`
