import Link from "next/link";
import type { Ld } from "@/lib/ld";
import { crumbsLd } from "@/lib/ld";

export function JsonLd({ data }: { data: Ld[] }) {
  return data.map((d, i) => (
    <script key={i} type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(d).replace(/<\//g, "<\\/") }} />
  ));
}

export function Crumbs({ items, md = false }: { items: [string, string][]; md?: boolean }) {
  const all: [string, string][] = [["phonedriver", "/"], ...items];
  return (
    <>
      <JsonLd data={[crumbsLd(all)]} />
      <nav className="crumbs" aria-label="Breadcrumb">
        {all.slice(0, -1).map(([n, p]) => (
          <span key={p}>
            <Link href={p}>{n}</Link>
            <i aria-hidden="true">/</i>
          </span>
        ))}
        <span>{all[all.length - 1][0]}</span>
        {md && (
          <a className="crumbs-md" href={`${all[all.length - 1][1]}index.md`}>
            Read as Markdown
          </a>
        )}
      </nav>
    </>
  );
}

export function OnPage({ toc }: { toc: { id: string; text: string }[] }) {
  if (toc.length < 2) return null;
  return (
    <aside className="onpage">
      <nav aria-label="On this page">
        <p className="lab">On this page</p>
        {toc.map((t) => (
          <a key={t.id} href={`#${t.id}`}>
            {t.text}
          </a>
        ))}
      </nav>
    </aside>
  );
}

export function Html({ html, className }: { html: string; className?: string }) {
  return <div className={className} dangerouslySetInnerHTML={{ __html: html }} />;
}

export function Arrow() {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="square" aria-hidden="true">
      <path d="M4.5 11.5 11.5 4.5M5.5 4.5h6v6" />
    </svg>
  );
}

export function Star() {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M8 1.3l2.06 4.18 4.61.67-3.34 3.25.79 4.6L8 11.83 3.88 14l.79-4.6L1.33 6.15l4.61-.67z" />
    </svg>
  );
}

export function Pane({ title, note, lines, label, children }: { title: string; note?: string; lines?: string[]; label: string; children?: React.ReactNode }) {
  return (
    <figure className="pane">
      <figcaption className="pane-head">
        <span>{title}</span>
        {note && <span>{note}</span>}
      </figcaption>
      {lines && (
        <pre tabIndex={0} aria-label={label}>
          <code>
            {lines.map((ln, i) => {
              const mark = /^([$~+-]) /.exec(ln)?.[1];
              const n = ln.length - ln.trimStart().length;
              return (
                <span key={i} className={mark === "$" ? "tl cmd" : mark ? "tl chg" : "tl"} style={{ "--i": n + 4 } as React.CSSProperties}>
                  {ln.trimStart()}
                </span>
              );
            })}
          </code>
        </pre>
      )}
      {children}
    </figure>
  );
}
