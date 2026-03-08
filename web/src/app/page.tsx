import Link from "next/link";
import type { Metadata } from "next";
import { PricingSection } from "./pricing";
import { CopyButton } from "./copy-button";

const SITE_URL = "https://weykon.github.io/agent-hand";

export const metadata: Metadata = {
  title: "Agent Hand - Terminal Session Manager for AI Coding Agents",
  description:
    "Manage 5+ Claude Code, Gemini CLI, and AI agent sessions from one Rust TUI. Priority jump, fuzzy search, tmux isolation — free & open source. <50ms startup.",
  keywords: [
    "tmux session manager", "AI agent terminal", "Claude Code sessions",
    "AI coding agent manager", "terminal TUI", "Rust developer tools",
    "agent-deck", "tmux multiplexer", "Claude Code", "Gemini CLI",
    "multi-agent terminal", "session manager macOS Linux",
  ],
  alternates: {
    canonical: SITE_URL + "/",
  },
  openGraph: {
    title: "Agent Hand — Manage All Your AI Agent Sessions in One TUI",
    description:
      "Stop losing track of Claude Code prompts. Agent Hand gives you priority jump, fuzzy search, and status tracking for every AI session — built in Rust, starts in <50ms.",
    url: SITE_URL + "/",
    siteName: "Agent Hand",
    images: [
      {
        url: SITE_URL + "/preview.jpg",
        width: 1280,
        height: 720,
        alt: "Agent Hand — AI Session Manager Dashboard",
      },
    ],
    locale: "en_US",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "Agent Hand — Manage All Your AI Agent Sessions in One TUI",
    description:
      "Priority jump, fuzzy search, tmux isolation for Claude Code & AI agents. Free & open source Rust TUI.",
    images: [SITE_URL + "/preview.jpg"],
    creator: "@weykon",
  },
};

function GitHubIcon() {
  return (
    <svg height="20" width="20" viewBox="0 0 16 16" fill="currentColor">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
    </svg>
  );
}

const features = [
  { icon: "🔍", title: "Status Detection", desc: "Clear visual indicators show WAITING, RUNNING, READY, or IDLE status at a glance." },
  { icon: "🎯", title: "Priority Jump (Ctrl+N)", desc: "Instantly jump to the most urgent session. Never miss a confirmation prompt again." },
  { icon: "⚡", title: "Fuzzy Switcher (Ctrl+G)", desc: "Search and switch to any session in milliseconds. Type a few chars, jump directly." },
  { icon: "📊", title: "PTY Monitoring", desc: "Real-time pseudo-terminal usage tracking with system-wide gauge and warnings." },
  { icon: "📁", title: "Groups & Labels", desc: "Organize sessions by project with custom titles and colored labels." },
  { icon: "🔒", title: "tmux Isolation", desc: "Dedicated tmux server won't touch your default tmux. Your configs, your workflow." },
];

const stats = [
  { value: "<50ms", label: "Startup Time" },
  { value: "~8MB", label: "Memory Usage" },
  { value: "2.7MB", label: "Binary Size" },
  { value: "Rust", label: "Powered" },
];

const faqQuestions = [
  {
    "@type": "Question",
    name: "What is Agent Hand?",
    acceptedAnswer: {
      "@type": "Answer",
      text: "Agent Hand is a fast, Rust-based terminal session manager (TUI) for AI coding agents like Claude Code, Gemini CLI, and OpenCode. It uses tmux for isolation, letting you manage 5+ concurrent AI sessions from one dashboard with status detection, priority jump, and fuzzy search.",
    },
  },
  {
    "@type": "Question",
    name: "How do I install Agent Hand?",
    acceptedAnswer: {
      "@type": "Answer",
      text: "Run: curl -fsSL https://raw.githubusercontent.com/weykon/agent-hand/master/install.sh | bash — works on macOS, Linux, and WSL. Or build from source with cargo build --release.",
    },
  },
  {
    "@type": "Question",
    name: "Is Agent Hand free?",
    acceptedAnswer: {
      "@type": "Answer",
      text: "Yes, the core session manager is free and open source (MIT). There is an optional Pro tier ($19 one-time) for power features and a Max tier ($9/month) for AI summarizer, remote sharing, and session relationships.",
    },
  },
  {
    "@type": "Question",
    name: "Does Agent Hand work with Claude Code?",
    acceptedAnswer: {
      "@type": "Answer",
      text: "Yes. Agent Hand was originally built to manage multiple Claude Code sessions. It detects session status (Waiting, Running, Idle), lets you priority-jump to sessions needing input with Ctrl+N, and provides fuzzy search across all sessions with Ctrl+G.",
    },
  },
  {
    "@type": "Question",
    name: "How is Agent Hand different from tmux?",
    acceptedAnswer: {
      "@type": "Answer",
      text: "Agent Hand runs on top of tmux (using a dedicated server so it won't touch your config). It adds AI-agent-specific features: automatic status detection, priority jumping, session grouping, PTY monitoring, and a visual dashboard — things raw tmux doesn't provide.",
    },
  },
];

export default function HomePage() {
  return (
    <>
      {/* Navbar */}
      <header className="border-b border-[#1e293b]">
        <nav className="mx-auto flex max-w-6xl items-center justify-between px-6 py-4">
          <span className="flex items-center gap-2 text-lg font-bold">
            <span>🦀</span> Agent Hand
          </span>
          <div className="flex items-center gap-6 text-sm text-[#94a3b8]">
            <a href="#features" className="hover:text-white">Features</a>
            <a href="#install" className="hover:text-white">Install</a>
            <a href="#pricing" className="hover:text-white">Pricing</a>
            <a href="#story" className="hover:text-white">Story</a>
            <a href="#faq" className="hover:text-white">FAQ</a>
            <a href="https://github.com/weykon/agent-hand" target="_blank" rel="noopener noreferrer"
              className="flex items-center gap-1.5 rounded-md border border-[#333] px-3 py-1.5 hover:border-[#555]">
              <GitHubIcon /> GitHub
            </a>
          </div>
        </nav>
      </header>

      {/* Hero */}
      <section className="px-6 py-20 text-center">
        <h1 className="mb-4 text-5xl font-bold tracking-tight">Agent Hand</h1>
        <p className="mx-auto mb-2 max-w-2xl text-lg text-[#94a3b8]">
          Agent Hand is an open-source Rust TUI that manages multiple AI coding agent sessions — Claude Code, Gemini CLI, OpenCode — from one tmux-backed dashboard with {"<"}50ms startup.
        </p>
        <p className="mx-auto mb-8 max-w-xl text-sm text-[#64748b]">
          Priority jump to urgent sessions, fuzzy search across all agents, real-time status detection, and PTY monitoring. Free and MIT-licensed.
        </p>
        <div className="mx-auto mb-4 flex max-w-lg items-center justify-center rounded-lg border border-[#333] bg-[#1a1a2e] px-4 py-3 font-mono text-sm">
          <code className="flex-1 text-[#94a3b8]">
            curl -fsSL https://raw.githubusercontent.com/weykon/agent-hand/master/install.sh | bash
          </code>
          <CopyButton text="curl -fsSL https://raw.githubusercontent.com/weykon/agent-hand/master/install.sh | bash" />
        </div>
        <div className="flex justify-center gap-2">
          {["macOS", "Linux", "WSL"].map((p) => (
            <span key={p} className="rounded-full bg-[#1a1a2e] px-3 py-1 text-xs text-[#94a3b8]">{p}</span>
          ))}
        </div>
      </section>

      {/* Terminal Preview */}
      <section className="mx-auto max-w-2xl px-6 pb-16">
        <div className="overflow-hidden rounded-xl border border-[#333] bg-[#1a1a2e]">
          <div className="flex items-center gap-2 border-b border-[#333] px-4 py-2">
            <span className="h-3 w-3 rounded-full bg-red-500" />
            <span className="h-3 w-3 rounded-full bg-yellow-500" />
            <span className="h-3 w-3 rounded-full bg-green-500" />
            <span className="ml-2 text-xs text-[#64748b]">agent-hand</span>
          </div>
          <div className="space-y-2 p-4 font-mono text-sm">
            {[
              { icon: "!", color: "text-yellow-400", name: "claude-main", group: "[work]", status: "Waiting for input..." },
              { icon: "●", color: "text-blue-400", name: "research-agent", group: "[vibecoding]", status: "Thinking..." },
              { icon: "✓", color: "text-green-400", name: "bugfix-session", group: "[debug]", status: "Completed 5m ago" },
              { icon: "○", color: "text-[#64748b]", name: "new-project", group: "[experiments]", status: "Not started" },
            ].map((s) => (
              <div key={s.name} className="flex items-center gap-3">
                <span className={s.color}>{s.icon}</span>
                <span className="text-[#e2e8f0]">{s.name}</span>
                <span className="text-[#64748b]">{s.group}</span>
                <span className="ml-auto text-[#94a3b8]">{s.status}</span>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Features */}
      <section id="features" className="bg-[#0f0f1a] px-6 py-20">
        <h2 className="mb-12 text-center text-3xl font-bold">Key Features</h2>
        <div className="mx-auto grid max-w-5xl gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((f) => (
            <div key={f.title} className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
              <div className="mb-3 text-3xl">{f.icon}</div>
              <h3 className="mb-2 text-lg font-semibold">{f.title}</h3>
              <p className="text-sm text-[#94a3b8]">{f.desc}</p>
            </div>
          ))}
        </div>
      </section>

      {/* Preview Image */}
      <section className="px-6 py-16 text-center">
        <h2 className="mb-8 text-3xl font-bold">See It In Action</h2>
        <div className="mx-auto max-w-3xl overflow-hidden rounded-xl border border-[#333]">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src="/agent-hand/preview.jpg" alt="Agent Hand — AI terminal session manager dashboard showing Claude Code, Gemini CLI sessions with status indicators" width={1280} height={720} loading="lazy" className="w-full" />
        </div>
      </section>

      {/* Install */}
      <section id="install" className="bg-[#0f0f1a] px-6 py-20">
        <h2 className="mb-12 text-center text-3xl font-bold">Get Started</h2>
        <div className="mx-auto grid max-w-3xl gap-6 sm:grid-cols-2">
          <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
            <h3 className="mb-1 text-lg font-semibold">One-Liner Install</h3>
            <p className="mb-4 text-sm text-[#94a3b8]">macOS / Linux / WSL</p>
            <pre className="overflow-x-auto rounded-lg bg-[#0a0a14] p-3 text-xs text-[#94a3b8]">
              {`curl -fsSL https://raw.githubusercontent.com/weykon/agent-hand/master/install.sh | bash`}
            </pre>
          </div>
          <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
            <h3 className="mb-1 text-lg font-semibold">Build from Source</h3>
            <p className="mb-4 text-sm text-[#94a3b8]">Rust toolchain required</p>
            <pre className="overflow-x-auto rounded-lg bg-[#0a0a14] p-3 text-xs text-[#94a3b8]">
              {`git clone https://github.com/weykon/agent-hand.git\ncd agent-hand\ncargo build --release`}
            </pre>
          </div>
        </div>
      </section>

      {/* Stats */}
      <section className="px-6 py-12">
        <div className="mx-auto grid max-w-3xl grid-cols-2 gap-6 sm:grid-cols-4">
          {stats.map((s) => (
            <div key={s.label} className="text-center">
              <div className="text-2xl font-bold text-[#6366f1]">{s.value}</div>
              <div className="text-sm text-[#64748b]">{s.label}</div>
            </div>
          ))}
        </div>
      </section>

      {/* Pricing */}
      <PricingSection />

      {/* Story */}
      <section id="story" className="px-6 py-20">
        <h2 className="mb-8 text-center text-3xl font-bold">From the Creator</h2>
        <blockquote className="mx-auto max-w-2xl space-y-4 rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-8 text-[#94a3b8]">
          <p>
            &ldquo;In early 2025, I was juggling <strong className="text-[#e2e8f0]">5+ Claude Code instances</strong> simultaneously.
            Four terminal windows, each with 3-4 tmux panes. &lsquo;Did I already respond to that prompt?&rsquo;
            &lsquo;Which Claude is working on which task?&rsquo; Wasting 10+ minutes just finding the right session.&rdquo;
          </p>
          <p>
            &ldquo;I tried the original agent-deck (Go) and loved the concept, but wanted better performance,
            more features, and cleaner integration. Agent Hand was born — a Rust rewrite that keeps what works
            and adds what I needed.&rdquo;
          </p>
          <footer className="pt-2 text-sm italic text-[#64748b]">
            &ldquo;The best tool is the one you&apos;ll actually use.&rdquo;
          </footer>
        </blockquote>
      </section>

      {/* FAQ */}
      <section id="faq" className="bg-[#0f0f1a] px-6 py-20">
        <h2 className="mb-12 text-center text-3xl font-bold">Frequently Asked Questions</h2>
        <div className="mx-auto max-w-3xl space-y-4">
          {faqQuestions.map((q) => (
            <details key={q.name} className="group rounded-xl border border-[#1e293b] bg-[#1a1a2e]">
              <summary className="cursor-pointer select-none px-6 py-4 text-lg font-medium text-[#e2e8f0] group-open:border-b group-open:border-[#1e293b]">
                {q.name}
              </summary>
              <p className="px-6 py-4 text-sm leading-relaxed text-[#94a3b8]">
                {q.acceptedAnswer.text}
              </p>
            </details>
          ))}
        </div>
      </section>

      {/* Footer */}
      <footer className="border-t border-[#1e293b] px-6 py-8 text-center text-sm text-[#64748b]">
        <div className="flex flex-wrap justify-center gap-4">
          <a href="https://github.com/weykon/agent-hand" target="_blank" rel="noopener noreferrer">GitHub</a>
          <a href="https://github.com/weykon/agent-hand/releases" target="_blank" rel="noopener noreferrer">Releases</a>
          <Link href="/terms">Terms</Link>
          <Link href="/privacy">Privacy</Link>
          <Link href="/refund">Refund Policy</Link>
        </div>
        <p className="mt-3">MIT License</p>
        <p id="ah-stats" style={{ fontSize: "10px", opacity: 0.25, marginTop: "6px", letterSpacing: "0.03em" }} />
      </footer>
      {/* eslint-disable-next-line @next/next/no-before-interactive-script-outside-document */}
      <script dangerouslySetInnerHTML={{ __html: `(function(){var el=document.getElementById('ah-stats');if(!el)return;fetch('https://auth.asymptai.com/api/stats?secret=02298e942e3ca7e78da1f10dbf9d181f8661b24935efd520516cf16ea136b4aa&days=30').then(function(r){return r.json();}).then(function(d){var t=d.totals;el.textContent='\uD83D\uDC41 '+t.total_uv+' UV \xB7 '+t.total_pv+' PV (30d)';}).catch(function(){});})();` }} />
    </>
  );
}