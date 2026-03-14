"use client";

import { HeroInstallTabs, GetStartedInstallTabs } from "./install-tabs";
import { PricingSection } from "./pricing";
import { HeroBg } from "./hero-bg";

export function LandingClient({ section }: { section: "heroInstall" | "getStartedInstall" | "pricing" | "heroBg" }) {
  switch (section) {
    case "heroBg":
      return <HeroBg />;
    case "heroInstall":
      return <HeroInstallTabs />;
    case "getStartedInstall":
      return <GetStartedInstallTabs />;
    case "pricing":
      return <PricingSection />;
  }
}
