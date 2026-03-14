import type { Locale } from '@/lib/i18n';
import en from './en.json';
import zh from './zh.json';
import ja from './ja.json';

export type Dictionary = typeof en;

const dictionaries: Record<Locale, Dictionary> = { en, zh, ja };

export function getDictionary(locale: Locale): Dictionary {
  return dictionaries[locale] ?? dictionaries.en;
}
