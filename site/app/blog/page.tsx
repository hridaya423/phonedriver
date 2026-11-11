import Link from "next/link";
import { JsonLd } from "@/components/Bits";
import { PageTop } from "@/components/PageTop";
import { TASKS, board, op, ratio } from "@/lib/bench";
import { POSTS, page } from "@/lib/content";
import { unbroken } from "@/lib/md";
import { meta } from "@/lib/meta";
import { url } from "@/lib/site";

export const metadata = meta({
  path: "/blog/",
  title: "phonedriver blog: AI phone agents, benchmarks and design",
  description:
    "Technical posts on how phonedriver lets an AI agent read and operate a phone, the benchmarks behind its speed claims, and the approval guard.",
});

export default function Blog() {
  const b = board();
  const read = op("tree_list");
  const figs: [string, string][] = [
    [ratio(read.adb / read.phonedriver), "faster screen reads"],
    [ratio(b.total("mobile-mcp").tokens / b.total("phonedriver").tokens), "fewer tokens than mobile-mcp"],
    ["Pay?", "the tap it holds"],
    [String(TASKS.length), "jobs only a phone can do"],
  ];
  return (
    <>
      <JsonLd data={[{ "@context": "https://schema.org", "@type": "Blog", name: "phonedriver blog", url: url("/blog/") }]} />
      <PageTop
        say="What did you learn building it?"
        short="What did you learn?"
        crumbs={[["Blog", "/blog/"]]}
        list={
          <div className="list rail">
            {POSTS.map((s, i) => {
              const m = page("blog", s).meta;
              return (
                <Link className="row dated fig" href={`/blog/${s}/`} key={s} {...(i ? {} : { "data-land": "", "data-at": ".93" })}>
                  <span className="lab">{m.date}</span>
                  <div>
                    <h2>
                      <span>{unbroken(m.title)}</span>
                    </h2>
                    <p>{m.description}</p>
                  </div>
                  <b className="row-fig">
                    {figs[i][0]}
                    <small>{figs[i][1]}</small>
                  </b>
                </Link>
              );
            })}
          </div>
        }
      >
        <h1>Blog</h1>
        <p className="lede">How it works, what I measured and what the numbers do not show.</p>
      </PageTop>
    </>
  );
}
