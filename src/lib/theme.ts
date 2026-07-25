export const SUPPORTED_THEMES = ["dark", "light", "ocean", "claude"] as const;
export type Theme = (typeof SUPPORTED_THEMES)[number];
export const DEFAULT_THEME: Theme = "dark";

export function cachedTheme(): Theme {
  const cached = localStorage.getItem("localmind-theme");
  return SUPPORTED_THEMES.includes(cached as Theme) ? (cached as Theme) : DEFAULT_THEME;
}

export function applyTheme(theme: string) {
  document.documentElement.setAttribute("data-theme", theme);
  localStorage.setItem("localmind-theme", theme);
}
