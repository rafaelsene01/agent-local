# Sidecar sem console e com ciclo de vida garantido — Specification

> **Status: IMPLEMENTADO e verificado contra o sidecar real (2026-07-26).** As 8 tasks foram executadas. O teste ponta a ponta subiu o `llama-server` de verdade, capturou 1131 bytes de log e viu o kernel encerrá-lo ao fechar o job. **Sobra um único item, e ele exige olhos:** confirmar na barra de tarefas que nenhuma janela de console aparece — o teste automatizado sai INCONCLUSIVO quando roda de um terminal, pelo motivo explicado no `tasks.md`. Continuação do M7 (`embedded-runtime`), que entregou o sidecar funcionando mas deixou três pontas soltas de integração com o sistema operacional.

## Problem Statement

Ao abrir o LocalMind no Windows, **uma janela de terminal preta aparece junto** e fica ali enquanto o app roda. É o `llama-server.exe`, que é uma aplicação de console: o `Command::spawn()` em `runtime/process.rs:92` não passa nenhuma flag de criação, então o Windows dá a ele um console próprio. Para o usuário, o app "abre duas coisas", uma delas com cara de erro — e fechar essa janela por engano mata o motor de IA sem nenhum aviso na interface.

O segundo problema é mais silencioso: o sidecar só é morto no `Drop` e no `RunEvent::ExitRequested`. Isso cobre o fechamento normal — verificado na AD-028 — mas **não cobre o LocalMind ser morto à força** (Gerenciador de Tarefas, crash, `taskkill`, fim de sessão do Windows). Nesses casos o `llama-server.exe` fica órfão, segurando a porta, o modelo carregado e vários GB de RAM, sem nenhuma interface que o mostre. O usuário só descobre reabrindo o app e vendo tudo lento — ou nunca.

O terceiro é consequência de resolver o primeiro: hoje os logs do `llama-server` aparecem naquele console. Foi lendo `stop: cancel task` nele que a AD-028 achou o bug do timeout de 5 s. Esconder a janela sem capturar a saída trocaria um incômodo visual por uma cegueira de diagnóstico.

## Goals

- [ ] Abrir o LocalMind mostra **uma** janela: a do app. Nenhum console, nem persistente nem piscando
- [ ] O `llama-server` morre junto com o LocalMind **em qualquer forma de encerramento**, inclusive `taskkill /F` e crash — verificado com o Gerenciador de Tarefas aberto
- [ ] A saída do `llama-server` continua acessível para diagnóstico, em arquivo, sem console

## Out of Scope

| Item | Motivo |
| --- | --- |
| Uma aba "Logs" na interface | O arquivo resolve o diagnóstico; UI para log é outra feature, com outra discussão (filtro, tail, tamanho) |
| Matar órfãos que já existem de execuções anteriores | Varrer processos por nome e matar é uma operação perigosa (o usuário pode ter um `llama-server` próprio rodando). Ver Deferred Ideas |
| Linux/macOS | No Linux não há console parasita: o processo herda o terminal do pai e o app roda sem um. O `Drop`/`ExitRequested` já cobre o encerramento normal, e o kill-on-close via `prctl(PR_SET_PDEATHSIG)` fica registrado como ideia adiada |
| Esconder o console do `onnxruntime`/`pdfium` | São bibliotecas carregadas em processo, não processos filhos — não têm console |

---

## User Stories

### P1: Nenhuma janela de console ⭐ MVP

**User Story**: Como usuário, quero que abrir o LocalMind abra só o LocalMind, para não ver uma janela preta que parece erro e que eu posso fechar por engano.

**Why P1**: É o sintoma que o usuário relatou, e o mais visível. Também é o de menor risco — uma flag na criação do processo.

**Acceptance Criteria**:

1. WHEN o app inicia o sidecar (no boot ou por ação do usuário) THEN o sistema SHALL criar o processo com `CREATE_NO_WINDOW`, e nenhuma janela de console SHALL aparecer
2. WHEN o app roda a detecção de GPU (`llama-server --list-devices`) THEN o sistema SHALL usar a mesma flag — hoje esse comando pisca um console por um instante
3. WHEN o sistema operacional não é Windows THEN o comportamento SHALL permanecer exatamente o de hoje, sem `#[cfg]` espalhado pelo fluxo de spawn

**Independent Test**: Abrir o app no Windows e observar a barra de tarefas: só o LocalMind. Trocar de modelo (que reinicia o sidecar) e confirmar que nada pisca.

---

### P1: O sidecar não sobrevive ao app ⭐ MVP

**User Story**: Como usuário, quero que matar o LocalMind mate também o motor de IA, para não ficar com vários GB de RAM ocupados por um processo que eu não tenho como ver nem fechar.

**Why P1**: É a parte "controlado pelo LocalMind" do pedido. Sem isso, esconder a janela **piora** o problema: hoje o console órfão pelo menos é visível e fechável; escondido, o órfão fica invisível.

**Acceptance Criteria**:

1. WHEN o app inicia o sidecar THEN o sistema SHALL associá-lo a um Job Object com `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, de modo que o encerramento do processo do app — por qualquer via — feche o job e o kernel mate o filho
2. WHEN o LocalMind é encerrado com `taskkill /F` ou pelo Gerenciador de Tarefas THEN nenhum `llama-server.exe` SHALL permanecer em execução
3. WHEN o app fecha normalmente THEN o comportamento atual (`kill` explícito em `RunEvent::ExitRequested` e no `Drop`) SHALL continuar valendo — o Job Object é rede de segurança, não substituto
4. WHEN o Job Object não pode ser criado ou o processo não pode ser associado a ele THEN o sistema SHALL registrar o motivo e **iniciar o sidecar assim mesmo**, com o comportamento de hoje — uma limitação do ambiente não pode impedir o app de funcionar
5. WHEN o sidecar é reiniciado (troca de modelo, mudança de contexto/GPU) THEN o processo novo SHALL entrar no mesmo job, sem vazar handles a cada reinício

**Independent Test**: Com o app aberto e o sidecar de pé, `taskkill /F /IM LocalMind.exe` (ou "Finalizar tarefa" no Gerenciador). Conferir com `tasklist | findstr llama-server` que não sobrou nada.

---

### P2: A saída do sidecar vai para arquivo

**User Story**: Como quem precisa diagnosticar o app, quero ler o que o `llama-server` imprimiu, para investigar lentidão e respostas cortadas como já foi preciso antes.

**Why P2**: Não é o que o usuário pediu, mas é o que impede a correção do P1 de ser uma regressão de diagnóstico. Sem console e sem arquivo, a informação simplesmente deixa de existir.

**Acceptance Criteria**:

1. WHEN o sidecar é iniciado THEN o sistema SHALL redirecionar `stdout` e `stderr` para `<pasta-base>/runtime/llama-server.log`
2. WHEN o sidecar é iniciado THEN o log da execução anterior SHALL ser preservado como `llama-server.log.1`, e o novo arquivo SHALL começar vazio — uma execução, um arquivo, sem crescimento sem fim
3. WHEN o arquivo de log não pode ser aberto (pasta somente-leitura, disco cheio) THEN o sistema SHALL descartar a saída e iniciar o sidecar mesmo assim, em vez de falhar

**Independent Test**: Iniciar o app, abrir `<pasta-base>/runtime/llama-server.log` e ver as linhas de carregamento do modelo. Reiniciar e confirmar que o anterior virou `.log.1`.

---

## Edge Cases

- WHEN o app já está dentro de um Job Object (executado por certos sandboxes/CI) THEN o sistema SHALL depender do aninhamento de jobs (suportado desde o Windows 8) e, se ainda assim a associação falhar, SHALL cair no caminho do AC4 do P1
- WHEN o sidecar morre sozinho enquanto o app roda THEN o job SHALL continuar utilizável para o próximo processo, sem precisar ser recriado
- WHEN o usuário troca a pasta-base com o sidecar rodando THEN o log SHALL passar a ser escrito na pasta nova a partir do próximo reinício do sidecar, não no meio da execução
- WHEN o processo é iniciado com `CREATE_NO_WINDOW` e falha imediatamente THEN a mensagem de erro SHALL vir do health check e do arquivo de log, que passam a ser a **única** fonte — é o que torna o P2 obrigatório e não opcional

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| --- | --- | --- | --- |
| SIDE-01 | P1: Sidecar criado sem janela de console | Implemented | Implementado; **falta a observação visual** — o teste automatizado sai INCONCLUSIVO quando rodado de um terminal, ver a nota de método no tasks.md |
| SIDE-02 | P1: Detecção de GPU sem piscar console | Implemented | **Verificado** — `probe_devices` com a flag devolveu `GpuAvailable("NVIDIA GeForce RTX 3060")` contra o binário real |
| SIDE-03 | P1: Não-Windows inalterado, sem `#[cfg]` no fluxo | Implemented | Implementado — `configure_command` num ponto só, com as duas implementações por `cfg` |
| SIDE-04 | P1: Job Object com kill-on-close | Implemented | **Verificado** — fechar o job encerrou um processo real; e no teste ponta a ponta, o próprio `llama-server` |
| SIDE-05 | P1: Kill forçado do app não deixa órfão | Implemented | **Verificado** — fechar o handle do job (a mesma via de um kill forçado) encerrou o `llama-server` pid 11572 |
| SIDE-06 | P1: Encerramento normal segue como hoje | Implemented | Implementado — `kill`/`Drop` intocados |
| SIDE-07 | P1: Falha ao criar/associar o job degrada, não bloqueia | Implemented | Implementado — falha devolve `None`/`false` com log, o sidecar sobe assim mesmo |
| SIDE-08 | P1: Reinício do sidecar reusa o job, sem vazar handle | Implemented | Implementado — um job por processo, criado no `setup` |
| SIDE-09 | P2: Saída redirecionada para arquivo de log | Implemented | **Verificado** — 1131 bytes do `llama-server` real no arquivo |
| SIDE-10 | P2: Rotação de uma geração a cada início | Implemented | **Verificado** por teste — 3 execuções deixam `.log` e `.log.1`, e só isso |
| SIDE-11 | P2: Log indisponível não impede o sidecar | Implemented | **Verificado** por teste — pasta inutilizável devolve `None` |

**ID format:** `SIDE-[NUMBER]`
**Status values:** Pending → In Design → In Tasks → Implementing → Verified
**Coverage:** 11 total, 0 mapeados para tasks ainda

---

## Success Criteria

- [ ] Abrir o app no Windows mostra **uma** janela na barra de tarefas
- [ ] `taskkill /F /IM LocalMind.exe` seguido de `tasklist | findstr llama-server` não devolve nada
- [ ] Trocar de modelo três vezes seguidas não deixa processo nem handle acumulado
- [ ] O arquivo de log contém as linhas de carregamento do modelo que antes iam para o console
- [ ] `cargo test` continua verde e o app continua subindo no Linux
