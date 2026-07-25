# Embedded Runtime — Context (User Decisions)

Captured 2026-07-25 via direct questions (M7 pulled forward ahead of documents-rag/chat-messaging at user's request — "manda bala" on the embedded-runtime option).

## Decision: Activation model

**Chosen:** Always available as an extra connection, same as Ollama/LM Studio — user enables/disables it manually in Conexões, not an automatic hidden fallback.
**Reason:** Reuses the existing `ConnectionManager`/`ConnectionsList` pattern from M3 as-is (detect → list → toggle → status). An automatic "only when nothing else is available" fallback would need new activation logic with no clear extra value for a single-user local app.
**Impact:** The embedded runtime becomes a fourth `provider` value (`"llamacpp"` or similar) alongside `ollama`/`lmstudio`/`custom`, going through the same `ProviderClient` trait — not a special-cased code path.

## Decision: Platform scope this milestone

**Chosen:** Windows + Linux both, now (not Windows-only first).
**Reason:** User's explicit choice, even though only Windows can be exercised/verified in this dev environment. Matches M8's existing multi-OS target.
**Trade-off:** Linux packaging will only be build-verified (binary resolution logic, correct asset URLs), never run-verified in this session — flagged clearly wherever it applies, not silently assumed working.

## Decision: GPU acceleration scope this milestone

**Chosen:** CPU + GPU from the start (not CPU-only v1).
**Reason:** User's explicit choice.
**Trade-off:** Research (Design phase) needs to settle CUDA vs. Vulkan vs. both — CUDA is NVIDIA-only and versioned (12.x/13.x separate binaries), Vulkan is one binary that works across NVIDIA/AMD/Intel without a vendor SDK. This is called out as an open question for Design, not decided here.

## Decision: Default bundled model

**Chosen:** Phi-3.5 Mini Instruct (MIT license, ~3.8B params, 128k context, GGUF Q4_K_M ≈ 2.39 GB).
**Reason:** User picked it over Qwen2.5-1.5B-Instruct (Apache 2.0, ~1.1 GB, better PT-BR) and SmolLM2-1.7B-Instruct (Apache 2.0, ~1.06 GB, English-focused) — explicitly preferring the stronger/larger model and MIT's zero-restriction license over a smaller download.
**Trade-off:** ~2.4 GB first-run download instead of ~1.1 GB; accepted by the user.
**Source (verified via web search, not fabricated):** [bartowski/Phi-3.5-mini-instruct-GGUF](https://huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF) — exact download URL/filename to be pinned during Design.
