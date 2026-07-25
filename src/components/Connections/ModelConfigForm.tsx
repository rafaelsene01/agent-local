import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useConnectionsStore } from "../../store/connectionsStore";
import type { ConfigApplied } from "../../types";

type GpuMode = "default" | "off" | "max" | "fraction";

interface Props {
  connectionId: string;
  modelName: string;
  onClose: () => void;
}

export function ModelConfigForm({ connectionId, modelName, onClose }: Props) {
  const { t } = useTranslation();
  const configureModel = useConnectionsStore((s) => s.configureModel);
  const [contextLength, setContextLength] = useState("");
  const [gpuMode, setGpuMode] = useState<GpuMode>("default");
  const [gpuFraction, setGpuFraction] = useState("0.5");
  const [isSaving, setIsSaving] = useState(false);
  const [applied, setApplied] = useState<ConfigApplied | null>(null);
  const [error, setError] = useState<string | null>(null);

  function gpuOffloadValue(): string | null {
    if (gpuMode === "off") return "off";
    if (gpuMode === "max") return "max";
    if (gpuMode === "fraction") return gpuFraction;
    return null;
  }

  async function handleSave() {
    setIsSaving(true);
    setError(null);
    try {
      const result = await configureModel(
        connectionId,
        modelName,
        contextLength.trim() ? Number(contextLength) : null,
        gpuOffloadValue(),
      );
      setApplied(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="rounded-md border border-[var(--border-color)] px-3 py-3">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-medium">{t("connections.configureModel", { model: modelName })}</h4>
        <button
          onClick={onClose}
          className="text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
        >
          {t("connections.close")}
        </button>
      </div>

      <div className="mt-3 space-y-3">
        <div>
          <label className="text-xs text-[var(--text-secondary)]">{t("connections.contextLength")}</label>
          <input
            type="number"
            min={1}
            value={contextLength}
            onChange={(e) => setContextLength(e.target.value)}
            placeholder={t("connections.contextLengthPlaceholder")}
            className="mt-1 w-full rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-2 py-1.5 text-sm"
          />
        </div>

        <div>
          <label className="text-xs text-[var(--text-secondary)]">{t("connections.gpuOffload")}</label>
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
                {t(`connections.gpuMode.${mode}`)}
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
          {t("connections.save")}
        </button>

        {error && <p className="text-xs text-red-500">{error}</p>}

        {applied && (
          <div className="rounded-md bg-[var(--bg-elevated)] px-3 py-2 text-xs text-[var(--text-secondary)]">
            <p>
              {t("connections.contextLength")}: {applied.context_length_applied ?? t("connections.notApplied")}
            </p>
            <p>
              {t("connections.gpuOffload")}: {applied.gpu_offload_applied ?? t("connections.notApplied")}
            </p>
            {applied.requires_reload && <p>{t("connections.requiresReload")}</p>}
            {applied.note && <p className="mt-1 italic">{applied.note}</p>}
          </div>
        )}
      </div>
    </div>
  );
}
