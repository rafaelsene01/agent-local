// SPEC: self-contained-runtime (SELF-01)

import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Cpu } from "lucide-react";
import { useUiStore } from "../../store/uiStore";
import { useRuntimeStore } from "../../store/runtimeStore";

export function RuntimeSection() {
  const { t } = useTranslation();
  const { activeView, setActiveView } = useUiStore();
  const { status, activeModel, loadStatus, loadActiveModel } = useRuntimeStore();
  const isActive = activeView === "runtime";

  useEffect(() => {
    loadStatus();
    loadActiveModel();
  }, [loadStatus, loadActiveModel]);

  // What the sidebar reports is the one thing that decides whether a message
  // can be sent: is a model chosen, and is it actually loaded right now.
  const isRunning = status?.stage === "running";
  const statusKey = !activeModel ? "none" : isRunning ? "running" : "idle";
  const dotColor = !activeModel
    ? "bg-[var(--text-secondary)]"
    : isRunning
      ? "bg-green-500"
      : "bg-amber-500";

  return (
    <div className="border-t border-[var(--border-color)] px-2 py-2">
      <button
        onClick={() => setActiveView("runtime")}
        className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm ${
          isActive
            ? "bg-[var(--bg-elevated)] text-[var(--text-primary)]"
            : "text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)]/60 hover:text-[var(--text-primary)]"
        }`}
      >
        <Cpu size={14} />
        <span className="flex-1 text-left">{t("sidebar.runtime")}</span>
        <span
          className={`h-2 w-2 rounded-full ${dotColor}`}
          title={t(`runtime.status.${statusKey}`, { name: activeModel?.name })}
        />
      </button>
    </div>
  );
}
