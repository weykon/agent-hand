import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import type { ReactNode } from 'react';
import { source } from '@/lib/source';
import { languages } from '@/lib/i18n';
import type { Locale } from '@/lib/i18n';

export default async function Layout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ lang: string }>;
}) {
  const { lang } = await params;
  const locale = (languages.includes(lang as Locale) ? lang : 'en') as Locale;

  return (
    <DocsLayout
      tree={source.getPageTree(locale)}
      nav={{ title: 'Agent Hand Docs' }}
    >
      {children}
    </DocsLayout>
  );
}
