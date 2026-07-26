# Configurações, Storage & i18n — Specification

## Problem Statement

Antes de baixar modelos ou indexar documentos, o app precisa saber **onde** guardar os dados, em **qual idioma** falar e com **qual tema** aparecer. Hoje o M1 grava tudo em `app_data_dir` fixo, a UI está só em português e não há tela de configurações. Esta feature entrega a camada de configuração base (pasta de armazenamento, idioma, tema) e o wizard de primeiro uso, sobre a qual os próximos milestones (modelos, documentos) se apoiam.

## Goals

- [ ] Usuário define uma pasta-base de armazenamento; todos os dados do app passam a viver nela
- [ ] UI internacionalizada com inglês padrão e português disponível, trocável em tempo real
- [ ] Sistema de temas (claro, escuro + extras) trocável em tempo real
- [ ] Wizard de primeiro uso coleta pasta + tema + idioma antes de entrar no app
- [ ] Nova seção "Configurações" na sidebar para editar tudo isso depois

## Out of Scope

| Feature | Reason |
| --- | --- |
| Config durante a instalação (NSIS/MSI) | AD-010 — substituído pelo wizard de 1º uso; deferido |
| Download/gestão de modelos na pasta `models/` | M3 |
| Ingestão de documentos na pasta `documents/` | M5 |
| Migração automática de dados ao trocar de pasta com conteúdo existente | v1 só move o `localmind.db`; migração completa é edge futura |

---

## User Stories

### P1: Escolher a pasta de armazenamento ⭐ MVP

**User Story**: Como usuário, quero escolher em que pasta do computador o app guarda modelos, documentos e dados, para controlar onde meus arquivos ficam.

**Why P1**: Modelos e documentos podem ocupar muitos GB; o usuário precisa decidir o local antes de baixar/importar. É pré-requisito de M3/M5.

**Acceptance Criteria**:

1. WHEN o usuário escolhe uma pasta-base THEN o sistema SHALL criar a estrutura `models/`, `documents/`, `vectors/`, `chats/` dentro dela
2. WHEN a pasta-base é definida THEN o sistema SHALL guardar o `localmind.db` nela e persistir o caminho escolhido
3. WHEN a pasta escolhida não existe ou é inválida/sem permissão de escrita THEN o sistema SHALL exibir erro e manter a pasta anterior
4. WHEN o app reinicia THEN o sistema SHALL reabrir usando a pasta-base persistida

**Independent Test**: Escolher uma pasta, reiniciar, confirmar que os subdiretórios existem e o `.db` está lá.

---

### P1: Trocar idioma (EN padrão, PT disponível) ⭐ MVP

**User Story**: Como usuário, quero usar o app em inglês ou português, para entender a interface.

**Why P1**: Requisito explícito; retrofit de i18n depois é caro, então toda a UI já nasce com chaves de tradução.

**Acceptance Criteria**:

1. WHEN o app roda pela 1ª vez THEN o idioma padrão SHALL ser inglês
2. WHEN o usuário troca o idioma THEN toda a UI SHALL atualizar sem reiniciar o app
3. WHEN o idioma é alterado THEN a escolha SHALL persistir entre sessões
4. WHEN uma string não tem tradução no idioma ativo THEN o sistema SHALL cair no inglês (fallback) sem quebrar

**Independent Test**: Alternar EN↔PT em Configurações e ver textos mudarem na hora; reiniciar e manter o idioma.

---

### P1: Trocar tema (claro/escuro/extras) ⭐ MVP

**User Story**: Como usuário, quero escolher o tema de cores, incluindo claro, escuro e outras opções, para conforto visual.

**Why P1**: Requisito explícito; o theme system precisa existir antes das telas ricas de M3+.

**Acceptance Criteria**:

1. WHEN o usuário seleciona um tema THEN a UI SHALL aplicar as cores imediatamente (CSS variables)
2. WHEN há pelo menos 3 temas (claro, escuro e ao menos um extra) THEN todos SHALL estar selecionáveis
3. WHEN o tema é alterado THEN a escolha SHALL persistir entre sessões
4. WHEN o app abre THEN o tema persistido SHALL ser aplicado antes do primeiro render visível (sem flash)

**Independent Test**: Trocar entre claro/escuro/extra e ver a mudança na hora; reiniciar e manter.

---

### P1: Wizard de primeiro uso ⭐ MVP

**User Story**: Como novo usuário, quero um assistente na primeira abertura para definir pasta, tema e idioma, para começar configurado.

**Why P1**: Substitui a config de instalação (AD-010); garante que a pasta-base exista antes de qualquer uso.

**Acceptance Criteria**:

1. WHEN o app abre e ainda não há config salva THEN o sistema SHALL exibir o wizard antes da tela principal
2. WHEN o wizard pede pasta, tema e idioma THEN o sistema SHALL sugerir defaults (pasta padrão do SO, tema escuro, inglês)
3. WHEN o usuário conclui o wizard THEN o sistema SHALL persistir as escolhas, criar a estrutura de pastas e abrir o app
4. WHEN a config já existe (execuções seguintes) THEN o wizard SHALL ser pulado

**Independent Test**: Apagar a config, abrir o app, completar o wizard e cair no app configurado; reabrir e não ver o wizard.

---

### P2: Seção Configurações na sidebar

**User Story**: Como usuário, quero uma seção de Configurações na sidebar para revisar/alterar pasta, tema e idioma depois.

**Why P2**: O wizard cobre o 1º uso; editar depois é importante mas não bloqueia o MVP inicial.

**Acceptance Criteria**:

1. WHEN o usuário abre Configurações THEN o sistema SHALL mostrar os controles de tema, idioma e pasta com os valores atuais
2. WHEN o usuário altera qualquer valor THEN o sistema SHALL aplicar/persistir conforme as stories P1 correspondentes

**Independent Test**: Abrir Configurações, mudar cada campo e confirmar efeito + persistência.

---

## Edge Cases

- WHEN a pasta-base persistida sumiu/foi movida entre sessões THEN o sistema SHALL avisar e reabrir o wizard ou pedir nova pasta
- WHEN o usuário escolhe uma pasta sem permissão de escrita THEN o sistema SHALL bloquear e explicar
- WHEN o usuário troca para uma pasta nova THEN o sistema SHALL criar a estrutura lá e mover o `localmind.db` (dados de chat permanecem); documentos/modelos antigos NÃO são migrados automaticamente no v1 (avisar)
- WHEN o arquivo de config está corrompido THEN o sistema SHALL cair nos defaults e reabrir o wizard sem travar

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| --- | --- | --- | --- |
| CFG-01 | P1: Pasta de armazenamento | Implemented | Implemented |
| CFG-02 | P1: Estrutura de pastas + realocar .db | Implemented | Implemented |
| CFG-03 | P1: Idioma EN padrão + PT (i18n) | Implemented | Implemented |
| CFG-04 | P1: Fallback de tradução p/ EN | Implemented | Implemented |
| CFG-05 | P1: Temas claro/escuro/extra (CSS vars) | Implemented | Implemented |
| CFG-06 | P1: Persistência de tema/idioma/pasta | Implemented | Implemented |
| CFG-07 | P1: Wizard de 1º uso | Implemented | Implemented |
| CFG-08 | P2: Seção Configurações na sidebar | Implemented | Implemented |

**ID format:** `CFG-[NUMBER]`
**Status values:** Pending → In Design → In Tasks → Implementing → Verified
**Coverage:** 8 total, 8 implementados. A tabela ficou marcada como `Pending` do planejamento (2026-07-24) até a auditoria de 2026-07-26, apesar de o M2 estar `✅ COMPLETE` no ROADMAP desde 2026-07-24 — os requisitos estavam prontos, o documento é que não acompanhou.

**Edge case da pasta que some — fechado em 2026-07-26.** *"WHEN a pasta-base persistida sumiu/foi movida entre sessões THEN o sistema SHALL avisar e reabrir o wizard"* era o único item desta spec implementado só pela metade: o boot registrava um `eprintln!`, deixava o banco fechado, e o app abria com aparência normal e **todos** os comandos falhando com "Nenhuma pasta de armazenamento configurada ainda". Agora `config::evaluate_storage` decide, o comando `get_storage_status` responde, e o wizard reabre nomeando a pasta perdida — mantendo tema, idioma e o caminho anterior preenchido, para o caso de ser só um drive removível que voltou. 4 testes unitários cobrem a decisão.

---

## Success Criteria

- [ ] 1ª abertura mostra o wizard; escolhas persistem e o wizard não reaparece
- [ ] Pasta-base escolhida contém `models/`, `documents/`, `vectors/`, `chats/` e o `localmind.db`
- [ ] Alternar idioma (EN↔PT) e tema (claro/escuro/extra) reflete na hora e sobrevive a reinício
- [ ] Toda string visível da UI passa pela camada i18n (sem texto hardcoded)
