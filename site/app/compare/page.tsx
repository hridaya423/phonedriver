import Link from "next/link";
import { JsonLd } from "@/components/Bits";
import { PageTop } from "@/components/PageTop";
import { board, op, ratio, simulator } from "@/lib/bench";
import { COMPARE, page } from "@/lib/content";
import { listLd } from "@/lib/ld";
import { meta } from "@/lib/meta";

export const metadata = meta({
  path: "/compare/",
  title: "phonedriver vs mobile-mcp, phone-harness and adb, measured",
  description: "Which tool should an AI agent use to control a phone? phonedriver against mobile-mcp, phone-harness, adb, Appium, simulator-use and Argent on tokens, passed runs and speed.",
  og: "/og/compare-adb.png",
});

export default function CompareIndex() {
  const b = board();
  const us = b.total("phonedriver").tokens;
  const read = op("tree_list");
  const tools = simulator().tools;
  const sim = (id: string) => tools.find((t) => t.tool === id)!;
  const figs: Record<string, [string, string]> = {
    adb: [ratio(read.adb / read.phonedriver), "faster screen reads"],
    "mobile-mcp": [ratio(b.total("mobile-mcp").tokens / us), "fewer tokens, nine jobs"],
    "phone-harness": [ratio(b.total("phone-harness").tokens / us), "fewer tokens, nine jobs"],
    appium: ["2.0x", "faster reads than tuned WebDriverAgent"],
    "simulator-use": [ratio(sim("simulator-use").read.medianMs / sim("phonedriver").read.medianMs), "faster reads, iOS Simulator"],
    argent: [ratio(sim("argent").read.medianTokens / sim("phonedriver").read.medianTokens), "fewer tokens a read"],
  };
  return (
    <>
      <JsonLd data={[listLd("phonedriver compared with other tools", "/compare/", COMPARE.map(([s, label]) => [label, `/compare/${s}/`]))]} />
      <PageTop
        say="Why not one of the others?"
        short="Why not the others?"
        crumbs={[["Compare", "/compare/"]]}
        list={
          <div className="list rail">
            {COMPARE.map(([s, label], i) => (
              <Link className="row fig" href={`/compare/${s}/`} key={s} {...(i ? {} : { "data-land": "", "data-at": ".93" })}>
                <h2>
                  <span>{label}</span>
                </h2>
                <p>{page("compare", s).meta.description}</p>
                <b className="row-fig">
                  {figs[s][0]}
                  <small>{figs[s][1]}</small>
                </b>
              </Link>
            ))}
          </div>
        }
      >
        <h1>phonedriver compared with adb, mobile&#8209;mcp, phone&#8209;harness and Appium</h1>
        <p className="lede">
          Four honest side-by-sides: where phonedriver wins ({figs["mobile-mcp"][0]} fewer tokens than mobile&#8209;mcp, {figs.adb[0]} faster list reads than adb) and where you should pick the other tool instead.
        </p>
      </PageTop>
    </>
  );
}
