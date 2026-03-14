"use client";

import { useState, useRef, useEffect } from "react";
import { useParams } from "next/navigation";
import type { Locale } from "@/lib/i18n";
import { languages, localeNames } from "@/lib/i18n";

export function LanguageSwitcher() {
  const params = useParams();
  const lang = (params.lang as Locale) ?? "en";
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  function switchLocale(newLang: Locale) {
    localStorage.setItem("agent_hand_locale", newLang);
    // Use window.location.pathname (includes basePath) instead of
    // Next.js usePathname() which strips basePath (/agent-hand)
    const segments = window.location.pathname.split("/");
    const langIdx = segments.findIndex((s) => languages.includes(s as Locale));
    if (langIdx !== -1) {
      segments[langIdx] = newLang;
    }
    window.location.href = segments.join("/") || "/";
  }

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1 rounded-md border border-[#333] px-2.5 py-1.5 text-xs text-[#94a3b8] hover:border-[#555] hover:text-white transition-colors"
      >
        <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9" />
        </svg>
        {localeNames[lang]}
      </button>
      {open && (
        <div className="absolute right-0 mt-1 w-32 rounded-lg border border-[#333] bg-[#1a1a2e] shadow-xl z-50 py-1">
          {languages.map((l) => (
            <button
              key={l}
              onClick={() => { switchLocale(l); setOpen(false); }}
              className={`block w-full px-3 py-1.5 text-left text-sm transition-colors ${
                l === lang
                  ? "text-white bg-[#252547]"
                  : "text-[#94a3b8] hover:text-white hover:bg-[#252547]"
              }`}
            >
              {localeNames[l]}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
