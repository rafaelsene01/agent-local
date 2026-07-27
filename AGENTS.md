# AGENTS.md

Instruções para agentes de código trabalhando no LocalMind — um chat de IA desktop que roda o modelo, os embeddings e o banco vetorial inteiramente na máquina do usuário.

Este arquivo é a fonte única. `CLAUDE.md` apenas aponta para cá.

---

## Antes de qualquer coisa: leia o STATE

`.specs/project/STATE.md` é a memória do projeto. O topo dele diz em que ponto o trabalho está **agora**, e a lista de decisões (AD-xxx) diz por que cada escolha foi feita — incluindo as que foram revogadas depois.

Não presuma o estado a partir do código: já aconteceu de o ROADMAP dizer "não implementado" sobre algo pronto no mesmo dia. Quando encontrar essa divergência, corrija o documento.

**Estado atual (2026-07-27):** o M9 está no meio. O backend já colapsou para um único runtime (`LlamaServerClient`), mas o frontend ainda chama comandos que não existem mais (`list_connections`, `pull_model`, `get_active_pair`). Como o `invoke` do Tauri recebe o nome como string, **o build passa e a quebra só aparece em runtime**. A Fase 2 (T7–T11 de `self-contained-runtime`) é o que fecha isso.

---

## Como este projeto trabalha

Desenvolvimento dirigido por spec, com a estrutura em `.specs/`:

| Onde | O que é |
| --- | --- |
| `project/PROJECT.md` | visão e escopo |
| `project/ROADMAP.md` | milestones e o que cada um entrega |
| `project/STATE.md` | decisões (AD), lições (L), bloqueadores, todos |
| `codebase/` | mapeamento do código existente: stack, arquitetura, convenções, testes, concerns |
| `features/<nome>/` | `spec.md` (requisitos com IDs rastreáveis), `design.md`, `tasks.md` |

Toda funcionalidade tem requisitos com ID (`CHAT-11`, `SELF-06`, `SIDE-04`…). Ao mexer no código que implementa um requisito, atualize a tabela de rastreabilidade da spec correspondente — e atualize com o que é verdade, não com o que se pretendia.

---

## A regra mais importante: "compila" não é "verificado"

Este repositório separa deliberadamente as duas coisas, e a separação já pagou várias vezes. Casos reais registrados:

- Um `ci.yml` foi dado como pronto com a evidência "YAML validado". Na primeira execução real ele falhou (L-005).
- Seis requisitos foram marcados como implementados quando só o backend existia; a UI não fechava o ciclo (AD-027).
- Um teste de "não há janela de console" passou por um motivo errado e teria sido registrado como prova se não tivesse sido questionado (AD-041).

Portanto, ao relatar trabalho:

- diga **o que foi executado**, não o que deveria funcionar;
- se algo não foi exercitado, diga isso com todas as letras, na mesma frase em que descreve o que foi feito;
- prefira uma evidência medida (um número, uma saída de comando) a um adjetivo.

Quando um teste automatizado não conseguir provar algo, escreva **dentro do teste** por que ele é inconclusivo, para ninguém depois o ler como prova.

---

## Comandos

```bash
# Backend: a suíte inteira (o gate padrão)
cd src-tauri && cargo test --lib

# Backend: só compilar
cd src-tauri && cargo check --lib

# Frontend: tsc + Vite
npm run build

# Scripts de release (Node puro)
npm run test:scripts

# Rodar o app de verdade
npm run tauri dev
```

`cargo test` está em **146 passando** e alguns `#[ignore]`. Se o número cair, cada teste perdido precisa de justificativa — remoção legítima (o código que ele testava saiu) é aceitável; deleção silenciosa não.

**Pré-requisito de build que não é óbvio:** o `lancedb` exige o compilador **protoc**. Sem ele o `cargo build` falha com *"Could not find `protoc`"*. Windows: `winget install Google.Protobuf`. Linux: `apt install protobuf-compiler`.

**Node 22+** é obrigatório: o `npm run test:scripts` depende de expansão de glob que versões anteriores não têm.

---

## Testes: o que exige cobertura

Da matriz em `.specs/codebase/TESTING.md`:

| Camada | Tipo de teste |
| --- | --- |
| Funções puras Rust (parsing, chunking, montagem de contexto, fórmulas) | unit — obrigatório |
| Comandos Tauri que só orquestram I/O | nenhum (não há runner de integração Tauri) |
| Componentes React | nenhum (não há Vitest/RTL) |
| Scripts Node | unit via `node --test` |

`#[cfg(test)] mod tests` fica **no fim do mesmo arquivo**, nunca num diretório `tests/` separado.

Quando algo só pode ser provado contra um recurso real, use `#[ignore]` — o teste fica no repositório, documentado e repetível, sem pesar na suíte padrão. Há dois formatos em uso:

- **recurso que o teste cria**, como um LanceDB numa pasta temporária: `rag::store`;
- **recurso que já existe na máquina** (o binário do llama.cpp, um banco de verdade): o caminho vem de **variável de ambiente** e nunca é adivinhado — `db::real_database`, `runtime::detect::detect_real`, `runtime::process::sidecar_real`.

O segundo formato é o que impede um teste de encostar por acidente nos dados do usuário.

---

## Nunca faça

**Não toque nos dados reais do usuário.** A pasta-base fica fora do repositório e contém as conversas dele. Para validar uma migração, **copie o banco** para o scratchpad, migre a cópia e apague. O original nunca é aberto para escrita por um teste.

**Não deixe arquivo de diagnóstico temporário no repositório.** Já havia um (`rag/diag.rs`) com caminhos absolutos da máquina do usuário, órfão de qualquer `mod`, meses depois de a investigação ter terminado. Se criar um, apague antes de terminar.

**Não faça force-push nem reescreva `master`.** Desfazer é `git revert`, mesmo que fique feio no histórico.

**Não commite sem o usuário pedir.** O padrão é deixar as mudanças no working tree.

**Não dispare release.** O workflow é `workflow_dispatch` manual, de propósito: nenhum push publica nada.

---

## Banco de dados

Migrações versionadas por `PRAGMA user_version`, numa lista ordenada em `db.rs`, cada uma em transação. **A próxima é a 8.**

`CREATE TABLE IF NOT EXISTS` sozinho não migra banco existente — vira no-op silencioso. Mudança de coluna exige entrada nova na lista.

**Chaves estrangeiras são aplicadas** (`PRAGMA foreign_keys = ON` no `open`). Isso muda a ordem de operações destrutivas: derrube a tabela que referencia antes da referenciada, senão falha.

Toda migração destrutiva deve ser ensaiada contra uma cópia de um banco real antes de ser considerada pronta.

---

## Convenções de código

Detalhe completo em `.specs/codebase/CONVENTIONS.md`. O essencial:

- **Comentários em inglês, no código.** Explicam **por quê**, não o quê — de preferência ancorados numa medição ou num caso real ("verified live against llama-server: n_ctx_train = 131072"). Comentário que repete o nome da função é ruído.
- **Prosa de documentação em português** (README, `.specs/`, este arquivo). Código, nomes e mensagens de commit em inglês.
- **Commits em Conventional Commits**, em inglês, um por task. O CI valida isso em PRs.
- Arquivos Rust `snake_case.rs`; comandos Tauri sempre em arquivo com sufixo `_commands`.
- Componentes React `PascalCase.tsx`, um por arquivo. Wrappers de `invoke` em `*Api.ts`, stores Zustand em `*Store.ts`.
- Campos que cruzam a fronteira Rust↔TS são `snake_case` dos dois lados (o serde não renomeia) — `src/types.ts` quebra o camelCase de propósito. A exceção são os **parâmetros** de `invoke()`, que vão camelCase e chegam snake_case; o Tauri converte.
- `src/types.ts` espelha as structs Rust **à mão**. Mudou uma, mude a outra — não há geração.

---

## Armadilhas conhecidas desta base

**i18n tem paridade obrigatória.** `en.json` e `pt.json` precisam ter exatamente as mesmas chaves. Adicionou uma, adicione nos dois.

**Streaming não passa pelo retorno do comando.** Comandos Tauri são request/response; tokens chegam por evento (`chat-stream-chunk`). Mesmo padrão para progresso de download e status de indexação.

**Um modelo pequeno responde ao que está perto da pergunta.** Os trechos recuperados entram no mesmo turno da pergunta, não num bloco `system` no topo — mudar isso fez o modelo parar de copiar as próprias respostas anteriores (AD-033).

**O orçamento do prompt reserva o que a resposta vai usar.** Se mexer em `answer_token_budget`, mexa no `budget_chars` junto: eles se referem ao mesmo espaço.

**Turnos precisam alternar.** Uma geração cancelada deixa a pergunta sem resposta, e dois `user` seguidos fazem o modelo divagar em vez de responder.

**O sidecar é morto por Job Object no Windows**, além do `kill` explícito. Se mexer no spawn, não remova nenhum dos dois: um cobre o fechamento normal, o outro cobre o kill forçado.

---

## Ao terminar uma tarefa

1. Rode os gates (`cargo test`, `npm run build`, e `npm run test:scripts` se mexeu em `scripts/`).
2. Atualize a rastreabilidade da spec afetada com o que ficou **verificado** e o que ficou pendente.
3. Registre uma decisão nova em `STATE.md` se você escolheu algo não óbvio, com o motivo e o trade-off — inclusive quando a escolha foi sua e não do usuário.
4. Relate o que **não** foi verificado. Essa parte não é opcional.
