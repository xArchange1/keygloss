import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en";
import ru from "./locales/ru";
import type { UiLanguage } from "./lib/ipc";

const systemLanguage = () => (navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en");

i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, ru: { translation: ru } },
  lng: systemLanguage(),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

/** Applies the `language` setting: auto follows the OS locale (ru -> Russian, else English). */
export function applyLanguage(lang: UiLanguage) {
  const lng = lang === "auto" ? systemLanguage() : lang;
  if (i18n.language !== lng) i18n.changeLanguage(lng);
  document.documentElement.lang = lng;
}

export default i18n;
