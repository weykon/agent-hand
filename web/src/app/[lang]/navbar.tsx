"use client";

import { useEffect, useState, useRef } from "react";
import Link from "next/link";
import { usePathname, useParams } from "next/navigation";
import { isLoggedIn, getEmail, getName, getAvatar, logout, getStatus, getPlanName, isAdmin } from "@/lib/auth";
import { useTranslation } from "@/i18n/provider";
import { LanguageSwitcher } from "./language-switcher";

function GitHubIcon() {
  return (
    <svg height="20" width="20" viewBox="0 0 16 16" fill="currentColor">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
    </svg>
  );
}

const PLAN_COLORS: Record<string, string> = {
  Free: "bg-[#334155] text-[#94a3b8]",
  Pro: "bg-[#7c3aed]/20 text-[#a78bfa]",
  Max: "bg-[#f59e0b]/20 text-[#fbbf24]",
};

export function Navbar() {
  const { dict, lang } = useTranslation();
  const t = dict.nav;
  const pathname = usePathname();
  const isHome = pathname === `/${lang}` || pathname === `/${lang}/`;
  const [loggedIn, setLoggedIn] = useState(false);
  const [email, setEmail] = useState("");
  const [name, setName] = useState("");
  const [avatar, setAvatar] = useState("");
  const [plan, setPlan] = useState<"Free" | "Pro" | "Max">("Free");
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const li = isLoggedIn();
    setLoggedIn(li);
    if (li) {
      setEmail(getEmail() ?? "");
      setName(getName() ?? "");
      setAvatar(getAvatar() ?? "");
      getStatus().then((s) => {
        if (s) {
          setPlan(getPlanName(s));
          if (s.name) setName(s.name);
          if (s.avatar_url) setAvatar(s.avatar_url);
        }
      });
    }
  }, []);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  const displayName = name || email.split("@")[0] || "";
  const initial = (displayName[0] ?? "?").toUpperCase();

  function handleLogout() {
    logout();
    setLoggedIn(false);
    setDropdownOpen(false);
    window.location.href = `/${lang}/`;
  }

  return (
    <header className="border-b border-[#1e293b]">
      <nav className="mx-auto flex max-w-6xl items-center justify-between px-6 py-4">
        <Link href={`/${lang}/`} className="flex items-center gap-2 text-lg font-bold hover:opacity-90">
          <span>🦀</span> Agent Hand
        </Link>

        <div className="flex items-center gap-6 text-sm text-[#94a3b8]">
          {isHome && (
            <>
              <a href="#features" className="hidden sm:inline hover:text-white">{t.features}</a>
              <a href="#install" className="hidden sm:inline hover:text-white">{t.install}</a>
              <a href="#pricing" className="hidden sm:inline hover:text-white">{t.pricing}</a>
              <a href="#story" className="hidden md:inline hover:text-white">{t.story}</a>
              <a href="#faq" className="hidden md:inline hover:text-white">{t.faq}</a>
            </>
          )}

          <Link
            href={`/${lang}/docs`}
            className="hidden sm:inline hover:text-white"
          >
            {t.docs}
          </Link>

          <a
            href="https://github.com/weykon/agent-hand"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1.5 rounded-md border border-[#333] px-3 py-1.5 hover:border-[#555]"
          >
            <GitHubIcon /> <span className="hidden sm:inline">{t.github}</span>
          </a>

          <LanguageSwitcher />

          {loggedIn ? (
            <div className="relative" ref={dropdownRef}>
              <button
                onClick={() => setDropdownOpen(!dropdownOpen)}
                className="flex items-center gap-2 rounded-full border border-[#333] p-0.5 pr-3 hover:border-[#555] transition-colors"
              >
                {avatar ? (
                  <img
                    src={avatar}
                    alt={displayName}
                    className="h-8 w-8 rounded-full object-cover"
                    referrerPolicy="no-referrer"
                  />
                ) : (
                  <span className="flex h-8 w-8 items-center justify-center rounded-full bg-[#7c3aed] text-sm font-semibold text-white">
                    {initial}
                  </span>
                )}
                <span className="hidden sm:inline max-w-[120px] truncate text-white text-sm">
                  {displayName}
                </span>
                <svg className={`h-4 w-4 text-[#64748b] transition-transform ${dropdownOpen ? "rotate-180" : ""}`} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                </svg>
              </button>

              {dropdownOpen && (
                <div className="absolute right-0 mt-2 w-64 rounded-lg border border-[#333] bg-[#1a1a2e] shadow-xl z-50">
                  <div className="border-b border-[#333] px-4 py-3">
                    <p className="text-sm font-medium text-white truncate">{displayName}</p>
                    <p className="text-xs text-[#64748b] truncate">{email}</p>
                    <span className={`mt-1.5 inline-block rounded-full px-2 py-0.5 text-xs font-medium ${PLAN_COLORS[plan] ?? PLAN_COLORS.Free}`}>
                      {plan}
                    </span>
                  </div>
                  <div className="py-1">
                    <Link
                      href={`/${lang}/account`}
                      onClick={() => setDropdownOpen(false)}
                      className="block px-4 py-2 text-sm text-[#94a3b8] hover:bg-[#252547] hover:text-white transition-colors"
                    >
                      {t.account}
                    </Link>
                    {isAdmin() && (
                      <Link
                        href={`/${lang}/admin`}
                        onClick={() => setDropdownOpen(false)}
                        className="block px-4 py-2 text-sm text-[#94a3b8] hover:bg-[#252547] hover:text-white transition-colors"
                      >
                        {t.admin}
                      </Link>
                    )}
                    <button
                      onClick={handleLogout}
                      className="w-full text-left px-4 py-2 text-sm text-red-400 hover:bg-[#252547] hover:text-red-300 transition-colors"
                    >
                      {t.signOut}
                    </button>
                  </div>
                </div>
              )}
            </div>
          ) : (
            <Link
              href={`/${lang}/account`}
              className="rounded-md bg-[#7c3aed] px-4 py-1.5 text-sm font-medium text-white hover:bg-[#6d28d9] transition-colors"
            >
              {t.signIn}
            </Link>
          )}
        </div>
      </nav>
    </header>
  );
}
