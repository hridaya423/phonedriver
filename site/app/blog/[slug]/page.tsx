import { Crumbs, Html, JsonLd, OnPage } from "@/components/Bits";
import { POSTS, page, words } from "@/lib/content";
import { meta } from "@/lib/meta";
import { AUTHOR, PUBLISHER, REPO, ttl, url } from "@/lib/site";

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
              <Html html={p.html} />
              <p className="small">
                Questions or corrections: <a href={`${REPO}/issues`}>open an issue</a>. Every figure links to the page or file it came from.
              </p>
            </div>
            <OnPage toc={p.toc} />
          </div>
        </article>
      </div>
    </>
  );
}
