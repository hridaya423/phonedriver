import { join } from "node:path";

export const SITE_URL = (process.env.PHONEDRIVER_SITE_URL ?? "https://phonedriver.dev").replace(/\/$/, "");
export const REPO = "https://github.com/hridaya423/phonedriver";
export const SPONSORS = "https://github.com/sponsors/hridaya423";
export const GLAMA = "https://glama.ai/mcp/servers/hridaya423/phonedriver";
export const VERSION = "0.1.8";
export const BOOT =
  "Set up phonedriver so you can use my phone. Run `npx -y phonedriver init`, which adds its MCP server to the agents on this machine and checks my setup, and help me fix what it flags. Then run `npx -y phonedriver skill`, follow it, and show me what is on my phone's screen.";
export const TODAY = "2026-11-20";
export const HOME = "2026-11-18";
export const PUBLISHED = "2026-11-10";
export const ROOT = join(process.cwd(), "..");

export const url = (path: string) => SITE_URL + path;
export const raw = (path: string) => `${REPO}/blob/main/${path}`;

export const NAV = [
  { href: "/docs/", label: "Docs" },
  { href: "/benchmarks/", label: "Benchmarks" },
  { href: "/compare/", label: "Compare" },
  { href: "/blog/", label: "Blog" },
];

export const RELEASED = "2026-11-10";
export const LATEST = "2026-11-20";
export const NPM = "https://www.npmjs.com/package/phonedriver";

export const AUTHOR = { "@type": "Person", "@id": SITE_URL + "/#author", name: "Hridya Agrawal", url: "https://github.com/hridaya423", sameAs: ["https://github.com/hridaya423"] };
export const PUBLISHER = { "@type": "Organization", "@id": SITE_URL + "/#org", name: "phonedriver", url: SITE_URL + "/", logo: SITE_URL + "/favicon.svg", sameAs: [REPO, NPM] };

export const ttl = (t: string) => (t.length > 52 ? t : `${t} | phonedriver`);
