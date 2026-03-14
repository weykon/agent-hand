import Link from "next/link";
import type { Metadata } from "next";
import { getDictionary } from "@/i18n";
import { languages } from "@/lib/i18n";
import type { Locale } from "@/lib/i18n";

export const metadata: Metadata = { title: "Refund Policy - Agent Hand" };

export default async function RefundPage({
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
      <h1 className="mb-1 text-3xl font-bold text-[#f1f5f9]">Refund Policy</h1>
      <p className="mb-10 text-sm text-[#64748b]">Last updated: February 2026</p>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">1. 14-Day Money-Back Guarantee</h2>
      <p className="mb-4">If you are not satisfied, you may request a full refund within <strong>14 days</strong> of purchase, no questions asked.</p>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">2. How to Request a Refund</h2>
      <ul className="mb-4 ml-6 list-disc space-y-1">
        <li>Email <a href="mailto:contact@asymptai.com" className="text-[#7c3aed]">contact@asymptai.com</a> with subject &ldquo;Refund Request&rdquo;</li>
        <li>Include your purchase email and order ID</li>
        <li>We process requests within 2 business days</li>
      </ul>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">3. Refund Processing</h2>
      <p className="mb-4">Refunds are returned to your original payment method within 3-5 business days.</p>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">4. License Revocation</h2>
      <p className="mb-4">Upon refund, your Pro license will be immediately deactivated.</p>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">5. Eligibility</h2>
      <p className="mb-2">Refunds are available for:</p>
      <ul className="mb-4 ml-6 list-disc space-y-1">
        <li>Purchases within the last 14 days</li>
        <li>First-time purchases of a given license tier</li>
      </ul>
      <p className="mb-2">Not available for:</p>
      <ul className="mb-4 ml-6 list-disc space-y-1">
        <li>Purchases older than 14 days</li>
        <li>Renewals where the previous period was already refunded</li>
      </ul>

      <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">6. Contact</h2>
      <p>Email <a href="mailto:contact@asymptai.com" className="text-[#7c3aed]">contact@asymptai.com</a>.</p>
    </main>
  );
}
