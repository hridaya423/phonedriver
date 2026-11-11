import { DEFINITION } from "./data";
import { AUTHOR, REPO, VERSION, url } from "./site";

export type Ld = Record<string, unknown>;

export const crumbsLd = (items: [string, string][]): Ld => ({
  "@context": "https://schema.org",
  "@type": "BreadcrumbList",
  itemListElement: items.map(([name, path], i) => ({ "@type": "ListItem", position: i + 1, name, item: url(path) })),
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
    sameAs: [REPO],
    author: AUTHOR,
    isAccessibleForFree: true,
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
