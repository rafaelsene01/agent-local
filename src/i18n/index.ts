import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json";
import pt from "./locales/pt.json";

export const SUPPORTED_LANGUAGES = ["en", "pt"] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];
export const DEFAULT_LANGUAGE: SupportedLanguage = "en";

const cachedLanguage = localStorage.getItem("localmind-language");
const initialLanguage = SUPPORTED_LANGUAGES.includes(cachedLanguage as SupportedLanguage)
  ? (cachedLanguage as SupportedLanguage)
  : DEFAULT_LANGUAGE;

i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    pt: { translation: pt },
  },
  lng: initialLanguage,
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: { escapeValue: false },
});

export function applyLanguage(language: string) {
  i18n.changeLanguage(language);
  localStorage.setItem("localmind-language", language);
}

export default i18n;
