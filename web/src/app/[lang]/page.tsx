import Link from "next/link";
import type { Metadata } from "next";
import { getDictionary } from "@/i18n";
import { languages } from "@/lib/i18n";
import type { Locale } from "@/lib/i18n";
import { LandingClient } from "./landing-client";

const SITE_URL = "https://weykon.github.io/agent-hand";

export async function generateMetadata({
  params,
}: {
  params: Promise<{ lang: string }>;
}): Promise<Metadata> {
  const { lang } = await params;
  const locale = (languages.includes(lang as Locale) ? lang : "en") as Locale;
  const dict = getDictionary(locale);

  const ogLocaleMap: Record<Locale, string> = { en: "en_US", zh: "zh_CN", ja: "ja_JP" };

  return {
    title: "Agent Hand - Terminal Session Manager for AI Coding Agents",
    description: dict.hero.subtitle.replace("<", "<"),
    keywords: [
      "tmux session manager", "AI agent terminal", "Claude Code sessions",
      "AI coding agent manager", "terminal TUI", "Rust developer tools",
      "agent-deck", "tmux multiplexer", "Claude Code", "Gemini CLI",
      "multi-agent terminal", "session manager macOS Linux",
    ],
    alternates: {
      canonical: SITE_URL + `/${locale}/`,
      languages: {
        en: SITE_URL + "/en/",
        zh: SITE_URL + "/zh/",
        ja: SITE_URL + "/ja/",
        "x-default": SITE_URL + "/en/",
      },
    },
    openGraph: {
      title: "Agent Hand — Manage All Your AI Agent Sessions in One TUI",
      description: dict.hero.tagline,
      url: SITE_URL + `/${locale}/`,
      siteName: "Agent Hand",
      images: [
        {
          url: SITE_URL + "/preview.jpg",
          width: 1280,
          height: 720,
          alt: dict.preview.alt,
        },
      ],
      locale: ogLocaleMap[locale],
      type: "website",
    },
    twitter: {
      card: "summary_large_image",
      title: "Agent Hand — Manage All Your AI Agent Sessions in One TUI",
      description: dict.hero.tagline,
      images: [SITE_URL + "/preview.jpg"],
      creator: "@weykon",
    },
  };
}

export default async function HomePage({
  params,
}: {
  params: Promise<{ lang: string }>;
}) {
  const { lang } = await params;
  const locale = (languages.includes(lang as Locale) ? lang : "en") as Locale;
  const dict = getDictionary(locale);

  return (
    <>
      {/* Hero */}
      <section className="px-6 py-20 text-center">
        <h1 className="mb-4 text-5xl font-bold tracking-tight">{dict.hero.title}</h1>
        <p className="mx-auto mb-2 max-w-2xl text-lg text-[#94a3b8]">
          {dict.hero.subtitle}
        </p>
        <p className="mx-auto mb-8 max-w-xl text-sm text-[#64748b]">
          {dict.hero.tagline}
        </p>
        <LandingClient section="heroInstall" />
      </section>

      {/* Terminal Preview */}
      <section className="mx-auto max-w-2xl px-6 pb-16">
        <div className="overflow-hidden rounded-xl border border-[#333] bg-[#1a1a2e]">
          <div className="flex items-center gap-2 border-b border-[#333] px-4 py-2">
            <span className="h-3 w-3 rounded-full bg-red-500" />
            <span className="h-3 w-3 rounded-full bg-yellow-500" />
            <span className="h-3 w-3 rounded-full bg-green-500" />
            <span className="ml-2 text-xs text-[#64748b]">{dict.terminal.title}</span>
          </div>
          <div className="space-y-2 p-4 font-mono text-sm">
            {dict.terminal.sessions.map((s: { icon: string; color: string; name: string; group: string; status: string }) => (
              <div key={s.name} className="flex items-center gap-3">
                <span className={s.color}>{s.icon}</span>
                <span className="text-[#e2e8f0]">{s.name}</span>
                <span className="text-[#64748b]">{s.group}</span>
                <span className="ml-auto text-[#94a3b8]">{s.status}</span>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Features — Free */}
      <section id="features" className="bg-[#0f0f1a] px-6 py-20">
        <h2 className="mb-4 text-center text-3xl font-bold">{dict.features.title}</h2>
        <p className="mb-12 text-center text-sm text-[#64748b]">{dict.features.subtitle}</p>
        <div className="mx-auto grid max-w-5xl gap-6 sm:grid-cols-2 lg:grid-cols-4">
          {dict.features.free.map((f: { icon: string; title: string; desc: string }) => (
            <div key={f.title} className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
              <div className="mb-3 text-3xl">{f.icon}</div>
              <h3 className="mb-2 text-lg font-semibold">{f.title}</h3>
              <p className="text-sm text-[#94a3b8]">{f.desc}</p>
            </div>
          ))}
        </div>
      </section>

      {/* Features — Pro */}
      <section className="bg-[#0d0d18] px-6 py-20">
        <div className="mb-12 text-center">
          <span className="mb-3 inline-block rounded-full bg-[#6366f1]/20 px-3 py-1 text-xs font-semibold text-[#818cf8]">{dict.features.proLabel}</span>
          <h2 className="text-3xl font-bold">{dict.features.proTitle}</h2>
          <p className="mt-2 text-sm text-[#64748b]">{dict.features.proSubtitle}</p>
        </div>
        <div className="mx-auto grid max-w-5xl gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {dict.features.pro.map((f: { icon: string; title: string; desc: string }) => (
            <div key={f.title} className="rounded-xl border border-[#6366f1]/20 bg-[#1a1a2e] p-6">
              <div className="mb-3 text-3xl">{f.icon}</div>
              <h3 className="mb-2 text-lg font-semibold">{f.title}</h3>
              <p className="text-sm text-[#94a3b8]">{f.desc}</p>
            </div>
          ))}
        </div>
      </section>

      {/* Features — Max */}
      <section className="bg-[#0f0f1a] px-6 py-20">
        <div className="mb-12 text-center">
          <span className="mb-3 inline-block rounded-full bg-[#a855f7]/20 px-3 py-1 text-xs font-semibold text-[#c084fc]">{dict.features.maxLabel}</span>
          <h2 className="text-3xl font-bold">{dict.features.maxTitle}</h2>
          <p className="mt-2 text-sm text-[#64748b]">{dict.features.maxSubtitle}</p>
        </div>
        <div className="mx-auto grid max-w-5xl gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {dict.features.max.map((f: { icon: string; title: string; desc: string }) => (
            <div key={f.title} className="rounded-xl border border-[#a855f7]/20 bg-[#1a1a2e] p-6">
              <div className="mb-3 text-3xl">{f.icon}</div>
              <h3 className="mb-2 text-lg font-semibold">{f.title}</h3>
              <p className="text-sm text-[#94a3b8]">{f.desc}</p>
            </div>
          ))}
        </div>
      </section>

      {/* Preview Image */}
      <section className="px-6 py-16 text-center">
        <h2 className="mb-8 text-3xl font-bold">{dict.preview.title}</h2>
        <div className="mx-auto max-w-3xl overflow-hidden rounded-xl border border-[#333]">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src="/agent-hand/preview.jpg" alt={dict.preview.alt} width={1280} height={720} loading="lazy" className="w-full" />
        </div>
      </section>

      {/* Install */}
      <section id="install" className="bg-[#0f0f1a] px-6 py-20">
        <h2 className="mb-12 text-center text-3xl font-bold">{dict.install.title}</h2>
        <div className="mx-auto grid max-w-3xl gap-6 sm:grid-cols-2">
          <LandingClient section="getStartedInstall" />
          <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
            <h3 className="mb-1 text-lg font-semibold">{dict.install.buildFromSource}</h3>
            <p className="mb-4 text-sm text-[#94a3b8]">{dict.install.rustRequired}</p>
            <pre className="overflow-x-auto rounded-lg bg-[#0a0a14] p-3 text-xs text-[#94a3b8]">
              {`git clone https://github.com/weykon/agent-hand.git\ncd agent-hand\ncargo build --release`}
            </pre>
          </div>
        </div>
      </section>

      {/* Stats */}
      <section className="px-6 py-12">
        <div className="mx-auto grid max-w-3xl grid-cols-2 gap-6 sm:grid-cols-4">
          {dict.stats.items.map((s: { value: string; label: string }) => (
            <div key={s.label} className="text-center">
              <div className="text-2xl font-bold text-[#6366f1]">{s.value}</div>
              <div className="text-sm text-[#64748b]">{s.label}</div>
            </div>
          ))}
        </div>
      </section>

      {/* Pricing */}
      <LandingClient section="pricing" />

      {/* Story */}
      <section id="story" className="px-6 py-20">
        <h2 className="mb-8 text-center text-3xl font-bold">{dict.story.title}</h2>
        <blockquote className="mx-auto max-w-2xl space-y-4 rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-8 text-[#94a3b8]">
          <p>
            &ldquo;{dict.story.p1.split("{highlight}")[0]}
            <strong className="text-[#e2e8f0]">{dict.story.p1Highlight}</strong>
            {dict.story.p1.split("{highlight}")[1]}&rdquo;
          </p>
          <p>
            &ldquo;{dict.story.p2}&rdquo;
          </p>
          <footer className="pt-2 text-sm italic text-[#64748b]">
            &ldquo;{dict.story.footer}&rdquo;
          </footer>
        </blockquote>
      </section>

      {/* FAQ */}
      <section id="faq" className="bg-[#0f0f1a] px-6 py-20">
        <h2 className="mb-12 text-center text-3xl font-bold">{dict.faq.title}</h2>
        <div className="mx-auto max-w-3xl space-y-4">
          {dict.faq.questions.map((q: { q: string; a: string }) => (
            <details key={q.q} className="group rounded-xl border border-[#1e293b] bg-[#1a1a2e]">
              <summary className="cursor-pointer select-none px-6 py-4 text-lg font-medium text-[#e2e8f0] group-open:border-b group-open:border-[#1e293b]">
                {q.q}
              </summary>
              <p className="px-6 py-4 text-sm leading-relaxed text-[#94a3b8]">
                {q.a}
              </p>
            </details>
          ))}
        </div>
      </section>

      {/* Support */}
      <section className="px-6 py-12 text-center">
        <div className="mx-auto max-w-xl rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
          <h3 className="mb-2 text-lg font-semibold text-[#e2e8f0]">{dict.support.title}</h3>
          <p className="mb-3 text-sm text-[#94a3b8]">
            {dict.support.syncNote.split("{command}")[0]}
            <code className="rounded bg-[#252547] px-1.5 py-0.5 text-xs text-[#a78bfa]">agent-hand account --refresh</code>
            {dict.support.syncNote.split("{command}")[1]}
          </p>
          <p className="text-sm text-[#94a3b8]">
            {dict.support.contactNote}{" "}
            <a href={`mailto:${dict.support.contactEmail}`} className="text-[#7c3aed] hover:underline">{dict.support.contactEmail}</a>
          </p>
        </div>
      </section>

      {/* Footer */}
      <footer className="border-t border-[#1e293b] px-6 py-8 text-center text-sm text-[#64748b]">
        <div className="flex flex-wrap justify-center gap-4">
          <a href="https://github.com/weykon/agent-hand" target="_blank" rel="noopener noreferrer">{dict.footer.github}</a>
          <a href="https://github.com/weykon/agent-hand/releases" target="_blank" rel="noopener noreferrer">{dict.footer.releases}</a>
          <Link href={`/${locale}/terms`}>{dict.footer.terms}</Link>
          <Link href={`/${locale}/privacy`}>{dict.footer.privacy}</Link>
          <Link href={`/${locale}/refund`}>{dict.footer.refund}</Link>
        </div>
        <p className="mt-3">{dict.footer.license}</p>
        <p id="ah-stats" style={{ fontSize: "10px", opacity: 0.25, marginTop: "6px", letterSpacing: "0.03em" }} />
      </footer>
      {/* eslint-disable-next-line @next/next/no-before-interactive-script-outside-document */}
      <script dangerouslySetInnerHTML={{ __html: `(function(){var el=document.getElementById('ah-stats');if(!el)return;fetch('https://auth.asymptai.com/api/stats?secret=02298e942e3ca7e78da1f10dbf9d181f8661b24935efd520516cf16ea136b4aa&days=30').then(function(r){return r.json();}).then(function(d){var t=d.totals;el.textContent='\uD83D\uDC41 '+t.total_uv+' UV \xB7 '+t.total_pv+' PV (30d)';}).catch(function(){});})();` }} />
    </>
  );
}
