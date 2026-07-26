export const SUPPORTED_THEMES = ["dark", "light", "ocean", "terracotta"] as const;
export type Theme = (typeof SUPPORTED_THEMES)[number];
export const DEFAULT_THEME: Theme = "dark";

/** Themes renamed after users could already have them persisted. Dropping the
 *  old id instead of mapping it would silently reset those users to the
 *  default and look like the app forgot their choice. */
const RENAMED_THEMES: Record<string, Theme> = {
  claude: "terracotta",
};

export function normalizeTheme(raw: string | null | undefined): Theme {
  const value = raw ?? "";
  if (SUPPORTED_THEMES.includes(value as Theme)) return value as Theme;
  return RENAMED_THEMES[value] ?? DEFAULT_THEME;
}

export function cachedTheme(): Theme {
  return normalizeTheme(localStorage.getItem("localmind-theme"));
}

export function applyTheme(theme: string) {
  const normalized = normalizeTheme(theme);
  document.documentElement.setAttribute("data-theme", normalized);
  localStorage.setItem("localmind-theme", normalized);
}
