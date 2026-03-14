import { languages } from "@/lib/i18n";
import type { Locale } from "@/lib/i18n";

/**
 * Detect user's preferred locale from localStorage or browser language.
 * Falls back to English.
 */
export function detectLocale(): Locale {
  // 1. Check localStorage preference
  try {
    const saved = localStorage.getItem("agent_hand_locale");
    if (saved && (languages as readonly string[]).includes(saved)) {
      return saved as Locale;
    }
  } catch {}

  // 2. Check browser language
  if (typeof navigator !== "undefined") {
    const browserLang = navigator.language?.toLowerCase() ?? "";
    if (browserLang.startsWith("zh")) return "zh";
    if (browserLang.startsWith("ja")) return "ja";
  }

  // 3. Default to English
  return "en";
}
