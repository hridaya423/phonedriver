import Link from "next/link";
import { Crumbs, Html, JsonLd, OnPage } from "@/components/Bits";
import { BlogFigure } from "@/components/BlogFigures";
import { POSTS, page, words } from "@/lib/content";
import { meta } from "@/lib/meta";
import { AUTHOR, PUBLISHER, REPO, ttl, url } from "@/lib/site";
import "../../blog-figures.css";

type Props = { params: Promise<{ slug: string }> };

export const dynamicParams = false;
export const generateStaticParams = () => POSTS.map((slug) => ({ slug }));

const card = (slug: string) => `/og/post-${POSTS.indexOf(slug) + 1}.png`;

export async function generateMetadata({ params }: Props) {
  const { slug } = await params;
  const p = page("blog", slug);
  return meta({ path: p.path, title: p.meta.seo || ttl(p.meta.title), description: p.meta.meta || p.meta.description, og: card(slug), article: true, md: true });
}

export default async function Post({ params }: Props) {
  const { slug } = await params;
  const p = page("blog", slug);
  const n = words(p.text);
  const m = p.meta;
  return (
    <>
      <JsonLd
        data={[
          {
            "@context": "https://schema.org",
            "@type": "BlogPosting",
            headline: m.title,
            description: m.description,
            url: url(p.path),
            mainEntityOfPage: url(p.path),
            inLanguage: "en",
            datePublished: m.date,
            dateModified: m.updated ?? m.date,
            author: AUTHOR,
            publisher: PUBLISHER,
            image: url(card(slug)),
            wordCount: n,
            about: [{ "@id": url("/#software") }],
          },
        ]}
      />
      <div className="wrap">
        <Crumbs items={[["Blog", "/blog/"], [m.short, p.path]]} md />
        <article className="prose post">
          <h1>{m.title}</h1>
          <p className="lede">{m.description}</p>
          <p className="byline">
            <a href="https://github.com/hridaya423">Hridya Agrawal</a> · {m.date}{m.updated ? `, updated ${m.updated}` : ""} · {Math.max(1, Math.round(n / 220))} min read
          </p>
          <nav className="toc" aria-label="In this post">
            <ol>
              {p.toc.map((t) => (
                <li key={t.id}>
                  <a href={`#${t.id}`}>{t.text}</a>
                </li>
              ))}
            </ol>
          </nav>
          <div className="article">
            <div className="body">
              {p.blocks.map((b, i) => (b.kind === "figure" ? <BlogFigure key={`f-${i}`} id={b.id} /> : <Html key={`h-${i}`} html={b.html} />))}
              <p className="small">
                Want to try it on your own phone? Start with <Link href="/docs/install/">the install guide</Link> or <Link href="/docs/claude-code/">the Claude Code guide</Link>. Found a bug or a flaw in my numbers? <a href={`${REPO}/issues`}>Open an issue</a>.
              </p>
            </div>
            <OnPage toc={p.toc} />
          </div>
        </article>
      </div>
    </>
  );
}
