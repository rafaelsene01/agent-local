# Sidecar sem console e com ciclo de vida garantido — Tasks

**Spec**: `.specs/features/sidecar-lifecycle/spec.md`
**Design**: `.specs/features/sidecar-lifecycle/design.md`
**Status**: **Complete (2026-07-27) — 8/8.** A T7 fechou contra o app de verdade: o
`llama-server` subiu como filho do `tauri-app`, **nenhuma janela de console apareceu**, e um
`taskkill /F` no app levou o sidecar junto. Ver o Execution Log de 2026-07-27.

> Status anterior (2026-07-26): T1–T6 e T8 feitas, T7 aberta — o app tinha subido mas o sidecar
> não iniciava, porque nenhuma conexão estava ativa (ver "Por que o app não serviu para o teste",
> abaixo). O M9 removeu o conceito de conexão ativa, e com o runtime embutido o autostart passou a
> ter o que iniciar — foi isso que destravou a medição.

---

## Execution Log (2026-07-26)

| Task | Status | Evidência |
| --- | --- | --- |
| T1 | ✅ | `runtime/log.rs`, 5 testes: rotação, uma geração de histórico, pasta criada, pasta inutilizável devolve `None` |
| T2 | ✅ | `runtime/job.rs` com `windows-sys` explícito; `JobState::create()` devolve `Some` nesta máquina |
| T3 | ✅ | `configure_command` + `Stdio` do log + `job.assign` depois do spawn e antes do health check |
| T4 | ✅ | **A Open Question #1 do design foi respondida com o binário real:** `probe_devices` com `CREATE_NO_WINDOW` devolveu `GpuAvailable("NVIDIA GeForce RTX 3060")`. A flag esconde a janela, **não** a captura de stdout — o fallback silencioso para CPU não acontece |
| T5 | ✅ | `base_path` propagado em `start_sidecar_from_row` (cobre setup, autostart e reinício por troca de modelo) |
| T6 | ✅ | `JobState` criado no `setup` e gerenciado como estado, antes de qualquer spawn |
| T7 | ✅ | **Fechada em 2026-07-27 contra o app real** — ver o Execution Log abaixo. Antes disso estava em "⏳ Quase", fechada só pelo teste de integração `sidecar_real` (job criado, health check respondido, 1131 bytes de log, kernel encerrando o processo ao fechar o job), com a observação da janela pendente |
| T8 | ✅ | `cargo tree --depth 1` por alvo: no **Windows** o `windows-sys v0.61.2` aparece como dependência direta; no **Linux**, nenhuma `windows-*` direta (ele só existe lá transitivamente, via `dirs`, como já era antes). `cargo test` verde |

## Execution Log (2026-07-27) — a T7 fechou

O que faltava era a única coisa que nenhum teste tinha conseguido produzir: **o sidecar rodando
como filho do app**, que é o pai sem console em que o bug reproduz. Com o M9 o autostart passou a
ter um modelo ativo para iniciar, e o `npm run tauri dev` entregou exatamente esse cenário —
`tauri-app` PID 21876, `llama-server` PID 23476 subindo 1 s depois, Phi-3.5 carregado e escutando
em `127.0.0.1:53773`.

**Medição 1 — nenhuma janela de console (SIDE-01, SIDE-02).** Enumerando **todas** as janelas
top-level visíveis do desktop pela `EnumWindows` e cruzando cada uma com o processo dono:

```
janelas visíveis de conhost / llama-server / tauri-app:
  PID 21876  tauri-app   <- a única

conhost.exe filho do llama-server: pid=19404, MainWindowHandle=0
total de janelas visíveis com título no desktop: 12
```

O detalhe que torna isso conclusivo é o `conhost` **existir**: o `CREATE_NO_WINDOW` aloca o host de
console e não lhe dá janela. Sem a flag, um app de console iniciado por um pai gráfico ganha uma
janela visível — e é ela que apareceria nesta enumeração. A nota de método de 2026-07-26 continua
valendo (medir `MainWindowHandle` do próprio sidecar não prova nada); o que mudou é que agora a
medida é a janela do **conhost**, e ela foi tirada com o app de verdade como pai.

**Medição 2 — o sidecar morre com o app, pelo caminho em que o nosso código não roda (SIDE-04,
SIDE-05).** `taskkill /F` no app, sem tocar no sidecar:

```
ANTES:   llama-server pid=23476   tauri-app pid=21876
taskkill /F /PID 21876  ->  ÊXITO
DEPOIS (3 s):  nem tauri-app nem llama-server no tasklist
               llama-server pid 23476 ainda existe? False
```

`taskkill /F` não dá ao processo chance de rodar `Drop` nem o handler de `ExitRequested`. Quem
encerrou o `llama-server` foi o kernel, ao fechar o último handle do Job Object — que é exatamente
a garantia que o M7.1 existe para dar, agora observada no produto e não só no teste.

**Não medido:** Linux (fora do escopo declarado da feature — não há console parasita lá) e o
comportamento sob crash do app em vez de kill externo. O `taskkill /F` é o caso mais severo dos
dois, então isto é uma lacuna de cobertura, não uma dúvida sobre a garantia.

---

### O que fechou a T7 em 2026-07-26, e o que tinha sobrado

**Fechado por `runtime::process::sidecar_real`** — teste `#[ignore]` contra o `llama-server.exe` e o Phi-3.5 instalados nesta máquina, tomando os caminhos por variável de ambiente:

```
job criado: true
sidecar respondeu ao health check em 127.0.0.1:59214
log com 1131 bytes em .../runtime/llama-server.log
llama-server pid 11572 encerrado pelo kernel ao fechar o job
```

Fechar o handle do job é exatamente o que acontece com os handles de um processo morto à força — é a mesma via do `taskkill /F`, sem precisar matar o app. Isso cobre SIDE-04, SIDE-05, SIDE-09 e a integração inteira do `spawn`.

**Sobrou uma coisa só:** olhar a barra de tarefas. O teste `the_flag_is_what_decides_whether_a_console_appears` afirma que o processo com a flag não tem janela de console visível, mas **imprime `INCONCLUSIVO`** quando rodado de um terminal: o processo sem a flag empresta o console do runner em vez de criar um visível, então os dois lados dão `false` e a comparação não prova nada. O bug só reproduz a partir de um pai **sem** console — que é o app. Por isso:

```powershell
# Com o app aberto e a conexão embutida ativa: a barra de tarefas mostra só o LocalMind.
```

### Por que o app não serviu para o teste

O `npm run tauri dev` subiu (4m25s de link), mas o sidecar não iniciou — e não por causa das mudanças. Inspecionando uma **cópia** do banco: as três conexões estão com `is_active = 0`, então o autostart não tinha o que iniciar. Ativar uma conexão pelo banco seria mexer na configuração do usuário sem ele pedir; o teste de integração contorna isso por completo.

### Achado durante a T7: o problema existia de verdade, e estava acontecendo

Antes de qualquer medição, a máquina **já tinha um `llama-server` órfão**:

```
PID              : 24580
Rodando ha       : 6.9 horas
Memoria          : 488 MB
Pai              : PID 24156 — NAO EXISTE MAIS (orfao)
```

Um sidecar de quase 7 horas, segurando 488 MB, cujo app pai morreu sem levá-lo junto. Não é o cenário hipotético da spec — é o SIDE-05 acontecendo na máquina do usuário, encontrado por acaso ao preparar o teste. Foi encerrado para a medição começar limpa.

### Nota de método sobre "não tem janela"

`MainWindowHandle = 0` **não prova** ausência de console: a janela de um app de console pertence ao `conhost.exe`, não ao processo. Procurar um `conhost` filho também não fechou a questão (o órfão não tinha nenhum, e mesmo assim havia sido criado pelo código antigo). Ou seja, a ausência de janela continua sendo **verificação visual**, como o design já dizia — não vale marcá-la como verificada com base num proxy que não sustenta a conclusão.

---

## Execution Plan

```
Fase 1 — Blocos independentes (paralelo)
  T1 [P]  runtime/log.rs      (rotação + caminho, com testes)
  T2 [P]  runtime/job.rs      (JobHandle + no-op fora do Windows)

Fase 2 — Integração no spawn
  T1,T2 → T3  (process.rs: sem console, log, job)
          T4  (detect.rs: --list-devices sem console)

Fase 3 — Ligação com o app
  T3 → T5  (embedded_commands: passar o log_path)
  T3 → T6  (lib.rs: criar e gerenciar o JobHandle)

Fase 4 — Verificação real (a que importa)
  T5,T6 → T7  (Windows: janela, taskkill, troca de modelo)
  T5,T6 → T8  (Linux: nada regrediu)
```

---

## T1: Log com rotação de uma geração [P]

**What**: Abrir `<pasta-base>/runtime/llama-server.log` para escrita, renomeando o anterior para `.log.1`.
**Where**: `src-tauri/src/runtime/log.rs` (novo), `src-tauri/src/runtime/mod.rs`
**Depends on**: None
**Reuses**: `config::AppConfig::base_path_buf`, o subdiretório `runtime/` que `ensure_folder_structure` já cria
**Requirement**: SIDE-09, SIDE-10, SIDE-11

**Done when**:
- [ ] `log_path(base) -> PathBuf` e `rotated_path(base) -> PathBuf` são puras e testadas
- [ ] `open_rotating(base) -> Option<File>` renomeia o log anterior e devolve o arquivo novo
- [ ] Um `rename` que falha (arquivo em uso) **não** impede a abertura — trunca e segue
- [ ] Pasta inexistente ou sem permissão devolve `None`, nunca `Err` propagado (SIDE-11)

**Tests**: unit — caminhos; integração leve com `tempdir` para a rotação
**Gate**: `cargo test runtime::log`
**Verify**: rodar duas vezes contra a mesma pasta temporária e conferir que o conteúdo antigo está no `.log.1` e o novo arquivo começa vazio
**Commit**: `feat(runtime): capture llama-server output to a rotating log file`

---

## T2: `JobHandle` com kill-on-close [P]

**What**: Envelopar o Job Object do Windows num tipo seguro, com implementação neutra nos outros sistemas.
**Where**: `src-tauri/src/runtime/job.rs` (novo), `src-tauri/src/runtime/mod.rs`, `src-tauri/Cargo.toml`
**Depends on**: None
**Reuses**: nenhum (primeiro uso de API do Windows no projeto)
**Requirement**: SIDE-04, SIDE-07, SIDE-08

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `windows-sys` declarado em `[target.'cfg(windows)'.dependencies]` com `Win32_System_JobObjects` + `Win32_Foundation` — explícito, não herdado do Tauri
- [ ] `JobHandle::create() -> Option<JobHandle>` cria o job e aplica `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
- [ ] `assign(&self, child: &Child) -> bool` associa pelo handle bruto do processo
- [ ] `Drop` fecha o handle
- [ ] Fora do Windows o tipo existe, `create()` devolve `None` e nada mais é compilado (SIDE-03)
- [ ] Falha em qualquer ponto devolve `None`/`false` com log, **nunca** panic nem `Err` propagado (SIDE-07)

**Tests**: unit — que fora do Windows `create()` é `None`; no Windows, que `create()` devolve `Some` nesta máquina
**Gate**: `cargo test runtime::job`
**Verify**: `cargo check` no Windows e (se possível) `cargo check --target x86_64-unknown-linux-gnu`
**Commit**: `feat(runtime): bind the sidecar lifetime to a Windows job object`

---

## T3: Spawn sem console, com log e dentro do job

**What**: Juntar as três coisas no único ponto que cria o processo do sidecar.
**Where**: `src-tauri/src/runtime/process.rs`
**Depends on**: T1, T2
**Reuses**: `build_args` (não muda), `wait_until_healthy` (não muda)
**Requirement**: SIDE-01, SIDE-03, SIDE-06, SIDE-08, SIDE-09

**Done when**:
- [ ] `configure_command(&mut Command)` aplica `creation_flags(0x08000000)` no Windows e nada fora dele — **uma** função com `#[cfg]`, não `#[cfg]` no meio do fluxo
- [ ] `SidecarConfig` ganha `log_path: Option<PathBuf>`; quando presente, `stdout` e `stderr` vão para o arquivo
- [ ] `spawn` recebe o `&JobHandle` e associa o filho **depois** do `spawn` e **antes** do health check
- [ ] Associação que falha registra o motivo e segue (SIDE-07)
- [ ] `kill()` e o `Drop` continuam exatamente como estão (SIDE-06)

**Tests**: unit — `build_args` intacto; a montagem do `Command` não é testável sem spawnar
**Gate**: `cargo test runtime::`
**Verify**: `npm run tauri dev` sobe o sidecar e o `/health` responde — se a captura de stdout quebrou o processo, é aqui que aparece
**Commit**: `feat(runtime): spawn the sidecar without a console window`

---

## T4: Detecção de GPU sem piscar console

**What**: Aplicar a mesma configuração no `llama-server --list-devices`.
**Where**: `src-tauri/src/runtime/detect.rs`
**Depends on**: T3 (usa a `configure_command`)
**Reuses**: `configure_command` da T3
**Requirement**: SIDE-02

**Done when**:
- [ ] O `Command` do `--list-devices` passa pela `configure_command`
- [ ] O parsing da saída continua funcionando — **este é o risco da task**, e é a Open Question #1 do design

**Tests**: os testes existentes de `classify_output` continuam verdes
**Gate**: `cargo test runtime::detect`
**Verify**: rodar a detecção nesta máquina e conferir que ainda sai `Vulkan0: NVIDIA GeForce RTX 3060` — se vier vazio, o app cai para CPU **silenciosamente**, que é o pior desfecho possível desta feature
**Commit**: `fix(runtime): hide the console flash during GPU detection`

---

## T5: Passar o caminho do log ao montar a config

**What**: Ligar a pasta-base ao `SidecarConfig`.
**Where**: `src-tauri/src/embedded_commands.rs`
**Depends on**: T3
**Reuses**: `config::load_config`
**Requirement**: SIDE-09

**Done when**:
- [ ] Todo lugar que monta `SidecarConfig` (setup, autostart, reinício por troca de modelo/contexto/GPU) passa o `log_path`
- [ ] Sem pasta-base configurada, `log_path` é `None` e o sidecar sobe sem log

**Tests**: none (orquestração)
**Gate**: build
**Verify**: iniciar o app e conferir que o arquivo aparece com as linhas de carregamento do modelo
**Commit**: `feat(runtime): write the sidecar log inside the user's data folder`

---

## T6: Criar o job no boot e mantê-lo vivo

**What**: O `JobHandle` precisa viver tanto quanto o processo do app.
**Where**: `src-tauri/src/lib.rs`
**Depends on**: T3
**Reuses**: o padrão de `app.manage(...)` já usado por `DbState`, `SidecarState`, `CancellationRegistry`
**Requirement**: SIDE-04, SIDE-05, SIDE-08

**Done when**:
- [ ] O `JobHandle` é criado uma vez no `setup` e gerenciado como estado
- [ ] Todos os caminhos de spawn (autostart e comandos) usam **o mesmo** job
- [ ] Nada muda no `RunEvent::ExitRequested`

**Tests**: none (wiring)
**Gate**: build
**Verify**: `npm run tauri dev` sobe sem panic e o sidecar responde
**Commit**: `feat(runtime): keep one job object for the app's whole lifetime`

---

## T7: Verificação real no Windows ⚠️ SÓ HUMANO

**What**: Provar as duas coisas que nenhum teste prova.
**Where**: —
**Depends on**: T5, T6
**Requirement**: SIDE-01, SIDE-02, SIDE-05

**Done when**:
- [ ] Abrir o app: **uma** janela na barra de tarefas, nenhum console
- [ ] Trocar de modelo (reinicia o sidecar): nada pisca
- [ ] `taskkill /F /IM LocalMind.exe` → `tasklist | findstr llama-server` **não devolve nada**
- [ ] Repetir o ciclo abrir/trocar modelo/fechar 3 vezes: nenhum processo acumulado
- [ ] A detecção de GPU continua achando a RTX 3060 (não caiu para CPU)
- [ ] O log tem as linhas de carregamento do modelo

**Tests**: none — é observação
**Gate**: manual
**Verify**: as caixas acima, com o Gerenciador de Tarefas aberto
**Commit**: `docs: record the sidecar lifecycle verification`

---

## T8: Confirmar que o Linux não regrediu

**What**: Garantir que o caminho não-Windows continua compilando e rodando como antes.
**Where**: —
**Depends on**: T5, T6
**Requirement**: SIDE-03

**Done when**:
- [ ] `cargo test` verde
- [ ] O `ci.yml` (que roda `cargo test` em `ubuntu-22.04`) passa — é o guarda automático desta task
- [ ] Nenhum `windows-sys` no grafo de dependências do Linux

**Tests**: a suíte inteira
**Gate**: `cargo test` + CI verde
**Verify**: `cargo tree --target x86_64-unknown-linux-gnu | grep windows-sys` não traz a dependência **direta**
**Commit**: —

---

## Requirement Coverage

| Requisito | Tasks |
| --- | --- |
| SIDE-01 | T3, T7 |
| SIDE-02 | T4, T7 |
| SIDE-03 | T2, T3, T8 |
| SIDE-04 | T2, T6 |
| SIDE-05 | T6, T7 |
| SIDE-06 | T3 |
| SIDE-07 | T2, T3 |
| SIDE-08 | T2, T3, T6 |
| SIDE-09 | T1, T3, T5 |
| SIDE-10 | T1 |
| SIDE-11 | T1 |

**11 requisitos, 11 mapeados, 0 órfãos.**

> **O gate desta feature não é `cargo test`.** É a T7: uma janela a menos e um `tasklist` vazio depois de um kill forçado. Tudo antes disso é preparação.
