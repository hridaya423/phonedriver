import Link from "next/link";
import { Crumbs, Html, JsonLd, OnPage } from "@/components/Bits";
import { DOCS, page } from "@/lib/content";
import { meta } from "@/lib/meta";
import { AUTHOR, PUBLISHED, PUBLISHER, ttl, url } from "@/lib/site";

type Props = { params: Promise<{ slug: string }> };

export const dynamicParams = false;
export const generateStaticParams = () => DOCS.map((slug) => ({ slug }));

export async function generateMetadata({ params }: Props) {
  const p = page("docs", (await params).slug);
  return meta({ path: p.path, title: p.meta.seo || ttl(p.meta.title), description: p.meta.meta || p.meta.description, og: "/og/docs.png", article: true, md: true });
}

export default async function Doc({ params }: Props) {
  const { slug } = await params;
  const p = page("docs", slug);
  const idx = DOCS.indexOf(slug);
  const nav = (s: string) => page("docs", s).meta.nav;
  const prev = DOCS[idx - 1];
  const next = DOCS[idx + 1];
  return (
    <>
      <JsonLd
        data={[
          {
            "@context": "https://schema.org",
            "@type": "TechArticle",
            headline: p.meta.title,
            description: p.meta.description,
            url: url(p.path),
            inLanguage: "en",
            dateModified: p.meta.updated ?? PUBLISHED,
            author: AUTHOR,
            publisher: PUBLISHER,
            isPartOf: { "@type": "WebSite", name: "phonedriver", url: url("/") },
            about: { "@id": url("/#software") },
          },
        ]}
      />
      <div className="wrap">
        <Crumbs items={[["Docs", "/docs/"], [p.meta.nav, p.path]]} md />
        <div className="docs">
          <aside className="side">
            <nav aria-label="Docs">
              {DOCS.map((s) => (
                <Link key={s} href={`/docs/${s}/`} aria-current={s === slug ? "page" : undefined}>
                  {nav(s)}
                </Link>
              ))}
            </nav>
          </aside>
          <article className="prose">
            <h1>{p.meta.title}</h1>
            <p className="lede">{p.meta.description}</p>
            <p className="byline">
              <a href="https://github.com/hridaya423">Hridya Agrawal</a> · Last updated <time dateTime={p.meta.updated ?? PUBLISHED}>{p.meta.updated ?? PUBLISHED}</time>
            </p>
            <Html className="body" html={p.html} />
            <div className="pnav">
              {prev && (
                <Link className="pn" href={`/docs/${prev}/`}>
                  <span className="lab">Previous</span>
                  {nav(prev)}
                </Link>
              )}
              {next && (
                <Link className="pn next" href={`/docs/${next}/`}>
                  <span className="lab">Next</span>
                  {nav(next)}
                </Link>
              )}
            </div>
          </article>
          <OnPage toc={p.toc} />
        </div>
      </div>
    </>
  );
}
