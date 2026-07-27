# Todo ajuste passa pela spec

Esta regra vale para **qualquer** pedido de ajuste, correção ou melhoria neste repositório — do rename de uma variável ao milestone inteiro. Ela não tem `paths` no frontmatter de propósito: carrega em toda sessão, porque decide *como* o trabalho começa, não *onde* ele acontece.

## 1. Na dúvida, pergunte antes de escrever código

Se o pedido admite duas leituras que levariam a trabalhos diferentes, **pergunte**. Não escolha a interpretação mais provável e siga.

Perguntar é obrigatório quando:

- o pedido não deixa claro **qual requisito** ele altera, cria ou revoga;
- ele parece contradizer uma decisão já registrada em `.specs/project/STATE.md` (AD-xxx) — cite a AD na pergunta;
- ele toca uma feature que outra spec também implementa, e o comportamento esperado das duas juntas não está óbvio;
- há mais de um lugar razoável para a mudança morar (`runtime/` vs `providers/`, store vs componente);
- ele implica remover algo que hoje funciona, e não está dito se a remoção é intencional.

Pergunte **antes** de planejar, não no meio da implementação. Uma pergunta feita depois de três arquivos editados custa o retrabalho dos três.

Não pergunte o que o repositório já responde: leia `.specs/` e o código primeiro. Pergunta boa é a que só o usuário pode responder.

## 2. Planejamento sempre pelo `tlc-spec-driven`

Invoque a skill `tlc-spec-driven` para planejar o ajuste — inclusive os pequenos, onde ela cai no modo rápido. Não improvise um plano fora dela.

O que a skill produz (`spec.md`, `design.md`, `tasks.md`, ou o registro de quick task) é o que autoriza a edição. Código antes de plano é a ordem errada.

## 3. Todo arquivo criado ou editado leva o marcador `SPEC:`

Uma linha no **topo do arquivo**, antes dos imports, listando a feature e os IDs de requisito que aquele arquivo implementa:

```rust
// SPEC: sidecar-lifecycle (SIDE-04, SIDE-05, SIDE-09)

use std::process::Command;
```

```tsx
// SPEC: chat-messaging (CHAT-11, CHAT-12)

import { useEffect } from "react";
```

Regras do marcador:

- **Em inglês**, como todo comentário de código desta base (`AGENTS.md`).
- O nome da feature é o **nome exato da pasta** em `.specs/features/`.
- Os IDs são os reais da spec. Prefixos em uso: `SHELL`, `CHAT`, `CONN`, `DOC`, `EMBED`, `SELF`, `SIDE`, `REL`, `CFG`, `ACTIVE`. **Não invente ID** — se o requisito não existe, ele precisa existir na spec primeiro.
- Editou um arquivo e o escopo mudou? **Atualize o marcador no mesmo commit.** Marcador desatualizado é pior que marcador ausente: ele mente com autoridade.
- Arquivo que implementa requisito de mais de uma feature lista as duas: `// SPEC: chat-messaging (CHAT-11), documents-rag (DOC-07)`.

**Arquivos sem sintaxe de comentário** (`en.json`, `pt.json`, `tauri.conf.json`) não recebem marcador — a rastreabilidade deles vive na tabela da spec, e é lá que ela precisa ser atualizada.

**Arquivo de infraestrutura que não implementa requisito nenhum** (um script de build, um config) fica sem marcador. Não force um ID só para preencher a linha; se você não consegue nomear o requisito, provavelmente falta spec — veja o item 4.

Verificação: `grep -rn "SPEC:" src/ src-tauri/src/`.

## 4. Spec nova que encosta em spec antiga

Antes de implementar, verifique **o que mais depende daquilo**. O marcador `SPEC:` existe justamente para isso: `grep` pelo ID revela quem mais toca o mesmo território.

Dois casos, com desfechos diferentes:

**A mudança convive com o que já existe.** Então as duas specs continuam válidas, e você precisa **provar que ambas continuam funcionando** — não afirmar. Rode o teste que cobre a spec antiga; se não houver, esse é o teste que falta escrever. Atualize a rastreabilidade das duas.

**A mudança revoga a feature antiga.** Então a spec antiga não pode ficar descrevendo um recurso que saiu. Remover código sem remover a spec cria exatamente o problema que o `AGENTS.md` registra: documentação que descreve um projeto que não existe mais. Nesse caso:

- marque na spec antiga o que foi revogado, **por qual spec e em que decisão** (AD-xxx) — não apague o requisito em silêncio, o histórico do "por quê" tem valor;
- atualize `.specs/project/ROADMAP.md` e o topo de `STATE.md` se o escopo do milestone mudou;
- registre a AD nova em `STATE.md` com o trade-off;
- apague os marcadores `SPEC:` que apontam para o requisito revogado, junto com o código que saiu.

O critério final é único: **`.specs/` descreve o que o projeto oferece hoje.** Se um leitor puder acreditar numa spec e estar errado sobre o app, a spec está quebrada e consertá-la faz parte da tarefa — não é trabalho para depois.

## 5. Ao relatar

Vale o que o `AGENTS.md` já manda: diga o que foi **executado**, não o que deveria funcionar. Some a isso, sempre:

- quais specs foram atualizadas e o que ficou pendente nelas;
- quais marcadores `SPEC:` você adicionou ou mexeu;
- se uma spec antiga foi afetada, **como** você verificou que ela continua valendo — ou que ela deixou de valer de propósito.
