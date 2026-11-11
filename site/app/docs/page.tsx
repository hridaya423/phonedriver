import Link from "next/link";
import { PageTop } from "@/components/PageTop";
import { page } from "@/lib/content";
import { meta } from "@/lib/meta";

export const metadata = meta({
  path: "/docs/",
  title: "phonedriver docs: set up an AI agent to control a phone",
  description: "How-to guides for phonedriver: install, Claude Code, Cursor, Android and iOS devices, the approval guard, the shell and troubleshooting.",
  og: "/og/docs.png",
});

const GROUPS: [string, string[]][] = [
  ["Start", ["install", "claude-code", "cursor-and-mcp-clients"]],
  ["Connect a phone", ["android-emulator", "android-phone", "ios-simulator", "iphone"]],
  ["Run it safely", ["approval-guard", "shell", "troubleshooting"]],
];

const COUNT = GROUPS.reduce((n, [, slugs]) => n + slugs.length, 0);

export default function Docs() {
  return (
    <>
      <PageTop
        say="Show me how to set it up."
        short="Show me how."
        crumbs={[["Docs", "/docs/"]]}
        list={
          <div className="rail">
            {GROUPS.map(([name, slugs], g) => (
              <section className="group" key={name}>
                <h2>{name}</h2>
                <div className="list">
                  {slugs.map((slug, k) => {
                    const m = page("docs", slug).meta;
                    const first = g === 0 && k === 0;
                    return (
                      <Link className={first ? "row first" : "row"} href={`/docs/${slug}/`} key={slug} {...(first ? { "data-land": "", "data-at": ".93" } : {})}>
                        <h3>
                          <span>{m.nav}</span>
                        </h3>
                        <p>{m.description}</p>
                        {first && <b className="row-tag">start here</b>}
                      </Link>
                    );
                  })}
                </div>
              </section>
            ))}
          </div>
        }
      >
        <h1>Docs</h1>
        <p className="lede">
          {COUNT} guides, one task each: install it, point your agent at it, connect a phone, and keep the agent from spending your money.
        </p>
      </PageTop>
    </>
  );
}
