// SPEC: chat-messaging (CHAT-16)

import { useTranslation } from "react-i18next";

/** The backend budgets the prompt in characters at four per token
 *  (`CHARS_PER_TOKEN` in `chat/context_assembler.rs`). Using the same divisor
 *  here keeps the number on screen and the number that truncates the prompt in
 *  the same unit — a different estimate would drift from what actually fits. */
const CHARS_PER_TOKEN = 4;

/** Fallback ceiling for when no window has been configured. It matches
 *  `DEFAULT_CONTEXT_TOKENS` in `chat/context_assembler.rs`, which is what the
 *  backend budgets against in that case — drawing the gauge against a larger
 *  number would show room the prompt assembler will not actually use. */
export const FALLBACK_CONTEXT_TOKENS = 4096;

export function estimateTokens(texts: string[]): number {
  const chars = texts.reduce((total, text) => total + text.length, 0);
  return Math.ceil(chars / CHARS_PER_TOKEN);
}

const SIZE = 20;
const STROKE = 2.5;
const RADIUS = (SIZE - STROKE) / 2;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

export function ContextGauge({ tokens, ceiling }: { tokens: number; ceiling: number | null }) {
  const { t } = useTranslation();
  const max = ceiling ?? FALLBACK_CONTEXT_TOKENS;
  const ratio = Math.min(tokens / max, 1);
  const format = (n: number) => n.toLocaleString();

  // Amber past three quarters, red when the ceiling is reached: the colour is
  // the part read at a glance, the tooltip is for the exact number.
  const colour =
    ratio >= 1 ? "text-red-500" : ratio >= 0.75 ? "text-amber-500" : "text-[var(--text-secondary)]";

  return (
    <span
      className={`group relative flex shrink-0 items-center ${colour}`}
      title={t("chatPanel.contextUsage", { used: format(tokens), max: format(max) })}
    >
      <svg width={SIZE} height={SIZE} viewBox={`0 0 ${SIZE} ${SIZE}`} aria-hidden="true">
        <circle
          cx={SIZE / 2}
          cy={SIZE / 2}
          r={RADIUS}
          fill="none"
          strokeWidth={STROKE}
          className="stroke-[var(--border-color)]"
        />
        <circle
          cx={SIZE / 2}
          cy={SIZE / 2}
          r={RADIUS}
          fill="none"
          strokeWidth={STROKE}
          stroke="currentColor"
          strokeLinecap="round"
          strokeDasharray={CIRCUMFERENCE}
          strokeDashoffset={CIRCUMFERENCE * (1 - ratio)}
          // Starts at twelve o'clock instead of three, which is where a filling
          // gauge is read from.
          transform={`rotate(-90 ${SIZE / 2} ${SIZE / 2})`}
        />
      </svg>

      {/* `title` alone is enough for the text, but it takes a second to appear
          and cannot be styled; this is the one the user actually sees. */}
      <span
        role="tooltip"
        className="pointer-events-none absolute bottom-full right-0 mb-2 hidden whitespace-nowrap rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-2 py-1 text-xs text-[var(--text-primary)] shadow-md group-hover:block"
      >
        {t("chatPanel.contextUsage", { used: format(tokens), max: format(max) })}
      </span>
    </span>
  );
}
