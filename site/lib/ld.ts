import { DEFINITION } from "./data";
import { AUTHOR, NPM, PUBLISHER, RELEASED, REPO, TODAY, VERSION, url } from "./site";

export type Ld = Record<string, unknown>;

export const crumbsLd = (items: [string, string][]): Ld => ({
  "@context": "https://schema.org",
  "@type": "BreadcrumbList",
  itemListElement: items.map(([name, path], i) => ({ "@type": "ListItem", position: i + 1, name, item: url(path) })),
});

export const listLd = (name: string, path: string, items: [string, string][]): Ld => ({
  "@context": "https://schema.org",
  "@type": "CollectionPage",
  name,
  url: url(path),
  isPartOf: { "@type": "WebSite", name: "phonedriver", url: url("/") },
  about: { "@id": url("/#software") },
  mainEntity: {
    "@type": "ItemList",
    numberOfItems: items.length,
    itemListElement: items.map(([label, to], i) => ({ "@type": "ListItem", position: i + 1, name: label, url: url(to) })),
  },
});

export const softwareLd = (): Ld[] => [
  {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    "@id": url("/#software"),
    name: "phonedriver",
    alternateName: "phonedriver MCP server",
    description: DEFINITION,
    url: url("/"),
    applicationCategory: "DeveloperApplication",
    operatingSystem: "macOS, Linux, Windows (controls Android and iOS devices)",
    softwareVersion: VERSION,
    license: "https://www.gnu.org/licenses/agpl-3.0.html",
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    featureList: [
      "Text snapshot of an Android or iOS screen",
      "Tap, type, swipe, scroll and press",
      "Every action returns the lines that changed after the screen settles",
      "MCP server and CLI",
      "Approval guard for taps that spend money",
      "No telemetry",
    ],
    sameAs: [REPO, NPM],
    author: AUTHOR,
    publisher: PUBLISHER,
    isAccessibleForFree: true,
    datePublished: RELEASED,
    dateModified: TODAY,
    downloadUrl: `${REPO}/releases/latest`,
    installUrl: NPM,
    releaseNotes: url("/changelog/"),
    softwareHelp: url("/docs/"),
    softwareRequirements: "Node 18 or later for npx. adb for Android. A Mac with Xcode for iOS.",
    screenshot: url("/assets/screen-settings.jpg"),
    image: url("/assets/og-home.png"),
    keywords: "MCP server, Model Context Protocol, AI agent, Claude Code, Codex, Cursor, Android automation, iOS automation, adb, mobile testing",
  },
  {
    "@context": "https://schema.org",
    "@type": "SoftwareSourceCode",
    "@id": url("/#source"),
    name: "phonedriver source",
    codeRepository: REPO,
    programmingLanguage: "Rust",
    license: "https://www.gnu.org/licenses/agpl-3.0.html",
    targetProduct: { "@id": url("/#software") },
  },
];
