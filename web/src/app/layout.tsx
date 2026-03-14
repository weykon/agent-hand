import type { Metadata } from "next";
import { Inter } from "next/font/google";
import "./globals.css";

const inter = Inter({ subsets: ["latin"] });

export const metadata: Metadata = {
  title: "Agent Hand - Terminal Session Manager for AI Agents",
  description:
    "A fast tmux-backed terminal session manager for AI coding agents like Claude, Copilot, and OpenCode",
  icons: {
    icon: "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><text y='.9em' font-size='90'>🦀</text></svg>",
  },
  verification: {
    google: "_5ro5HWJaAo5Ezfh-skV2ScZK7m4Q7V49bKQgKecgNc",
  },
  other: {
    "theme-color": "#0a0a14",
    "apple-mobile-web-app-capable": "yes",
    "apple-mobile-web-app-status-bar-style": "black-translucent",
  },
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark" suppressHydrationWarning>
      <head>
        <link rel="alternate" type="text/plain" href="/agent-hand/llms.txt" title="LLM-friendly site description" />
      </head>
      <body className={`${inter.className} antialiased`}>
        {children}
      </body>
    </html>
  );
}
