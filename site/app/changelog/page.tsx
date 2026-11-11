import { readFileSync } from "node:fs";
import { join } from "node:path";
import { Html, JsonLd } from "@/components/Bits";
import { PageTop } from "@/components/PageTop";
import { crumbsLd } from "@/lib/ld";
import { render } from "@/lib/md";
import { meta } from "@/lib/meta";
import { AUTHOR, PUBLISHER, RELEASED, REPO, ROOT, VERSION, url } from "@/lib/site";

export const metadata = meta({
  path: "/changelog/",
  title: "phonedriver changelog: every release and what changed",
  description: `Release notes for phonedriver, the MCP server that lets AI agents use a phone. The latest version is ${VERSION}. What was added and fixed in each release.`,
});

const SOURCE = readFileSync(join(ROOT, "CHANGELOG.md"), "utf8");
const BODY = SOURCE.slice(SOURCE.indexOf("\n## "));

export default function Changelog() {
  return (
    <>
      <JsonLd
        data={[
          {
            "@context": "https://schema.org",
            "@type": "TechArticle",
            headline: "phonedriver changelog",
            description: "Every release of phonedriver and what changed in it.",
            url: url("/changelog/"),
            inLanguage: "en",
            datePublished: RELEASED,
            dateModified: RELEASED,
            author: AUTHOR,
            publisher: PUBLISHER,
            about: { "@id": url("/#software") },
          },
          crumbsLd([
            ["Home", "/"],
            ["Changelog", "/changelog/"],
          ]),
        ]}
      />
      <PageTop
        say="What changed since last time?"
        short="What changed?"
        crumbs={[["Changelog", "/changelog/"]]}
        list={
          <article className="prose log" data-land="" data-at=".93">
            <Html className="body" html={render(BODY)} />
          </article>
        }
      >
        <h1>Changelog</h1>
        <p className="lede">
          Every release, newest first. The latest version is {VERSION}. Binaries and checksums are on <a href={`${REPO}/releases`}>GitHub Releases</a>.
        </p>
      </PageTop>
    </>
  );
}
