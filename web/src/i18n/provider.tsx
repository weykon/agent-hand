"use client";

import { createContext, useContext } from "react";
import type { Dictionary } from "./index";
import type { Locale } from "@/lib/i18n";

interface I18nContextValue {
  dict: Dictionary;
  lang: Locale;
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({
  dict,
  lang,
  children,
}: {
  dict: Dictionary;
  lang: Locale;
  children: React.ReactNode;
}) {
  return (
    <I18nContext.Provider value={{ dict, lang }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useTranslation() {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useTranslation must be used within I18nProvider");
  return ctx;
}
