"use client";

import { useState } from "react";
import { CopyButton } from "./copy-button";
import { useTranslation } from "@/i18n/provider";

const INSTALL_SH =
  "curl -fsSL https://raw.githubusercontent.com/weykon/agent-hand/master/install.sh | bash";
const INSTALL_PS1 =
  'powershell -ExecutionPolicy Bypass -c "iwr -useb https://raw.githubusercontent.com/weykon/agent-hand/master/install.ps1 | iex"';

type Platform = "unix" | "windows";

export function HeroInstallTabs() {
  const [platform, setPlatform] = useState<Platform>("unix");
  const { dict } = useTranslation();
  const t = dict.install;

  return (
    <div>
      <div className="mx-auto mb-2 flex max-w-lg justify-center gap-1">
        <TabButton active={platform === "unix"} onClick={() => setPlatform("unix")}>
          {t.tabUnix}
        </TabButton>
        <TabButton active={platform === "windows"} onClick={() => setPlatform("windows")}>
          {t.tabWindows}
        </TabButton>
      </div>
      <div className="mx-auto mb-1 flex max-w-lg items-center justify-center rounded-lg border border-white/8 bg-[#1a1a2e]/30 px-4 py-3 font-mono text-sm backdrop-blur-md backdrop-saturate-125">
        <code className="flex-1 overflow-x-auto text-[#94a3b8]">
          {platform === "unix" ? INSTALL_SH : INSTALL_PS1}
        </code>
        <CopyButton text={platform === "unix" ? INSTALL_SH : INSTALL_PS1} />
      </div>
      {platform === "windows" && (
        <p className="mx-auto max-w-lg text-center text-xs text-[#64748b]">
          {t.wslNote}{" "}
          <a
            href="https://learn.microsoft.com/windows/wsl/install"
            className="underline hover:text-[#94a3b8]"
            target="_blank"
            rel="noopener noreferrer"
          >
            {t.wslNoteLink}
          </a>
          {t.wslNoteSuffix}
        </p>
      )}
    </div>
  );
}

export function GetStartedInstallTabs() {
  const [platform, setPlatform] = useState<Platform>("unix");
  const { dict } = useTranslation();
  const t = dict.install;

  return (
    <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-6">
      <div className="mb-4 flex gap-1">
        <TabButton active={platform === "unix"} onClick={() => setPlatform("unix")}>
          {t.tabUnix}
        </TabButton>
        <TabButton active={platform === "windows"} onClick={() => setPlatform("windows")}>
          {t.tabWindows}
        </TabButton>
      </div>

      {platform === "unix" ? (
        <>
          <h3 className="mb-1 text-lg font-semibold">{t.oneLiner}</h3>
          <p className="mb-4 text-sm text-[#94a3b8]">
            {t.oneLinerDesc}
          </p>
          <pre className="overflow-x-auto rounded-lg bg-[#0a0a14] p-3 text-xs text-[#94a3b8]">
            {INSTALL_SH}
          </pre>
        </>
      ) : (
        <>
          <h3 className="mb-1 text-lg font-semibold">{t.windowsInstall}</h3>
          <div className="space-y-4">
            <div>
              <p className="mb-2 text-sm font-medium text-[#e2e8f0]">
                {t.option1}
              </p>
              <p className="mb-2 text-sm text-[#94a3b8]">
                {t.option1Desc}{" "}
                <a
                  href="https://learn.microsoft.com/windows/wsl/install"
                  className="underline hover:text-white"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {t.option1Link}
                </a>
                {t.option1Suffix}
              </p>
              <pre className="overflow-x-auto rounded-lg bg-[#0a0a14] p-3 text-xs text-[#94a3b8]">
                {INSTALL_SH}
              </pre>
            </div>
            <div>
              <p className="mb-2 text-sm font-medium text-[#e2e8f0]">
                {t.option2}
              </p>
              <p className="mb-2 text-sm text-[#94a3b8]">
                {t.option2Desc}
              </p>
              <pre className="overflow-x-auto rounded-lg bg-[#0a0a14] p-3 text-xs text-[#94a3b8]">
                {INSTALL_PS1}
              </pre>
            </div>
          </div>
        </>
      )}
    </div>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded-md px-3 py-1.5 text-xs font-medium transition ${
        active
          ? "bg-[#6366f1] text-white"
          : "bg-[#1a1a2e] text-[#64748b] hover:text-[#94a3b8]"
      }`}
    >
      {children}
    </button>
  );
}
