import i18n from "i18next";
import { initReactI18next } from "react-i18next";

const resources = {
  en: { translation: { placeholder: "Settings will live here." } },
  ru: { translation: { placeholder: "Здесь будут настройки." } },
};

i18n.use(initReactI18next).init({
  resources,
  lng: navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en",
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
