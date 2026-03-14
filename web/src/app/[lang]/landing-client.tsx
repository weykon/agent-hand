"use client";

import { HeroInstallTabs, GetStartedInstallTabs } from "./install-tabs";
import { PricingSection } from "./pricing";

export function LandingClient({ section }: { section: "heroInstall" | "getStartedInstall" | "pricing" }) {
  switch (section) {
    case "heroInstall":
      return <HeroInstallTabs />;
    case "getStartedInstall":
      return <GetStartedInstallTabs />;
    case "pricing":
      return <PricingSection />;
  }
}
