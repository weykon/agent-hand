"use client";

import { useState } from "react";
import Link from "next/link";
import { useTranslation } from "@/i18n/provider";

export function PricingSection() {
  const [billing, setBilling] = useState<"monthly" | "yearly">("monthly");
  const { dict, lang } = useTranslation();
  const t = dict.pricing;

  const maxPrice = billing === "monthly" ? 9 : 89;
  const maxPriceLabel = billing === "monthly" ? "/mo" : "/yr";
  const maxSavings = billing === "yearly" ? (
    <span className="ml-2 rounded-full bg-green-900/40 px-2 py-0.5 text-xs text-green-400">{t.save}</span>
  ) : null;

  return (
    <section id="pricing" className="bg-[#0f0f1a] px-6 py-20 text-center">
      <h2 className="mb-2 text-3xl font-bold">{t.title}</h2>
      <p className="mb-8 text-[#94a3b8]">{t.subtitle}</p>

      <div className="mb-12 flex items-center justify-center gap-3">
        <button
          onClick={() => setBilling("monthly")}
          className={`rounded-lg px-4 py-2 text-sm font-medium transition ${
            billing === "monthly"
              ? "bg-[#6366f1] text-white"
              : "bg-[#1a1a2e] text-[#94a3b8] hover:text-white"
          }`}
        >
          {t.monthly}
        </button>
        <button
          onClick={() => setBilling("yearly")}
          className={`rounded-lg px-4 py-2 text-sm font-medium transition ${
            billing === "yearly"
              ? "bg-[#6366f1] text-white"
              : "bg-[#1a1a2e] text-[#94a3b8] hover:text-white"
          }`}
        >
          {t.yearly} {maxSavings}
        </button>
      </div>

      <div className="mx-auto grid max-w-4xl gap-6 sm:grid-cols-2 lg:grid-cols-3">
        {/* Free */}
        <div className="rounded-xl border border-[#333] bg-[#1a1a2e] p-6">
          <h3 className="mb-1 text-lg font-semibold">{t.free.name}</h3>
          <p className="mb-4 text-3xl font-bold text-[#6366f1]">{t.free.price}</p>
          <ul className="mb-6 space-y-2 text-left text-sm text-[#94a3b8]">
            {t.free.features.map((f: string) => (
              <li key={f}>{f}</li>
            ))}
          </ul>
          <a
            href="https://github.com/weykon/agent-hand"
            target="_blank"
            rel="noopener noreferrer"
            className="block rounded-lg border border-[#444] py-2.5 text-center font-semibold hover:border-[#666]"
          >
            {t.free.cta}
          </a>
        </div>

        {/* Pro */}
        <div className="rounded-xl border border-[#333] bg-[#1a1a2e] p-6">
          <h3 className="mb-1 text-lg font-semibold">{t.pro.name}</h3>
          <p className="mb-4 text-3xl font-bold text-[#6366f1]">
            {t.pro.price} <span className="text-base font-normal text-[#94a3b8]">{t.pro.period}</span>
          </p>
          <ul className="mb-6 space-y-2 text-left text-sm text-[#94a3b8]">
            {t.pro.features.map((f: string, i: number) => (
              <li key={f}>
                {i === 0 ? f : <strong className="text-[#e2e8f0]">{f}</strong>}
              </li>
            ))}
          </ul>
          <Link
            href={`/${lang}/account`}
            className="block rounded-lg bg-[#6366f1] py-2.5 text-center font-semibold text-white hover:bg-[#818cf8]"
          >
            {t.pro.cta}
          </Link>
        </div>

        {/* Max */}
        <div className="relative rounded-xl border-2 border-[#a855f7] bg-[#1a1a2e] p-6 sm:col-span-2 lg:col-span-1">
          <span className="absolute -top-3 left-1/2 -translate-x-1/2 rounded-full bg-[#a855f7] px-3 py-0.5 text-xs font-semibold text-white">
            {t.max.popular}
          </span>
          <h3 className="mb-1 text-lg font-semibold">{t.max.name}</h3>
          <p className="mb-4 text-3xl font-bold text-[#a855f7]">
            ${maxPrice} <span className="text-base font-normal text-[#94a3b8]">{maxPriceLabel}</span>
          </p>
          <ul className="mb-6 space-y-2 text-left text-sm text-[#94a3b8]">
            {t.max.features.map((f: string, i: number) => (
              <li key={f}>
                {i === 0 ? f : <strong className="text-[#e2e8f0]">{f}</strong>}
              </li>
            ))}
          </ul>
          <Link
            href={`/${lang}/account`}
            className="block rounded-lg bg-[#a855f7] py-2.5 text-center font-semibold text-white hover:bg-[#c084fc]"
          >
            {t.max.cta}
          </Link>
        </div>
      </div>

      <p className="mt-8 text-sm text-[#64748b]">
        {t.activateNote.replace("{command}", "")}
        <code className="rounded bg-[#1a1a2e] px-1.5 py-0.5">agent-hand login</code>
      </p>
    </section>
  );
}
