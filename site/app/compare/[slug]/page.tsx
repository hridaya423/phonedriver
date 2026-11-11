import Link from "next/link";
import { Fragment } from "react";
import { Crumbs, Html, JsonLd, OnPage } from "@/components/Bits";
import { COMPARE, page } from "@/lib/content";
import { meta } from "@/lib/meta";
import { AUTHOR, PUBLISHED, PUBLISHER, REPO, ttl, url } from "@/lib/site";

type Props = { params: Promise<{ slug: string }> };

export const dynamicParams = false;
export const generateStaticParams = () => COMPARE.map(([slug]) => ({ slug }));

export async function generateMetadata({ params }: Props) {
  const { slug } = await params;
  const p = page("compare", slug);
  return meta({ path: p.path, title: p.meta.seo || ttl(p.meta.title), description: p.meta.meta || p.meta.description, og: `/og/compare-${slug}.png`, article: true, md: true });
}

export default async function Compare({ params }: Props) {
  const { slug } = await params;
  const p = page("compare", slug);
  const label = COMPARE.find(([s]) => s === slug)![1];
  const others = COMPARE.filter(([s]) => s !== slug);
  return (
    <>
      <JsonLd
        data={[
          {
            "@context": "https://schema.org",
            "@type": "Article",
            headline: p.meta.title,
            description: p.meta.description,
            url: url(p.path),
            inLanguage: "en",
            datePublished: PUBLISHED,
            dateModified: p.meta.updated ?? PUBLISHED,
            author: AUTHOR,
            publisher: PUBLISHER,
            about: [{ "@id": url("/#software") }],
            image: url(`/og/compare-${slug}.png`),
          },
        ]}
      />
      <div className="wrap">
        <Crumbs items={[["Compare", "/compare/"], [label, p.path]]} md />
        <article className="prose">
          <h1>{p.meta.title}</h1>
          <p className="lede">{p.meta.description}</p>
          <div className="article">
            <div className="body">
              <Html html={p.html} />
            <p className="small">
              By <a href="https://github.com/hridaya423">Hridya Agrawal</a>. Last updated <time dateTime={p.meta.updated ?? PUBLISHED}>{p.meta.updated ?? PUBLISHED}</time>. Other comparisons:{" "}
              {others.map(([s, l], i) => (
                <Fragment key={s}>
                  {i > 0 && ", "}
                  <Link href={`/compare/${s}/`}>{l}</Link>
                </Fragment>
              ))}
              . Something wrong or out of date? <a href={`${REPO}/issues`}>Open an issue</a>.
            </p>
            </div>
            <OnPage toc={p.toc} />
          </div>
        </article>
      </div>
    </>
  );
}
