# App Shell & Gerenciamento de Chats — Specification

> **Status: ✅ IMPLEMENTADO e verificado (2026-07-24).** Backend compila, janela abre, `localmind.db` criado e migrado. Falta apenas a verificação manual dos fluxos de CRUD na UI (clicar). Ver STATE.md L-002. A sidebar ganha uma 4ª seção (Configurações) no M2.

## Problem Statement

O produto precisa de um esqueleto funcional (walking skeleton) antes de qualquer IA: uma janela desktop com a sidebar de 3 zonas (Chats, Documentos, Conexões) e a capacidade de criar e persistir conversas localmente. Sem essa base, não há onde plugar streaming, RAG ou conexões. Esta feature entrega a fundação instalável e navegável.

## Goals

- [ ] App Tauri abre uma janela com layout de sidebar de 3 zonas + área de conteúdo principal
- [ ] Usuário cria, lista, renomeia e exclui chats; dados persistem em SQLite entre reinícios
- [ ] Histórico de mensagens é isolado por chat (base para AD-004)

## Out of Scope

| Feature                          | Reason                                          |
| --------------------------------- | ------------------------------------------------ |
| Envio de mensagem a um LLM       | Depende do Connection Manager (M2)              |
| Ingestão/indexação de documentos | Feature de M3                                    |
| Detecção de conexões             | Zona "Conexões" renderiza placeholder até M2    |
| Streaming e system prompt        | M2                                               |

---

## User Stories

### P1: Abrir o app com a sidebar de 3 zonas ⭐ MVP

**User Story**: Como usuário, quero abrir o app e ver uma sidebar com Chats (topo), Documentos (meio) e Conexões (base), para navegar pelas áreas principais.

**Why P1**: É a moldura visual de todo o produto; nada funciona sem ela.

**Acceptance Criteria**:

1. WHEN o app inicia THEN o sistema SHALL exibir uma janela com sidebar à esquerda e painel principal à direita
2. WHEN a sidebar é renderizada THEN o sistema SHALL mostrar 3 seções nesta ordem vertical: Chats (topo, área rolável), Documentos (meio), Conexões (base)
3. WHEN não há chat selecionado THEN o painel principal SHALL exibir um estado vazio com instrução para criar um chat
4. WHEN Documentos e Conexões ainda não têm backend THEN o sistema SHALL exibir placeholders claros ("em breve") sem quebrar o layout

**Independent Test**: Rodar o app; ver janela com as 3 zonas na ordem correta e o estado vazio no painel.

---

### P1: Criar e listar chats ⭐ MVP

**User Story**: Como usuário, quero criar um novo chat e vê-lo na lista, para iniciar conversas.

**Why P1**: Ação central do produto.

**Acceptance Criteria**:

1. WHEN o usuário clica em "Novo chat" THEN o sistema SHALL criar um chat com título padrão e selecioná-lo
2. WHEN um chat é criado THEN o sistema SHALL persisti-lo em SQLite e exibi-lo no topo da lista de Chats
3. WHEN o app é reiniciado THEN o sistema SHALL recarregar todos os chats persistidos na ordem de atualização mais recente
4. WHEN o usuário seleciona um chat THEN o sistema SHALL marcá-lo como ativo e carregar suas mensagens (vazio nesta feature)

**Independent Test**: Criar 2 chats, reiniciar o app, confirmar que ambos reaparecem e são selecionáveis.

---

### P2: Renomear e excluir chats

**User Story**: Como usuário, quero renomear e excluir chats, para organizar minhas conversas.

**Why P2**: Importante para organização, mas não bloqueia o MVP de criar/listar.

**Acceptance Criteria**:

1. WHEN o usuário renomeia um chat THEN o sistema SHALL salvar o novo título e refletir na lista imediatamente
2. WHEN o usuário exclui um chat THEN o sistema SHALL remover o chat e suas mensagens do SQLite e da lista
3. WHEN o chat excluído era o ativo THEN o sistema SHALL voltar ao estado vazio ou selecionar o próximo chat

**Independent Test**: Renomear um chat e confirmar persistência após reiniciar; excluir e confirmar que não retorna.

---

## Edge Cases

- WHEN o banco SQLite não existe no primeiro início THEN o sistema SHALL criá-lo e rodar migrações automaticamente
- WHEN a lista de chats está vazia THEN a zona Chats SHALL exibir um estado vazio com CTA "Novo chat"
- WHEN o usuário exclui o único chat existente THEN o painel principal SHALL retornar ao estado vazio sem erro
- WHEN o título do chat é muito longo THEN a sidebar SHALL truncar visualmente sem quebrar o layout
- WHEN há falha ao ler/escrever no SQLite THEN o sistema SHALL exibir erro amigável e não travar a UI

---

## Requirement Traceability

| Requirement ID | Story                          | Phase  | Status  |
| --------------- | ------------------------------- | ------ | ------- |
| SHELL-01       | P1: Sidebar de 3 zonas         | Tasks  | Pending |
| SHELL-02       | P1: Estado vazio do painel     | Tasks  | Pending |
| SHELL-03       | P1: Criar chat                 | Tasks  | Pending |
| SHELL-04       | P1: Persistir/listar chats     | Tasks  | Pending |
| SHELL-05       | P1: Selecionar chat ativo      | Tasks  | Pending |
| SHELL-06       | P2: Renomear chat              | Tasks  | Pending |
| SHELL-07       | P2: Excluir chat + mensagens   | Tasks  | Pending |
| SHELL-08       | Edge: init DB + migrações      | Tasks  | Pending |

**ID format:** `SHELL-[NUMBER]`
**Status values:** Pending → In Design → In Tasks → Implementing → Verified
**Coverage:** 8 total, 8 mapeados para design/tasks, 0 não mapeados

---

## Success Criteria

- [ ] App instalável abre em Windows e Linux mostrando a sidebar de 3 zonas
- [ ] Criar 2+ chats, reiniciar, e todos persistem e são selecionáveis
- [ ] Renomear e excluir refletem no SQLite após reinício
- [ ] Layout não quebra com lista vazia, títulos longos ou placeholders das zonas ainda sem backend
