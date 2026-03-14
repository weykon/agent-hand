"use client";

import { useEffect } from "react";
import { useSearchParams } from "next/navigation";
import { Suspense } from "react";
import { detectLocale } from "@/lib/detect-locale";

function ActivateRedirect() {
  const searchParams = useSearchParams();

  useEffect(() => {
    const locale = detectLocale();
    const qs = searchParams.toString();
    const target = `/agent-hand/${locale}/activate/${qs ? `?${qs}` : ""}`;
    window.location.replace(target);
  }, [searchParams]);

  return (
    <div className="flex min-h-screen items-center justify-center">
      <p className="text-[#94a3b8]">Redirecting...</p>
    </div>
  );
}

export default function ActivateRootPage() {
  return (
    <Suspense
      fallback={
        <div className="flex min-h-screen items-center justify-center">
          <p className="text-[#94a3b8]">Redirecting...</p>
        </div>
      }
    >
      <ActivateRedirect />
    </Suspense>
  );
}
