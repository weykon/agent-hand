"use client";

import { useSearchParams, useRouter, useParams } from "next/navigation";
import { useEffect, Suspense } from "react";
import { saveAuth } from "@/lib/auth";

function CallbackContent() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const params = useParams();
  const lang = (params.lang as string) ?? "en";
  const token = searchParams.get("token");
  const email = searchParams.get("email");
  const name = searchParams.get("name");
  const avatar = searchParams.get("avatar");

  useEffect(() => {
    if (token && email) {
      saveAuth(token, email, name ?? undefined, avatar ?? undefined);
      router.replace(`/${lang}/account`);
    }
  }, [token, email, name, avatar, router, lang]);

  if (!token || !email) {
    return (
      <div className="flex min-h-screen items-center justify-center px-6">
        <div className="max-w-md text-center">
          <h2 className="mb-2 text-2xl font-bold text-red-400">Auth Error</h2>
          <p className="text-[#94a3b8]">Missing token or email in callback URL.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center px-6">
      <p className="text-[#94a3b8]">Signing you in...</p>
    </div>
  );
}

export default function AuthCallbackPage() {
  return (
    <Suspense fallback={<div className="flex min-h-screen items-center justify-center">Loading...</div>}>
      <CallbackContent />
    </Suspense>
  );
}
