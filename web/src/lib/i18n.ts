import { defineI18n } from 'fumadocs-core/i18n';

export const i18n = defineI18n({
  defaultLanguage: 'en',
  languages: ['en', 'zh', 'ja'],
  parser: 'dir',
});

export type Locale = 'en' | 'zh' | 'ja';

export const languages = ['en', 'zh', 'ja'] as const;

export const localeNames: Record<Locale, string> = {
  en: 'English',
  zh: '中文',
  ja: '日本語',
};
