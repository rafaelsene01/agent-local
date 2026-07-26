import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, FolderOpen, Sparkles } from "lucide-react";
import { configApi } from "../../lib/configApi";
import { applyLanguage } from "../../i18n";
import { applyTheme, SUPPORTED_THEMES, DEFAULT_THEME, type Theme } from "../../lib/theme";
import { SUPPORTED_LANGUAGES, DEFAULT_LANGUAGE, type SupportedLanguage } from "../../i18n";
import { useConfigStore } from "../../store/configStore";

const THEME_LABEL_KEYS: Record<Theme, string> = {
  dark: "settings.themeDark",
  light: "settings.themeLight",
  ocean: "settings.themeOcean",
  terracotta: "settings.themeTerracotta",
};

const LANGUAGE_LABEL_KEYS: Record<SupportedLanguage, string> = {
  en: "settings.languageEnglish",
  pt: "settings.languagePortuguese",
};

export function Wizard() {
  const { t } = useTranslation();
  const completeOnboarding = useConfigStore((s) => s.completeOnboarding);
  const missingBasePath = useConfigStore((s) => s.missingBasePath);
  const savedConfig = useConfigStore((s) => s.config);

  const [basePath, setBasePath] = useState("");
  const [theme, setTheme] = useState<Theme>(DEFAULT_THEME);
  const [language, setLanguage] = useState<SupportedLanguage>(DEFAULT_LANGUAGE);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Recovering from a lost folder: keep pointing at the same path (plugging
    // the drive back in makes one click enough) and keep the choices the user
    // already made, instead of resetting them to the defaults.
    if (missingBasePath) {
      setBasePath(missingBasePath);
      return;
    }
    configApi.getDefaultBasePath().then(setBasePath).catch(() => {});
  }, [missingBasePath]);

  useEffect(() => {
    if (!savedConfig) return;
    if (SUPPORTED_THEMES.includes(savedConfig.theme as Theme)) {
      setTheme(savedConfig.theme as Theme);
    }
    if (SUPPORTED_LANGUAGES.includes(savedConfig.language as SupportedLanguage)) {
      setLanguage(savedConfig.language as SupportedLanguage);
    }
  }, [savedConfig]);

  async function handleChooseFolder() {
    const picked = await configApi.pickFolder();
    if (picked) setBasePath(picked);
  }

  function handleThemeChange(next: Theme) {
    setTheme(next);
    applyTheme(next);
  }

  function handleLanguageChange(next: SupportedLanguage) {
    setLanguage(next);
    applyLanguage(next);
  }

  async function handleFinish() {
    setIsSubmitting(true);
    setError(null);
    try {
      await completeOnboarding(basePath, theme, language);
    } catch (err) {
      setError(String(err));
      setIsSubmitting(false);
    }
  }

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-[var(--bg-app)] p-6 text-[var(--text-primary)]">
      <div className="w-full max-w-md rounded-xl border border-[var(--border-color)] bg-[var(--bg-elevated)] p-6 shadow-xl">
        <div className="flex items-center gap-2">
          <Sparkles size={22} className="text-[var(--accent)]" />
          <h1 className="text-lg font-semibold">
            {missingBasePath ? t("onboarding.storageLostTitle") : t("onboarding.welcomeTitle")}
          </h1>
        </div>
        <p className="mt-1 text-sm text-[var(--text-secondary)]">
          {missingBasePath ? t("onboarding.storageLostSubtitle") : t("onboarding.welcomeSubtitle")}
        </p>

        {missingBasePath && (
          <div className="mt-4 flex items-start gap-2 rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-sm">
            <AlertTriangle size={16} className="mt-0.5 shrink-0 text-amber-500" />
            <p className="min-w-0 break-all">
              {t("onboarding.storageLostWarning", { path: missingBasePath })}
            </p>
          </div>
        )}

        <div className="mt-6 space-y-5">
          <div>
            <label className="text-sm font-medium">{t("onboarding.folderLabel")}</label>
            <div className="mt-2 flex items-center gap-2">
              <input
                readOnly
                value={basePath}
                className="min-w-0 flex-1 truncate rounded-md border border-[var(--border-color)] bg-[var(--bg-app)] px-2.5 py-1.5 text-sm"
              />
              <button
                onClick={handleChooseFolder}
                className="flex shrink-0 items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)]"
              >
                <FolderOpen size={14} />
                {t("onboarding.folderChoose")}
              </button>
            </div>
          </div>

          <div>
            <label className="text-sm font-medium">{t("onboarding.themeLabel")}</label>
            <div className="mt-2 grid grid-cols-2 gap-2">
              {SUPPORTED_THEMES.map((themeOption) => (
                <button
                  key={themeOption}
                  onClick={() => handleThemeChange(themeOption)}
                  className={`rounded-md border px-3 py-1.5 text-sm ${
                    theme === themeOption
                      ? "border-[var(--accent)] bg-[var(--accent)] text-[var(--accent-fg)]"
                      : "border-[var(--border-color)] hover:bg-[var(--bg-app)]"
                  }`}
                >
                  {t(THEME_LABEL_KEYS[themeOption])}
                </button>
              ))}
            </div>
          </div>

          <div>
            <label className="text-sm font-medium">{t("onboarding.languageLabel")}</label>
            <div className="mt-2 flex gap-2">
              {SUPPORTED_LANGUAGES.map((langOption) => (
                <button
                  key={langOption}
                  onClick={() => handleLanguageChange(langOption)}
                  className={`flex-1 rounded-md border px-3 py-1.5 text-sm ${
                    language === langOption
                      ? "border-[var(--accent)] bg-[var(--accent)] text-[var(--accent-fg)]"
                      : "border-[var(--border-color)] hover:bg-[var(--bg-app)]"
                  }`}
                >
                  {t(LANGUAGE_LABEL_KEYS[langOption])}
                </button>
              ))}
            </div>
          </div>
        </div>

        {error && <p className="mt-4 text-sm text-red-400">{error}</p>}

        <button
          onClick={handleFinish}
          disabled={isSubmitting || !basePath}
          className="mt-6 w-full rounded-md bg-[var(--accent)] px-4 py-2 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          {isSubmitting ? t("onboarding.settingUp") : t("onboarding.finish")}
        </button>
      </div>
    </div>
  );
}
