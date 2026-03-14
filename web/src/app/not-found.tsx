"use client";

import { useEffect } from "react";
import { languages } from "@/lib/i18n";
import { detectLocale } from "@/lib/detect-locale";

/**
 * Catch-all 404 handler that redirects non-locale paths to the correct locale.
 *
 * On GitHub Pages (static export), any unmatched path serves 404.html.
 * This page detects the user's locale and redirects:
 *   /agent-hand/admin  →  /agent-hand/en/admin/
 *   /agent-hand/foo    →  /agent-hand/en/foo/
 */
export default function NotFound() {
  useEffect(() => {
    const basePath = "/agent-hand";
    const { pathname, search } = window.location;

    // Strip basePath prefix
    let rest = pathname.startsWith(basePath)
      ? pathname.slice(basePath.length)
      : pathname;

    // Remove leading slash for segment check
    const segments = rest.replace(/^\//, "").split("/").filter(Boolean);

    // If first segment is already a valid locale, this is a genuine 404
    // (the localized route itself doesn't exist)
    if (segments.length > 0 && (languages as readonly string[]).includes(segments[0])) {
      return; // Show the 404 UI below
    }

    // Otherwise, prepend locale and redirect
    const locale = detectLocale();
    const cleanPath = rest.replace(/^\//, "").replace(/\/$/, "");
    const target = `${basePath}/${locale}/${cleanPath}${cleanPath ? "/" : ""}${search}`;
    window.location.replace(target);
  }, []);

  return (
    <div
      style={{
        fontFamily:
          'system-ui, "Segoe UI", Roboto, Helvetica, Arial, sans-serif',
        height: "100vh",
        textAlign: "center",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        color: "#94a3b8",
        background: "#0a0a14",
      }}
    >
      <h1 style={{ fontSize: 48, fontWeight: 700, margin: 0 }}>404</h1>
      <p style={{ marginTop: 8 }}>This page could not be found.</p>
    </div>
  );
}
