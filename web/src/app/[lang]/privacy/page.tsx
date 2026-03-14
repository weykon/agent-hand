import Link from "next/link";
import type { Metadata } from "next";
import { getDictionary } from "@/i18n";
import { languages } from "@/lib/i18n";
import type { Locale } from "@/lib/i18n";

export const metadata: Metadata = { title: "Privacy Policy - Agent Hand" };

export default async function PrivacyPage({
  params,
}: {
  params: Promise<{ lang: string }>;
}) {
  const { lang } = await params;
  const locale = (languages.includes(lang as Locale) ? lang : "en") as Locale;
  const dict = getDictionary(locale);

  return (
    <main className="mx-auto max-w-3xl px-6 py-12 leading-relaxed text-[#cbd5e1]">
      {locale !== "en" && (
        <div className="mb-6 rounded-lg border border-[#6366f1]/30 bg-[#6366f1]/10 px-4 py-3 text-sm text-[#a5b4fc]">
          {dict.legal.englishOnly}
        </div>
      )}
      <h1 className="mb-1 text-3xl font-bold text-[#f1f5f9]">Privacy Policy</h1>
      <p className="mb-10 text-sm text-[#64748b]">Last updated: February 2026</p>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">1. Overview</h2>
      <p className="mb-4">Agent Hand is a local CLI tool. We collect as little data as possible.</p>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">2. Data We Collect</h2>
      <p className="mb-2">We collect only:</p>
      <ul className="mb-4 ml-6 list-disc space-y-1">
        <li><strong>Email address</strong> — via Google OAuth when you run <code className="rounded bg-[#1a1a2e] px-1 py-0.5 text-sm">agent-hand login</code>.</li>
      </ul>
      <p className="mb-2">We do <strong>not</strong> collect:</p>
      <ul className="mb-4 ml-6 list-disc space-y-1">
        <li>Usage data, telemetry, or analytics</li>
        <li>Terminal session content or command history</li>
        <li>IP addresses or device identifiers beyond license validation</li>
      </ul>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">3. How We Use Your Data</h2>
      <p className="mb-2">Your email is used solely to:</p>
      <ul className="mb-4 ml-6 list-disc space-y-1">
        <li>Validate your Pro license</li>
        <li>Send purchase receipts (via creem.io)</li>
        <li>Respond to support or refund requests</li>
      </ul>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">4. Data Storage</h2>
      <p className="mb-4">License data is stored in a PostgreSQL database on a dedicated server. Not shared with third parties beyond payment processing.</p>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">5. Third-Party Payment Processor</h2>
      <p className="mb-4">Payments are processed by creem.io. We do not store credit card information.</p>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">6. Data Retention and Deletion</h2>
      <p className="mb-4">We retain your email while your license is active. To request deletion, email <a href="mailto:contact@asymptai.com" className="text-[#7c3aed]">contact@asymptai.com</a>.</p>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">7. Changes</h2>
      <p className="mb-4">We may update this policy. Updated versions will be posted here.</p>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">8. Contact</h2>
      <p>Email <a href="mailto:contact@asymptai.com" className="text-[#7c3aed]">contact@asymptai.com</a>.</p>
    </main>
  );
}
