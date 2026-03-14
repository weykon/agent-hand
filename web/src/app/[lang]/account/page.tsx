"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { getStatus, getToken, isLoggedIn, logout, getEmail, isPro, isMax, getPlanName, getDevices, unbindDevice } from "@/lib/auth";
import type { UserStatus, DevicesResponse } from "@/lib/auth";
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

export default function AccountPage() {
  const { dict, lang } = useTranslation();
  const t = dict.account;
  const [status, setStatus] = useState<UserStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [loggedIn, setLoggedIn] = useState(false);
  const [devices, setDevices] = useState<DevicesResponse | null>(null);
  const [unbinding, setUnbinding] = useState<string | null>(null);
  const [checkoutPending, setCheckoutPending] = useState(false);

  useEffect(() => {
    setLoggedIn(isLoggedIn());
    if (!isLoggedIn()) { setLoading(false); return; }

    const params = new URLSearchParams(window.location.search);
    const isCheckoutReturn = params.get("checkout") === "success";

    if (isCheckoutReturn) {
      window.history.replaceState({}, "", window.location.pathname);
    }

    Promise.all([getStatus(), getDevices()]).then(([s, d]) => {
      setStatus(s);
      setDevices(d);
      setLoading(false);

      if (isCheckoutReturn && !s?.features?.includes("upgrade")) {
        setCheckoutPending(true);
        let attempts = 0;
        const poll = setInterval(async () => {
          attempts++;
          const fresh = await getStatus();
          if (fresh?.features?.includes("upgrade") || attempts >= 10) {
            clearInterval(poll);
            setStatus(fresh);
            setCheckoutPending(false);
            if (fresh?.features?.includes("upgrade")) {
              const dd = await getDevices();
              setDevices(dd);
            }
          }
        }, 3000);
      }
    });
  }, []);

  function handleLogout() {
    logout();
    setLoggedIn(false);
    setStatus(null);
  }

  async function handleRefresh() {
    setLoading(true);
    const [s, d] = await Promise.all([getStatus(), getDevices()]);
    setStatus(s);
    setDevices(d);
    setLoading(false);
  }

  async function handleUnbind(deviceId: string) {
    if (!confirm(t.removeDeviceConfirm)) return;
    setUnbinding(deviceId);
    const ok = await unbindDevice(deviceId);
    setUnbinding(null);
    if (ok) {
      const d = await getDevices();
      setDevices(d);
    }
  }

  const [checkoutLoading, setCheckoutLoading] = useState(false);

  async function handleCheckout(productId: string) {
    const token = getToken();
    if (!token) return;
    setCheckoutLoading(true);
    try {
      const res = await fetch(`${AUTH_BASE}/checkout/create`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ product_id: productId }),
      });
      if (!res.ok) {
        alert("Failed to create checkout session. Please try again.");
        return;
      }
      const data = await res.json();
      if (data.checkout_url) {
        window.location.href = data.checkout_url;
      }
    } finally {
      setCheckoutLoading(false);
    }
  }

  if (!loggedIn) {
    return (
      <div className="flex min-h-screen items-center justify-center px-6">
        <div className="w-full max-w-sm rounded-xl border border-[#333] bg-[#1a1a2e] p-8 text-center">
          <h2 className="mb-4 text-2xl font-bold">{t.title}</h2>
          <p className="mb-6 text-sm text-[#94a3b8]">
            {t.signInDesc.split("{command}")[0]}
            <code className="rounded bg-[#0f0f1a] px-1.5 py-0.5">agent-hand login</code>
            {t.signInDesc.split("{command}")[1]}
          </p>
          <div className="space-y-3">
            <a
              href={`${AUTH_BASE}/auth/google`}
              className="flex w-full items-center justify-center gap-3 rounded-lg bg-white px-4 py-3 font-semibold text-gray-800 hover:bg-gray-100"
            >
              <GoogleIcon />
              {t.signInGoogle}
            </a>
            <a
              href={`${AUTH_BASE}/auth/github`}
              className="flex w-full items-center justify-center gap-3 rounded-lg bg-[#24292f] px-4 py-3 font-semibold text-white hover:bg-[#32383f]"
            >
              <GitHubIcon />
              {t.signInGithub}
            </a>
          </div>
          <Link href={`/${lang}/`} className="mt-6 block text-sm text-[#6366f1] hover:text-[#818cf8]">
            &larr; {t.backToHome}
          </Link>
        </div>
      </div>
    );
  }

  const plan = getPlanName(status);
  const userEmail = status?.email ?? getEmail() ?? "";

  const planColors: Record<string, string> = {
    Free: "text-[#94a3b8]",
    Pro: "text-[#4ade80]",
    Max: "text-[#a855f7]",
  };

  const CREEM_PRO_PRODUCT_ID = "prod_44F1yThRt3QEV6QnkeWNjO";
  const CREEM_MAX_MONTHLY_ID = "prod_15F20YtTPacpgeBuWLKu4H";

  return (
    <div className="mx-auto max-w-lg px-6 py-16">
      <h2 className="mb-8 text-2xl font-bold">{t.title}</h2>

      {checkoutPending && (
        <div className="mb-6 flex items-center gap-3 rounded-xl border border-[#6366f1]/30 bg-[#6366f1]/10 p-4">
          <svg className="h-5 w-5 animate-spin text-[#6366f1]" viewBox="0 0 24 24" fill="none">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
          <p className="text-sm text-[#a5b4fc]">{t.processingPayment}</p>
        </div>
      )}

      {loading ? (
        <p className="text-[#94a3b8]">{t.loading}</p>
      ) : (
        <div className="space-y-6">
          <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
            <p className="mb-1 text-sm text-[#64748b]">{t.email}</p>
            <p className="font-medium">{userEmail}</p>
          </div>

          <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
            <p className="mb-1 text-sm text-[#64748b]">{t.plan}</p>
            <p className="text-lg font-semibold">
              <span className={planColors[plan]}>{plan}</span>
            </p>
            {isPro(status) && status?.purchased_at && (
              <p className="mt-1 text-sm text-[#64748b]">
                {t.purchased}: {new Date(status.purchased_at).toLocaleDateString()}
              </p>
            )}
            {isMax(status) && status?.subscription_status && (
              <div className="mt-2 space-y-1">
                <p className="text-sm text-[#64748b]">
                  {t.subscription}:{" "}
                  <span className={status.subscription_status === "active" ? "text-green-400" : "text-yellow-400"}>
                    {status.subscription_status}
                  </span>
                </p>
                {status.subscription_end_date && (
                  <p className="text-sm text-[#64748b]">
                    {status.subscription_status === "active" ? t.nextBilling : t.accessUntil}:{" "}
                    {new Date(status.subscription_end_date).toLocaleDateString()}
                  </p>
                )}
              </div>
            )}
          </div>

          <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
            <p className="mb-2 text-sm text-[#64748b]">{t.includedFeatures}</p>
            <ul className="space-y-1 text-sm text-[#94a3b8]">
              <li>{t.sessionManagement}</li>
              <li>{t.tmuxIntegration}</li>
              {isPro(status) && (
                <>
                  <li className="text-[#4ade80]">{t.autoUpgrade}</li>
                  <li className="text-[#4ade80]">{t.prioritySupport}</li>
                </>
              )}
              {isMax(status) && (
                <>
                  <li className="text-[#a855f7]">{t.aiSummarizer}</li>
                  <li className="text-[#a855f7]">{t.remoteSharing}</li>
                  <li className="text-[#a855f7]">{t.sessionRelationships}</li>
                </>
              )}
            </ul>
          </div>

          {(isPro(status) || isMax(status)) && devices && (
            <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
              <div className="mb-3 flex items-center justify-between">
                <p className="text-sm text-[#64748b]">{t.activeDevices}</p>
                <span className="rounded-full bg-[#1e293b] px-2.5 py-0.5 text-xs font-medium text-[#94a3b8]">
                  {devices.active_count} / {devices.device_limit}
                </span>
              </div>
              {devices.devices.length === 0 ? (
                <p className="text-sm text-[#475569]">{t.noDevices}</p>
              ) : (
                <ul className="space-y-3">
                  {devices.devices.map((d) => (
                    <li key={d.device_id} className="flex items-center justify-between rounded-lg border border-[#1e293b] bg-[#0f0f1a] px-4 py-3">
                      <div>
                        <p className="font-medium text-sm">{d.hostname || "Unknown"}</p>
                        <p className="text-xs text-[#64748b]">
                          {d.os_arch} &middot; Last seen {new Date(d.last_seen).toLocaleDateString()}
                        </p>
                      </div>
                      <button
                        onClick={() => handleUnbind(d.device_id)}
                        disabled={unbinding === d.device_id}
                        className="rounded-md border border-red-900/50 px-3 py-1 text-xs text-red-400 hover:bg-red-900/20 disabled:opacity-50"
                      >
                        {unbinding === d.device_id ? "..." : t.remove}
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              <p className="mt-3 text-xs text-[#475569]">
                {t.inactiveNote}
              </p>
            </div>
          )}

          {plan === "Free" && (
            <div className="space-y-3">
              <button
                onClick={() => handleCheckout(CREEM_PRO_PRODUCT_ID)}
                disabled={checkoutLoading}
                className="block w-full rounded-lg bg-[#6366f1] py-3 text-center font-semibold text-white hover:bg-[#818cf8] disabled:opacity-50"
              >
                {checkoutLoading ? t.redirecting : t.upgradePro}
              </button>
              <button
                onClick={() => handleCheckout(CREEM_MAX_MONTHLY_ID)}
                disabled={checkoutLoading}
                className="block w-full rounded-lg border-2 border-[#a855f7] py-3 text-center font-semibold text-[#a855f7] hover:bg-[#a855f7]/10 disabled:opacity-50"
              >
                {checkoutLoading ? t.redirecting : t.upgradeMax}
              </button>
            </div>
          )}

          {plan === "Pro" && (
            <button
              onClick={() => handleCheckout(CREEM_MAX_MONTHLY_ID)}
              disabled={checkoutLoading}
              className="block w-full rounded-lg bg-[#a855f7] py-3 text-center font-semibold text-white hover:bg-[#c084fc] disabled:opacity-50"
            >
              {checkoutLoading ? t.redirecting : t.upgradeToMax}
            </button>
          )}

          <div className="flex gap-3">
            <button
              onClick={handleRefresh}
              className="flex-1 rounded-lg border border-[#333] py-2.5 text-center font-medium hover:border-[#555]"
            >
              {t.refresh}
            </button>
            <button
              onClick={handleLogout}
              className="flex-1 rounded-lg border border-red-900 py-2.5 text-center font-medium text-red-400 hover:bg-red-900/20"
            >
              {t.logout}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
