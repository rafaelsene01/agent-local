// SPEC: self-contained-runtime (SELF-08)

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useRuntimeStore } from "../../store/runtimeStore";
import { runtimeApi } from "../../lib/runtimeApi";
import type { ModelLimits } from "../../types";

type GpuMode = "default" | "off" | "max" | "fraction";

/// Below this a chat barely fits a system prompt, and llama.cpp rounds tiny
/// windows up anyway.
const MIN_CONTEXT = 512;
const CONTEXT_STEP = 512;

/// The window is capped at a share of what the model was trained for rather
/// than at the full figure. The KV cache is allocated for the whole window at
/// start-up, so a machine that can hold `n_ctx_train` exactly has nothing left
/// for the generation itself — the headroom is what keeps a long prompt from
/// turning into an out-of-memory at the worst moment.
const CONTEXT_HEADROOM = 0.8;

/// Default window when the user has not chosen one. Deliberately far below the
/// ceiling: a large window costs VRAM whether or not the conversation uses it.
const PREFERRED_CONTEXT = 40_000;

export function contextCeiling(maxContext: number | null): number | null {
  if (!maxContext) return null;
  const usable = Math.floor((maxContext * CONTEXT_HEADROOM) / CONTEXT_STEP) * CONTEXT_STEP;
  return Math.max(usable, MIN_CONTEXT);
}

interface Props {
  modelName: string;
  onClose: () => void;
}

export function ModelConfigForm({ modelName, onClose }: Props) {
  const { t } = useTranslation();
  const configureModel = useRuntimeStore((s) => s.configureModel);
  const [contextLength, setContextLength] = useState("");
  const [gpuMode, setGpuMode] = useState<GpuMode>("default");
  const [gpuFraction, setGpuFraction] = useState("0.5");
  const [isSaving, setIsSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [limits, setLimits] = useState<ModelLimits | null>(null);

  // The ceiling comes from the model itself — llama.cpp reports `n_ctx_train`
  // (AD-029). A runtime that can't report it leaves the field free instead of
  // getting an invented limit.
  useEffect(() => {
    let active = true;
    runtimeApi
      .modelLimits(modelName)
      .then((result) => active && setLimits(result))
      .catch(() => active && setLimits(null));
    return () => {
      active = false;
    };
  }, [modelName]);

  const maxContext = contextCeiling(limits?.max_context ?? null);
  const sliderValue = Number(contextLength) || limits?.current_context || MIN_CONTEXT;

  // Filled in once the model reports its ceiling, so the field opens on the
  // preferred window instead of empty — but never above what this model can
  // take, which is the whole reason the ceiling is consulted first.
  useEffect(() => {
    if (maxContext === null || contextLength !== "") return;
    setContextLength(String(Math.min(PREFERRED_CONTEXT, maxContext)));
  }, [maxContext, contextLength]);

  function setClamped(value: number) {
    const ceiling = maxContext ?? value;
    setContextLength(String(Math.min(Math.max(value, MIN_CONTEXT), ceiling)));
  }

  function gpuOffloadValue(): string | null {
    if (gpuMode === "off") return "off";
    if (gpuMode === "max") return "max";
    if (gpuMode === "fraction") return gpuFraction;
    return null;
  }

  async function handleSave() {
    setIsSaving(true);
    setError(null);
    setSaved(false);
    try {
      await configureModel(contextLength.trim() ? Number(contextLength) : null, gpuOffloadValue());
      setSaved(true);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="rounded-md border border-[var(--border-color)] px-3 py-3">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-medium">{t("runtime.configureModel", { model: modelName })}</h4>
        <button
          onClick={onClose}
          className="text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
        >
          {t("runtime.close")}
        </button>
      </div>

      <div className="mt-3 space-y-3">
        <div>
          <div className="flex items-baseline justify-between gap-2">
            <label className="text-xs text-[var(--text-secondary)]">
              {t("runtime.contextLength")}
            </label>
            <span className="text-xs text-[var(--text-secondary)]">
              {maxContext
                ? t("runtime.contextMax", { max: maxContext.toLocaleString() })
                : t("runtime.contextMaxUnknown")}
              {limits?.current_context
                ? ` · ${t("runtime.contextCurrent", {
                    current: limits.current_context.toLocaleString(),
                  })}`
                : ""}
            </span>
          </div>

          <div className="mt-1 flex items-center gap-2">
            <input
              type="number"
              min={MIN_CONTEXT}
              max={maxContext ?? undefined}
              step={CONTEXT_STEP}
              value={contextLength}
              onChange={(e) => setContextLength(e.target.value)}
              onBlur={(e) => e.target.value && setClamped(Number(e.target.value))}
              placeholder={t("runtime.contextLengthPlaceholder")}
              className="w-28 rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-2 py-1.5 text-sm"
            />
            {/* The slider only exists when there is a real ceiling to slide
                against; without one it would imply a limit nobody reported. */}
            {maxContext && (
              <input
                type="range"
                min={MIN_CONTEXT}
                max={maxContext}
                step={CONTEXT_STEP}
                value={sliderValue}
                onChange={(e) => setClamped(Number(e.target.value))}
                className="flex-1 accent-[var(--accent)]"
              />
            )}
            {contextLength && (
              <button
                type="button"
                onClick={() => setContextLength("")}
                className="shrink-0 text-xs text-[var(--text-secondary)] underline hover:text-[var(--text-primary)]"
              >
                {t("runtime.useRuntimeDefault")}
              </button>
            )}
          </div>
        </div>

        <div>
          <label className="text-xs text-[var(--text-secondary)]">{t("runtime.gpuOffload")}</label>
          <div className="mt-1 flex flex-wrap items-center gap-2">
            {(["default", "off", "max", "fraction"] as const).map((mode) => (
              <button
                key={mode}
                onClick={() => setGpuMode(mode)}
                className={`rounded-md border px-2 py-1 text-xs ${
                  gpuMode === mode
                    ? "border-[var(--accent)] bg-[var(--accent)] text-[var(--accent-fg)]"
                    : "border-[var(--border-color)] text-[var(--text-secondary)]"
                }`}
              >
                {t(`runtime.gpuMode.${mode}`)}
              </button>
            ))}
            {gpuMode === "fraction" && (
              <input
                type="number"
                min={0}
                max={1}
                step={0.1}
                value={gpuFraction}
                onChange={(e) => setGpuFraction(e.target.value)}
                className="w-20 rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-2 py-1 text-xs"
              />
            )}
          </div>
        </div>

        <button
          onClick={handleSave}
          disabled={isSaving}
          className="rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          {t("runtime.save")}
        </button>

        {error && <p className="text-xs text-red-500">{error}</p>}
        {/* Context and GPU offload are start-up flags, so saving them restarts
            the sidecar — the message says so rather than leaving the user to
            wonder whether the setting took effect (EMBED-12). */}
        {saved && !error && <p className="text-xs text-[var(--text-secondary)]">{t("runtime.saved")}</p>}
      </div>
    </div>
  );
}
