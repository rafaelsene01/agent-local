# Conexão e Modelo Ativos Únicos — Specification

## Problem Statement

O M3 entregou conexões como um conjunto de checkboxes: várias podem estar "habilitadas" ao mesmo tempo (`connections.enabled`), enquanto o modelo ativo já é único (`model_configs.is_active`, com `set_active_model` zerando os outros). Essa assimetria deixa uma pergunta sem resposta na hora de mandar uma mensagem: **se três conexões estão habilitadas, qual delas o chat usa?** Antes de implementar o chat (M4) ou adicionar um quarto runtime (M7), o modelo mental precisa ser um só: **uma conexão ativa, um modelo ativo, e é esse par que responde.**

## Goals

- [x] Exatamente uma conexão pode estar ativa por vez (ou nenhuma, se o usuário ainda não escolheu)
- [x] Exatamente um modelo pode estar ativo por vez, e ele sempre pertence à conexão ativa
- [x] O par (conexão ativa, modelo ativo) é a única fonte da verdade para o chat
- [x] Conexões inativas continuam visíveis com status e com seus modelos inspecionáveis, sem precisar ativá-las antes
- [x] O schema do banco ganha versionamento de migração, para que essa mudança de coluna seja aplicável em bancos já existentes (resolve C-01)

## Out of Scope

| Feature | Reason |
| --- | --- |
| Modelo diferente por chat | Decisão do usuário nesta sessão: **revoga a AD-016** — não existe mais override por chat, o ativo global vale para todos |
| Usar o chat de fato com o par ativo | `chat-messaging` (M4) consome isso; aqui só se estabelece a regra e o estado |
| Fallback automático para outra conexão quando a ativa cai | Se a ativa está indisponível, o app avisa — não escolhe outra sozinho (seria justamente o comportamento ambíguo que esta feature remove) |

---

## Context / Decisões do usuário (2026-07-25)

- **"Conexão e modelo deve ter somente um único ativo, que é ele que deve ser usado na hora do chat"** — pedido literal.
- **AD-016 morre**: perguntado explicitamente se o override por chat sobrevivia, o usuário escolheu *"Mata a AD-016 — só global, um único ativo pra tudo"*.
- **Conexões inativas continuam listadas**: escolhido *"Continuam listadas com status, só não são a ativa"* — dá pra inspecionar os modelos de qualquer conexão antes de ativar.

---

## User Stories

### P1: Escolher a conexão ativa ⭐ MVP

**User Story**: Como usuário, quero marcar **uma** conexão como a ativa, para saber exatamente de onde virão as respostas.

**Why P1**: É a mudança central — sem ela o chat não tem destino definido.

**Acceptance Criteria**:

1. WHEN o usuário ativa uma conexão THEN o sistema SHALL desativar automaticamente qualquer outra conexão que estivesse ativa
2. WHEN nenhuma conexão foi ativada ainda THEN o sistema SHALL permitir esse estado (zero ativas) e indicá-lo claramente na UI
3. WHEN a lista de conexões é exibida THEN todas SHALL aparecer com seu status (disponível/indisponível), com a ativa visualmente distinta das demais
4. WHEN o usuário tenta ativar uma conexão com status "indisponível" THEN o sistema SHALL permitir a ativação mesmo assim, mas SHALL sinalizar que ela não está respondendo
5. WHEN a conexão ativa é excluída ou fica órfã THEN o sistema SHALL voltar ao estado "nenhuma ativa" em vez de manter um ponteiro quebrado

**Independent Test**: Ativar Ollama, confirmar que fica marcada; ativar LM Studio, confirmar que Ollama deixou de estar marcada — sem passo intermediário de desmarcar.

---

### P1: Escolher o modelo ativo, vinculado à conexão ativa ⭐ MVP

**User Story**: Como usuário, quero escolher um modelo e que essa escolha já implique qual conexão está ativa, para não precisar sincronizar duas escolhas na mão.

**Why P1**: Evita o estado inconsistente "conexão A ativa, modelo ativo pertence à conexão B".

**Acceptance Criteria**:

1. WHEN o usuário escolhe um modelo de qualquer conexão THEN o sistema SHALL marcar aquele modelo como o ativo E SHALL marcar a conexão dona dele como a conexão ativa, numa única ação
2. WHEN a conexão ativa muda para outra THEN o modelo ativo anterior SHALL deixar de ser ativo, já que pertence a outra conexão
3. WHEN existe modelo ativo THEN ele SHALL sempre pertencer à conexão ativa — o sistema NÃO SHALL permitir a combinação inconsistente
4. WHEN o usuário consulta qual é o par ativo THEN o sistema SHALL devolver conexão e modelo juntos, ou explicitamente "nenhum"

**Independent Test**: Com modelo X da conexão A ativo, escolher o modelo Y da conexão B; confirmar que o ativo virou (B, Y) e que (A, X) não está mais marcado.

---

### P1: Inspecionar modelos de conexões inativas ⭐ MVP

**User Story**: Como usuário, quero ver os modelos instalados em qualquer conexão disponível, mesmo nas que não estão ativas, para decidir para qual quero trocar.

**Why P1**: Sem isso, escolher exige ativar às cegas — o usuário decidiu explicitamente por esse comportamento.

**Acceptance Criteria**:

1. WHEN a aba Modelos é aberta THEN o sistema SHALL listar os modelos instalados de todas as conexões com status "disponível", agrupados por conexão, e não apenas os da ativa
2. WHEN uma conexão está indisponível THEN o sistema SHALL indicar isso no lugar da lista dela, sem quebrar a listagem das demais

**Independent Test**: Com Ollama ativa e LM Studio disponível mas inativa, confirmar que os modelos das duas aparecem na aba Modelos.

---

### P1: Migração de schema versionada ⭐ MVP

**User Story**: Como desenvolvedor, quero que mudanças de coluna sejam aplicadas em bancos já existentes, para que essa alteração não quebre silenciosamente o banco que já está na máquina.

**Why P1**: `connections.enabled` precisa virar `is_active`, e o schema atual (`CREATE TABLE IF NOT EXISTS`) vira no-op em banco existente — a mudança simplesmente não aconteceria (C-01 em CONCERNS.md).

**Acceptance Criteria**:

1. WHEN o app abre um banco criado antes desta mudança THEN o sistema SHALL aplicar a migração e o banco SHALL ficar com o schema novo, preservando as conexões já cadastradas
2. WHEN o app abre um banco já migrado THEN o sistema NÃO SHALL reaplicar a migração
3. WHEN o banco é criado do zero THEN o sistema SHALL chegar exatamente ao mesmo schema de um banco migrado
4. WHEN a migração converte `enabled` para `is_active` E mais de uma conexão estava habilitada THEN o sistema SHALL manter no máximo uma como ativa (a mais antiga por `created_at`), nunca duas

**Independent Test**: Criar um banco no schema antigo com duas conexões habilitadas, rodar a abertura, confirmar `user_version` incrementado e exatamente uma conexão ativa.

---

## Edge Cases

- WHEN a conexão ativa fica indisponível (runtime foi fechado) THEN o sistema SHALL manter a escolha do usuário e apenas refletir o status, sem trocar sozinho
- WHEN o modelo ativo some da conexão (removido externamente) THEN o sistema SHALL sinalizar "modelo não encontrado" e tratar como se não houvesse modelo ativo, mantendo a conexão ativa
- WHEN o usuário desativa a conexão ativa sem ativar outra THEN o modelo ativo SHALL também deixar de ser ativo (não faz sentido um modelo ativo sem conexão)
- WHEN duas conexões apontam para a mesma URL THEN cada uma continua sendo uma entrada separada — a unicidade é de "ativa", não de URL

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| --- | --- | --- | --- |
| ACTIVE-01 | P1: Ativar conexão desativa a anterior | Tasks | Complete |
| ACTIVE-02 | P1: Estado "nenhuma ativa" é válido e visível | Tasks | Complete |
| ACTIVE-03 | P1: Todas as conexões listadas com status, ativa destacada | Tasks | Complete |
| ACTIVE-04 | P1: Ativar conexão indisponível é permitido, mas sinalizado | Tasks | Complete |
| ACTIVE-05 | P1: Escolher modelo ativa sua conexão na mesma ação | Tasks | Complete |
| ACTIVE-06 | P1: Modelo ativo sempre pertence à conexão ativa (invariante) | Tasks | Complete |
| ACTIVE-07 | P1: Consultar o par ativo (conexão + modelo) num único retorno | Tasks | Complete |
| ACTIVE-08 | P1: Modelos de conexões disponíveis inativas são inspecionáveis | Tasks | Complete |
| ACTIVE-09 | P1: Migração versionada aplicada em banco existente | Tasks | Complete |
| ACTIVE-10 | P1: Migração normaliza múltiplos habilitados para um ativo | Tasks | Complete |

**ID format:** `ACTIVE-[NUMBER]`
**Coverage:** 10 total, 10 mapeados para tasks, 0 não mapeados — **10 implementados (2026-07-25)**

---

## Success Criteria

- [x] É impossível chegar num estado com duas conexões ativas ou dois modelos ativos, por qualquer caminho da UI
- [x] É impossível ter modelo ativo de uma conexão diferente da ativa
- [x] O banco existente na máquina do dev migra sem perder as conexões cadastradas
- [x] Um único comando devolve "quem responde agora" — sem o frontend precisar cruzar duas listas
