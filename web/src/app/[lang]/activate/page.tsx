"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";
import { useTranslation } from "@/i18n/provider";

const AUTH_BASE = "https://auth.asymptai.com";

function GoogleIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 48 48">
      <path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z" />
      <path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z" />
      <path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z" />
      <path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.18 1.48-4.97 2.31-8.16 2.31-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z" />
    </svg>
  );
}

function GitHubIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
      <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
    </svg>
  );
}

function ActivateContent() {
  const searchParams = useSearchParams();
  const { dict } = useTranslation();
  const t = dict.activate;
  const code = searchParams.get("code");

  function loginWithGoogle() {
    if (!code) return;
    window.location.href = `${AUTH_BASE}/auth/google?code=${encodeURIComponent(code)}`;
  }

  function loginWithGitHub() {
    if (!code) return;
    window.location.href = `${AUTH_BASE}/auth/github?code=${encodeURIComponent(code)}`;
  }

  if (!code) {
    return (
      <div className="flex min-h-screen items-center justify-center px-6">
        <div className="max-w-md text-center">
          <h2 className="mb-2 text-2xl font-bold">{t.noCode}</h2>
          <p className="text-[#94a3b8]">
            {t.noCodeDesc.split("{command}")[0]}
            <code className="rounded bg-[#1a1a2e] px-1.5 py-0.5">agent-hand login</code>
            {t.noCodeDesc.split("{command}")[1]}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center px-6">
      <div className="w-full max-w-sm rounded-xl border border-[#333] bg-[#1a1a2e] p-8 text-center">
        <h2 className="mb-2 text-2xl font-bold">{t.authorizeDevice}</h2>
        <p className="mb-2 text-[#94a3b8]">{t.yourCode}</p>
        <p className="mb-6 font-mono text-3xl font-bold tracking-widest text-[#6366f1]">{code}</p>
        <p className="mb-6 text-sm text-[#94a3b8]">{t.signInToLink}</p>
        <div className="space-y-3">
          <button
            onClick={loginWithGoogle}
            className="flex w-full items-center justify-center gap-3 rounded-lg bg-white px-4 py-3 font-semibold text-gray-800 hover:bg-gray-100"
          >
            <GoogleIcon />
            {t.signInGoogle}
          </button>
          <button
            onClick={loginWithGitHub}
            className="flex w-full items-center justify-center gap-3 rounded-lg bg-[#24292f] px-4 py-3 font-semibold text-white hover:bg-[#32383f]"
          >
            <GitHubIcon />
            {t.signInGithub}
          </button>
        </div>
      </div>
    </div>
  );
}

export default function ActivatePage() {
  return (
    <Suspense fallback={<div className="flex min-h-screen items-center justify-center">Loading...</div>}>
      <ActivateContent />
    </Suspense>
  );
}
