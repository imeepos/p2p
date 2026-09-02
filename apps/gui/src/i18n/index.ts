import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import enUS from "./locales/en-US";
import zhCN from "./locales/zh-CN";
import "./types";

export const SUPPORTED_LOCALES = ["zh-CN", "en-US"] as const;
export type Locale = (typeof SUPPORTED_LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "zh-CN";

const LOCALE_STORAGE_KEY = "p2p-console-locale";

export function readStoredLocale(): Locale {
  try {
    const raw = localStorage.getItem(LOCALE_STORAGE_KEY);
    if (raw && (SUPPORTED_LOCALES as readonly string[]).includes(raw)) {
      return raw as Locale;
    }
  } catch {
    console.warn("[i18n] localStorage 不可读，使用默认语言");
  }
  return DEFAULT_LOCALE;
}

export function changeLocale(locale: Locale): void {
  try {
    localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
    console.warn("[i18n] localStorage 不可写，语言仅本次生效");
  }
  void i18n.changeLanguage(locale);
}

void i18n.use(initReactI18next).init({
  resources: {
    "zh-CN": { translation: zhCN },
    "en-US": { translation: enUS },
  },
  lng: readStoredLocale(),
  fallbackLng: DEFAULT_LOCALE,
  interpolation: { escapeValue: false },
});

export default i18n;
