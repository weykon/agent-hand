"use client";

import { useState } from "react";
import Link from "next/link";

const CREEM_MAX_MONTHLY_ID = "prod_15F20YtTPacpgeBuWLKu4H";
const CREEM_MAX_YEARLY_ID = "prod_6Ip9uykuzi8KqwBKczqYNx";

export function PricingSection() {
  const [billing, setBilling] = useState<"monthly" | "yearly">("monthly");

  const maxPrice = billing === "monthly" ? 9 : 89;
  const maxPriceLabel = billing === "monthly" ? "/mo" : "/yr";
  const maxSavings = billing === "yearly" ? (
    <span className="ml-2 rounded-full bg-green-900/40 px-2 py-0.5 text-xs text-green-400">Save ~$19</span>
  ) : null;

  return (
    <section id="pricing" className="bg-[#0f0f1a] px-6 py-20 text-center">
      <h2 className="mb-2 text-3xl font-bold">Simple Pricing</h2>
      <p className="mb-8 text-[#94a3b8]">Core features are free forever. Pro is one-time. Max is a subscription.</p>

      {/* Monthly/Yearly toggle */}
      <div className="mb-12 flex items-center justify-center gap-3">
        <button
          onClick={() => setBilling("monthly")}
          className={`rounded-lg px-4 py-2 text-sm font-medium transition ${
            billing === "monthly"
              ? "bg-[#6366f1] text-white"
              : "bg-[#1a1a2e] text-[#94a3b8] hover:text-white"
          }`}
        >
          Monthly
        </button>
        <button
          onClick={() => setBilling("yearly")}
          className={`rounded-lg px-4 py-2 text-sm font-medium transition ${
            billing === "yearly"
              ? "bg-[#6366f1] text-white"
              : "bg-[#1a1a2e] text-[#94a3b8] hover:text-white"
          }`}
        >
          Yearly {maxSavings}
        </button>
      </div>

      <div className="mx-auto grid max-w-4xl gap-6 sm:grid-cols-2 lg:grid-cols-3">
        {/* Free */}
        <div className="rounded-xl border border-[#333] bg-[#1a1a2e] p-6">
          <h3 className="mb-1 text-lg font-semibold">Free</h3>
          <p className="mb-4 text-3xl font-bold text-[#6366f1]">$0</p>
          <ul className="mb-6 space-y-2 text-left text-sm text-[#94a3b8]">
            <li>Session management TUI</li>
            <li>tmux integration</li>
            <li>Status monitoring</li>
            <li>All open-source features</li>
          </ul>
          <a
            href="https://github.com/weykon/agent-hand"
            target="_blank"
            rel="noopener noreferrer"
            className="block rounded-lg border border-[#444] py-2.5 text-center font-semibold hover:border-[#666]"
          >
            Get Started Free
          </a>
        </div>

        {/* Pro */}
        <div className="rounded-xl border border-[#333] bg-[#1a1a2e] p-6">
          <h3 className="mb-1 text-lg font-semibold">Pro</h3>
          <p className="mb-4 text-3xl font-bold text-[#6366f1]">
            $19 <span className="text-base font-normal text-[#94a3b8]">one-time</span>
          </p>
          <ul className="mb-6 space-y-2 text-left text-sm text-[#94a3b8]">
            <li>Everything in Free</li>
            <li><strong className="text-[#e2e8f0]">Auto-upgrade</strong> command</li>
            <li>Priority support</li>
            <li>Future premium features</li>
          </ul>
          <Link
            href="/account"
            className="block rounded-lg bg-[#6366f1] py-2.5 text-center font-semibold text-white hover:bg-[#818cf8]"
          >
            Buy License
          </Link>
        </div>

        {/* Max */}
        <div className="relative rounded-xl border-2 border-[#a855f7] bg-[#1a1a2e] p-6 sm:col-span-2 lg:col-span-1">
          <span className="absolute -top-3 left-1/2 -translate-x-1/2 rounded-full bg-[#a855f7] px-3 py-0.5 text-xs font-semibold text-white">
            POPULAR
          </span>
          <h3 className="mb-1 text-lg font-semibold">Max</h3>
          <p className="mb-4 text-3xl font-bold text-[#a855f7]">
            ${maxPrice} <span className="text-base font-normal text-[#94a3b8]">{maxPriceLabel}</span>
          </p>
          <ul className="mb-6 space-y-2 text-left text-sm text-[#94a3b8]">
            <li>Everything in Pro</li>
            <li><strong className="text-[#e2e8f0]">AI Session Summarizer</strong></li>
            <li><strong className="text-[#e2e8f0]">Remote Sharing & Collaboration</strong></li>
            <li><strong className="text-[#e2e8f0]">Session Relationships & Context</strong></li>
          </ul>
          <Link
            href="/account"
            className="block rounded-lg bg-[#a855f7] py-2.5 text-center font-semibold text-white hover:bg-[#c084fc]"
          >
            Subscribe
          </Link>
        </div>
      </div>

      <p className="mt-8 text-sm text-[#64748b]">
        After purchase, run <code className="rounded bg-[#1a1a2e] px-1.5 py-0.5">agent-hand login</code> to activate.
      </p>
    </section>
  );
}
