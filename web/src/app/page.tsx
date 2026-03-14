"use client";

import { useEffect } from "react";
import { detectLocale } from "@/lib/detect-locale";

export default function RootRedirect() {
  useEffect(() => {
    const locale = detectLocale();
    window.location.replace(`/agent-hand/${locale}/`);
  }, []);

  return (
    <div className="flex min-h-screen items-center justify-center">
      <p className="text-[#94a3b8]">Redirecting...</p>
    </div>
  );
}
