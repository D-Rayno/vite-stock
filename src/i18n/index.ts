// src/i18n/index.ts
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";

import fr from "./locales/fr.json";
import ar from "./locales/ar.json";

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      fr: { translation: fr },
      ar: { translation: ar },
    },
    fallbackLng:    "fr",
    supportedLngs:  ["fr", "ar"],
    interpolation:  { escapeValue: false },  // React already escapes
    detection: {
      order:  ["localStorage", "navigator"],
      caches: ["localStorage"],
      lookupLocalStorage: "superpos-lang",
    },
  });

/** Call this after a language change to flip the document's `dir` attribute. */
export function applyDirection(lang: string) {
  document.documentElement.setAttribute("dir",  lang === "ar" ? "rtl" : "ltr");
  document.documentElement.setAttribute("lang", lang);
}

// Apply on first load.
applyDirection(i18n.language);

i18n.on("languageChanged", applyDirection);

export default i18n;