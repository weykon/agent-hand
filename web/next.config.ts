import { createMDX } from 'fumadocs-mdx/next';
import type { NextConfig } from "next";

const withMDX = createMDX();

const nextConfig: NextConfig = {
  output: "export",
  basePath: "/agent-hand",
  trailingSlash: true,
};

export default withMDX(nextConfig);
