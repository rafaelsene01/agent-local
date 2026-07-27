# Embedded Runtime (llama.cpp) — Specification

**Context**: `.specs/features/embedded-runtime/context.md` (user decisions on activation, platform scope, GPU scope, default model)

> ## ⚠️ PARCIALMENTE REVOGADA por `self-contained-runtime` (M9) — 2026-07-27
>
> **Decisões: AD-039 (planejamento), AD-042 e AD-043 (execução).**
>
> Esta feature entregou o sidecar llama.cpp como **um fallback entre outros**:
> uma "conexão embutida" que o usuário habilitava numa tela de Conexões, e que
> baixava o próprio binário na primeira vez. O M9 promoveu esse sidecar a
> **único runtime** e mudou de onde ele vem.
>
> Duas mudanças invertem premissas deste documento:
>
> 1. **O binário não é mais baixado** — viaja dentro do instalador
>    (SELF-09/SELF-10). `runtime/release.rs`, que consultava a API do GitHub,
>    foi apagado.
> 2. **O setup não baixa mais um modelo padrão** — `prepare_runtime` resolve só
>    o motor; escolher o modelo é ação separada (AD-043).
>
> **Mas boa parte desta spec continua valendo, e é o coração do app hoje:** o
> ciclo de vida do sidecar (auto-start, morte no quit), a detecção de GPU com
> fallback para CPU e a reconfiguração por restart são exatamente o que roda.
> Ver o desfecho de cada requisito na rastreabilidade.
>
> **Complementada por** `.specs/features/sidecar-lifecycle/spec.md` (M7.1), que
> reforçou EMBED-07 com Job Object e janela de console oculta.

## Problem Statement

Today the app only talks to Ollama or LM Studio — both external programs the user must install themselves. On a clean machine with neither installed, M3's Conexões screen just shows an empty state; the app cannot chat at all. This feature ships a self-contained fallback: an embedded `llama.cpp` sidecar the app manages entirely on its own (download binary + a default model, run it as a local child process), so the app works out of the box with zero external installation.

## Goals

- [x] User can enable a built-in "embedded" connection that requires no external software
- [x] First enable downloads the right `llama.cpp` sidecar binary for the user's OS (+ GPU backend when available) and a default model, both with visible progress
- [x] The sidecar behaves like any other connection through the existing `ProviderClient` abstraction (M3) — no special-cased UI
- [x] Sidecar process lifecycle is tied to the app (starts when enabled/needed, stops on quit — never an orphaned process)
- [x] GPU acceleration used automatically when the hardware supports it, CPU otherwise, no manual flags

## Out of Scope

| Feature | Reason |
| --- | --- |
| macOS builds | Already a Future Consideration in ROADMAP.md; this milestone is Windows + Linux only (context.md) |
| Auto-updating the llama.cpp binary to newer releases | v1 pins whatever version it resolved on first download; re-checking/updating is P3 at most, not required to ship |
| Running multiple embedded models simultaneously | Matches the existing single "active model" concept from M3 (AD-016) — one embedded model active at a time |
| CUDA-specific packaging | Open technical question for Design (CUDA vs. Vulkan vs. both) — not decided in this spec, see context.md |

---

## Research Findings (Knowledge Verification Chain)

Confirmed via web search (not fabricated):

- **`llama-server`** (the current name for what used to be called `server`) ships an **OpenAI-compatible HTTP API** (`/v1/chat/completions`, `/v1/models`, etc.) — this is what lets it slot into the existing `ProviderClient` trait pattern from M3 exactly like Ollama/LM Studio.
- **Official prebuilt binaries** exist for both target platforms via GitHub Releases (`github.com/ggml-org/llama.cpp/releases`), as versioned per-build assets (tag format like `b10107`, incrementing continuously — there is no stable long-lived version number to hardcode). Confirmed asset naming from the latest release at research time: `llama-<tag>-bin-win-cpu-x64.zip`, `llama-<tag>-bin-win-cuda-12.4-x64.zip`, `llama-<tag>-bin-win-vulkan-x64.zip`, `llama-<tag>-bin-ubuntu-x64.tar.gz`, `llama-<tag>-bin-ubuntu-vulkan-x64.tar.gz`. Exact tag must be resolved at download time via GitHub's "latest release" API, not pinned at design time.
- **Default model** — Phi-3.5-mini-instruct-GGUF, MIT license, ~3.8B params, 128k context, Q4_K_M ≈ 2.39 GB, published by multiple GGUF re-packagers (e.g. `bartowski/Phi-3.5-mini-instruct-GGUF`) — user's explicit choice over two smaller Apache-2.0 alternatives (see context.md).

## Open Questions Carried to Design

- CUDA vs. Vulkan (vs. both) for GPU acceleration — Vulkan is one binary covering NVIDIA/AMD/Intel without a vendor SDK; CUDA is NVIDIA-only and split across major versions (12.x/13.x). Needs a recommendation backed by research into current llama.cpp Vulkan backend maturity/performance before committing.
- Exact mechanism to detect "GPU capable of Vulkan" from Rust without a heavy dependency (sysinfo alone, per M3, does not do GPU detection).
- Sidecar port allocation strategy (fixed default vs. first-free-port scan) and how `llama-server`'s CLI flags map to `context_length`/`gpu_offload` (`ConfigApplied` semantics, same honesty pattern as M3's `ConfigApplied.requires_reload`).
- Exact GitHub Releases API shape for resolving "latest" release + filtering assets by platform/backend (needs confirmation during Design, not assumed here).

---

## User Stories

### P1: Zero-setup chat on a clean machine ⭐ MVP

**User Story**: As a user with neither Ollama nor LM Studio installed, I want the app to offer a built-in option that sets itself up, so I'm not blocked before even trying the app.

**Why P1**: This is the entire point of the feature — without it, "zero setup" isn't true.

**Acceptance Criteria**:

1. WHEN the user opens Conexões THEN the system SHALL always show the embedded runtime as an option, regardless of whether Ollama/LM Studio are detected
2. WHEN the user enables the embedded runtime for the first time THEN the system SHALL download the OS+backend-appropriate `llama.cpp` sidecar binary with visible progress (reusing the M3 download-progress event pattern, CONN-11)
3. WHEN the sidecar binary is ready and no model is installed yet for it THEN the system SHALL offer to download the default model (Phi-3.5 Mini Instruct) with visible progress
4. WHEN both the binary and the default model are ready THEN the system SHALL start the `llama-server` sidecar as a local child process and the connection SHALL report status "available" (reusing M3's `ConnectionStatus`)
5. WHEN the embedded connection is enabled and healthy THEN its model SHALL be selectable as the active model exactly like any Ollama/LM Studio model (CONN-06)

**Independent Test**: On a machine with neither Ollama nor LM Studio running, enable the embedded connection from a fresh state, watch both downloads progress to completion, and select the resulting model as active.

---

### P1: Sidecar lifecycle tied to the app ⭐ MVP

**User Story**: As a user, I want the embedded server to start and stop with the app, so it never lingers as an orphaned background process.

**Why P1**: A silently-orphaned local server is a real resource leak and a trust problem for a "local-first/private" app — unacceptable even for an MVP.

**Acceptance Criteria**:

1. WHEN the app starts AND the embedded runtime is enabled with a model already downloaded THEN the system SHALL launch the sidecar automatically, without the user re-clicking anything
2. WHEN the app quits normally THEN the system SHALL terminate the sidecar child process
3. WHEN the sidecar process fails to start or crashes THEN the connection status SHALL show "unavailable" rather than hanging indefinitely
4. WHEN the user disables the embedded connection THEN the system SHALL stop the running sidecar process

**Independent Test**: Enable the embedded connection, confirm the sidecar process exists (OS process list), quit the app, confirm the process is gone.

---

### P2: GPU acceleration when available

**User Story**: As a user with a capable GPU, I want the embedded runtime to use it automatically, so responses come back faster than pure CPU.

**Why P2**: Meaningfully improves the experience but the feature is usable (if slower) on CPU alone — not required for the MVP to be demoable.

**Acceptance Criteria**:

1. WHEN the app detects a GPU capable of the chosen acceleration backend THEN the system SHALL download/use the GPU-accelerated sidecar variant instead of the CPU-only one
2. WHEN no compatible GPU is detected THEN the system SHALL fall back to the CPU-only sidecar without error or user intervention
3. WHEN the user configures context length / GPU offload for the embedded model (same UI as CONN-12/13) THEN the system SHALL apply it via the sidecar's own startup flags, restarting the sidecar if a live-reload isn't possible, and SHALL report `requires_reload: true` — same honesty pattern M3 already uses for LM Studio

**Independent Test**: On a machine with a Vulkan-capable GPU, confirm the sidecar launches with GPU flags (visible in process args/logs); on a machine without one, confirm it falls back to CPU cleanly.

---

### P2: Download a different GGUF model manually

**User Story**: As a user, I want to point the embedded runtime at a specific GGUF file (Hugging Face link), not just the bundled default, so I can use a different local model.

**Why P2**: Extends the feature past the bundled default but the MVP already works without it.

**Acceptance Criteria**:

1. WHEN the user provides a direct `.gguf` Hugging Face link THEN the system SHALL download it into the embedded runtime's model folder with visible progress, mirroring the manual-pull pattern already built for Ollama/LM Studio (CONN-10)
2. WHEN the download completes THEN the model SHALL appear in the embedded connection's installed-models list and be selectable

**Independent Test**: Paste a direct `.gguf` link for a small model, confirm it downloads and becomes selectable.

---

### P3: See which llama.cpp build is installed

**User Story**: As a user, I want to see which `llama.cpp` release is currently in use, so I have visibility if something ever needs troubleshooting.

**Why P3**: Nice-to-have transparency; no functional impact if missing.

**Acceptance Criteria**:

1. WHEN viewing the embedded connection's details THEN the system SHALL display the installed `llama.cpp` release tag

---

## Edge Cases

- WHEN the binary or model download is interrupted (network/power loss) THEN the system SHALL leave no half-usable state active and SHALL let the user retry cleanly
- WHEN available disk space is insufficient for the binary + default model THEN the system SHALL fail with a clear message before attempting the download, not partway through
- WHEN the app runs on a platform other than Windows or Linux THEN the system SHALL show the embedded option as unavailable rather than attempting a download that's guaranteed to fail
- WHEN the sidecar's default local port is already in use (e.g. by another process) THEN the system SHALL pick an alternate free port automatically instead of failing to start
- WHEN the app is updated to a new version THEN an already-downloaded sidecar binary/model SHALL persist as-is (no forced re-download) unless the user explicitly triggers an update later

---

## Requirement Traceability

> **Atualizada em 2026-07-27.** Os 14 requisitos foram implementados em
> 2026-07-25; a coluna abaixo diz o que restou depois do M9, conferido contra o
> código atual.

| Requirement ID | Story | Desfecho |
| --- | --- | --- |
| EMBED-01 | P1: Embedded option always visible in Conexões | ❌ **Revogado** por SELF-01 (AD-042) — não há tela de Conexões; o runtime é o único e a tela é "Runtime" |
| EMBED-02 | P1: First-enable downloads sidecar binary with progress | ❌ **Revogado** por SELF-09/SELF-10 (AD-039) — o binário vem do instalador; `runtime/release.rs` apagado |
| EMBED-03 | P1: First-enable downloads default model with progress | ❌ **Revogado** por AD-043 — `prepare_runtime` não baixa modelo; contradizia o alvo "instalar sem rede e conversar" |
| EMBED-04 | P1: Sidecar as child process, status via `ProviderClient` | ⚠️ **Meio revogado** — o processo filho e o status continuam (SELF-04); o `ProviderClient` foi substituído pelo `LlamaServerClient` concreto (SELF-03) |
| EMBED-05 | P1: Embedded model selectable as active model (CONN-06 reuse) | ❌ **Revogado por fusão** em SELF-07 — não há "modelo embutido" em oposição a outros |
| EMBED-06 | P1: Sidecar auto-starts with app when model ready | ✅ **Vive** — marcador `SPEC:` em `lib.rs`; a pergunta mudou de "a conexão embutida está ativa?" para "há runtime pronto e modelo ativo?" (AD-043) |
| EMBED-07 | P1: Sidecar terminates on normal app quit | ✅ **Vive e reforçado** — SIDE-04/05/06 acrescentaram o Job Object para o caso de kill forçado (AD-041) |
| EMBED-08 | P1: Crashed sidecar reports "unavailable", not a hang | ✅ **Vive** — `runtime_status`. A AD-046 mostrou o limite: com o binário quebrado a UI reportou indisponível **com a causa vazia**, porque o processo nem chegou a existir |
| EMBED-09 | P1: Disabling the connection stops the sidecar | ⚠️ **Reformulado** — virou `stop_runtime`, ação direta sobre o runtime, sem conexão no meio |
| EMBED-10 | P2: GPU-capable hardware uses accelerated variant | ✅ **Vive** como SELF-11 — `probe_devices`; verificado ao vivo com a RTX 3060 (AD-041, AD-046) |
| EMBED-11 | P2: No GPU falls back to CPU without error | ⚠️ **Vive, nunca exercitado** — o binário CPU embutido sobe (exit 0), mas o *fallback* de um para o outro nunca rodou numa máquina sem loader Vulkan (SELF-11) |
| EMBED-12 | P2: Context/GPU config applies via sidecar restart | ✅ **Vive** como SELF-08 — `configure_model` continua reiniciando o sidecar (AD-042) |
| EMBED-13 | P2: Manual GGUF download via direct Hugging Face link | ✅ **Vive** — `manualUrl` em `ModelsList.tsx`; hoje é o **único** caminho manual, já que o `pull` por nome saiu com o Ollama (CONN-10) |
| EMBED-14 | P3: Display installed llama.cpp release tag | ⚠️ **Vive, reformulado** — `RuntimeStatus.release_tag` continua; a tag agora vem fixa de `scripts/vendor.json` (`b10146`), não de uma consulta à API do GitHub (SELF-14) |

**ID format:** `EMBED-[NUMBER]`
**Coverage:** 14 no total — **4 revogados, 4 reformulados, 6 vivos.** Nenhum
apagado.

**\*** A marca "Verified" de 2026-07-25 vinha de teste automatizado + exercício
real do sidecar (`--list-devices`, `/health`, `/v1/models`,
`/v1/chat/completions`). **Ela não cobria o binário empacotado**, que é coisa
diferente do binário baixado — foi exatamente aí que a AD-046 encontrou um
`llama-server.exe` que não executava.

---

## Success Criteria

- [x] On a clean Windows or Linux machine with neither Ollama nor LM Studio installed, a user can enable the embedded runtime and successfully send/receive a chat message with zero manual terminal/install steps
- [x] The sidecar process never lingers on disk/in the process list after a normal app quit
- [x] GPU acceleration engages automatically on capable hardware with no user-provided flags, and CPU fallback never errors
