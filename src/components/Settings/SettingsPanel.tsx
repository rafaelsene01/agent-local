import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, FolderOpen } from "lucide-react";
import { useConfigStore } from "../../store/configStore";
import { useUiStore } from "../../store/uiStore";
import { configApi } from "../../lib/configApi";
import { SUPPORTED_THEMES, type Theme } from "../../lib/theme";
import { SUPPORTED_LANGUAGES, type SupportedLanguage } from "../../i18n";

const THEME_LABEL_KEYS: Record<Theme, string> = {
  dark: "settings.themeDark",
  light: "settings.themeLight",
  ocean: "settings.themeOcean",
  claude: "settings.themeClaude",
};

const LANGUAGE_LABEL_KEYS: Record<SupportedLanguage, string> = {
  en: "settings.languageEnglish",
  pt: "settings.languagePortuguese",
};

export function SettingsPanel() {
  const { t } = useTranslation();
  const { config, setTheme, setLanguage, setBasePath } = useConfigStore();
  const setActiveView = useUiStore((s) => s.setActiveView);
  const [isChangingFolder, setIsChangingFolder] = useState(false);

  if (!config) return null;

  async function handleChangeFolder() {
    const picked = await configApi.pickFolder();
    if (!picked || picked === config?.base_path) return;
    setIsChangingFolder(true);
    try {
      await setBasePath(picked);
    } finally {
      setIsChangingFolder(false);
    }
  }

  return (
    <div className="flex flex-1 flex-col overflow-y-auto bg-[var(--bg-app)] text-[var(--text-primary)]">
      <div className="flex items-center gap-3 border-b border-[var(--border-color)] px-6 py-4">
        <button
          onClick={() => setActiveView("chat")}
          className="rounded-md p-1.5 text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-primary)]"
          title={t("settings.back")}
        >
          <ArrowLeft size={18} />
        </button>
        <h1 className="text-base font-semibold">{t("settings.title")}</h1>
      </div>

      <div className="mx-auto w-full max-w-lg space-y-8 px-6 py-8">
        <section>
          <h2 className="text-sm font-medium">{t("settings.theme")}</h2>
          <div className="mt-2 grid grid-cols-2 gap-2 sm:grid-cols-4">
            {SUPPORTED_THEMES.map((themeOption) => (
              <button
                key={themeOption}
                onClick={() => setTheme(themeOption)}
                className={`rounded-md border px-3 py-2 text-sm ${
                  config.theme === themeOption
                    ? "border-[var(--accent)] bg-[var(--accent)] text-[var(--accent-fg)]"
                    : "border-[var(--border-color)] hover:bg-[var(--bg-elevated)]"
                }`}
              >
                {t(THEME_LABEL_KEYS[themeOption])}
              </button>
            ))}
          </div>
        </section>

        <section>
          <h2 className="text-sm font-medium">{t("settings.language")}</h2>
          <div className="mt-2 grid grid-cols-2 gap-2 sm:w-64">
            {SUPPORTED_LANGUAGES.map((langOption) => (
              <button
                key={langOption}
                onClick={() => setLanguage(langOption)}
                className={`rounded-md border px-3 py-2 text-sm ${
                  config.language === langOption
                    ? "border-[var(--accent)] bg-[var(--accent)] text-[var(--accent-fg)]"
                    : "border-[var(--border-color)] hover:bg-[var(--bg-elevated)]"
                }`}
              >
                {t(LANGUAGE_LABEL_KEYS[langOption])}
              </button>
            ))}
          </div>
        </section>

        <section>
          <h2 className="text-sm font-medium">{t("settings.storageFolder")}</h2>
          <p className="mt-2 truncate rounded-md border border-[var(--border-color)] bg-[var(--bg-elevated)] px-3 py-2 text-sm text-[var(--text-secondary)]" title={config.base_path}>
            {config.base_path}
          </p>
          <button
            onClick={handleChangeFolder}
            disabled={isChangingFolder}
            className="mt-2 flex items-center gap-1.5 rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-[var(--accent-fg)] hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            <FolderOpen size={14} />
            {t("settings.changeFolder")}
          </button>
        </section>
      </div>
    </div>
  );
}
