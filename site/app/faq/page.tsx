import { Crumbs, Html, JsonLd, OnPage } from "@/components/Bits";
import { faqLd } from "@/lib/content";
import { FAQ_GROUPS, HOME_FAQ, faqAnswer } from "@/lib/data";
import { inline, plain, slug } from "@/lib/md";
import { meta } from "@/lib/meta";
import { REPO, TODAY } from "@/lib/site";

export const metadata = meta({
  path: "/faq/",
  title: "phonedriver FAQ: AI agents on Android phones and iPhones",
  description: "Short answers about phonedriver: setup, real phones, speed against uiautomator dump, tokens, WebDriverAgent, other tools, approval before paying and telemetry.",
  md: true,
});

const ON_HOME = new Set(HOME_FAQ.map(([q]) => q));
const ALL = FAQ_GROUPS.flatMap(([, qs]) => qs);

export default function Faq() {
  return (
    <>
      <JsonLd data={[faqLd(ALL.filter((q) => !ON_HOME.has(q)).map((q) => [q, plain(faqAnswer(q))]))]} />
      <div className="wrap">
        <Crumbs items={[["FAQ", "/faq/"]]} md />
        <article className="prose">
          <h1>Questions about phonedriver</h1>
          <p className="lede">
            {ALL.length} short, straight answers—no fluff, real numbers, and links to the raw runs whenever a number shows up.
          </p>
          <div className="article">
            <div className="body">
              {FAQ_GROUPS.map(([name, qs]) => (
                <section key={name}>
                  <h2 id={slug(name)}>{name}</h2>
                  {qs.map((q) => (
                    <div key={q}>
                      <h3 id={slug(q)}>{q}</h3>
                      <Html html={`<p>${inline(faqAnswer(q))}</p>`} />
                    </div>
                  ))}
                </section>
              ))}
              <p className="small">
                Last updated <time dateTime={TODAY}>{TODAY}</time>. A question that is not here: <a href={`${REPO}/discussions`}>ask in Discussions</a>.
              </p>
            </div>
            <OnPage toc={FAQ_GROUPS.map(([name]) => ({ id: slug(name), text: name }))} />
          </div>
        </article>
      </div>
    </>
  );
}
