import Link from "next/link";
import type { Metadata } from "next";

export const metadata: Metadata = { title: "Terms of Service - Agent Hand" };

export default function TermsPage() {
  return (
    <>
      <nav className="flex items-center justify-between border-b border-[#1e293b] px-6 py-4">
        <Link href="/" className="flex items-center gap-2 font-bold">🦀 Agent Hand</Link>
        <Link href="/" className="text-sm text-[#94a3b8] hover:text-white">&larr; Back to Home</Link>
      </nav>
      <main className="mx-auto max-w-3xl px-6 py-12 leading-relaxed text-[#cbd5e1]">
        <h1 className="mb-1 text-3xl font-bold text-[#f1f5f9]">Terms of Service</h1>
        <p className="mb-10 text-sm text-[#64748b]">Last updated: February 2026</p>

        <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">1. Acceptance of Terms</h2>
        <p className="mb-4">By downloading, installing, or using Agent Hand (&ldquo;the Software&rdquo;), you agree to be bound by these Terms of Service. If you do not agree, do not use the Software.</p>

        <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">2. Software License</h2>
        <p className="mb-2">Agent Hand consists of two components with different licensing:</p>
        <ul className="mb-4 ml-6 list-disc space-y-1">
          <li><strong>Open-source core</strong>: Licensed under the MIT License.</li>
          <li><strong>Pro features</strong>: Require a commercial license. Non-transferable, bound to the email used at purchase.</li>
        </ul>

        <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">3. License Restrictions</h2>
        <p className="mb-2">With respect to the Pro license, you may not:</p>
        <ul className="mb-4 ml-6 list-disc space-y-1">
          <li>Transfer, sublicense, or resell your license</li>
          <li>Reverse engineer or decompile Pro feature source code</li>
          <li>Use a single license on more devices than permitted</li>
          <li>Share license credentials with others</li>
        </ul>

        <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">4. Disclaimer of Warranties</h2>
        <p className="mb-4">The Software is provided &ldquo;as is&rdquo; without warranty of any kind, express or implied.</p>

        <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">5. Limitation of Liability</h2>
        <p className="mb-4">To the maximum extent permitted by law, the authors shall not be liable for any indirect, incidental, special, or consequential damages.</p>

        <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">6. Governing Law</h2>
        <p className="mb-4">These Terms are governed by the laws of the jurisdiction in which you reside.</p>

        <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">7. Changes to Terms</h2>
        <p className="mb-4">We may update these Terms from time to time. Continued use constitutes acceptance.</p>

        <h2 className="mb-3 mt-8 border-b border-[#1e293b] pb-1 text-lg font-semibold text-[#e2e8f0]">8. Contact</h2>
        <p>Questions? Email <a href="mailto:contact@asymptai.com" className="text-[#7c3aed]">contact@asymptai.com</a>.</p>
      </main>
    </>
  );
}
