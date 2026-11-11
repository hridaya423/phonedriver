import Link from "next/link";
import { PageTop } from "@/components/PageTop";
import { board, op, ratio } from "@/lib/bench";
import { COMPARE, page } from "@/lib/content";
import { meta } from "@/lib/meta";

export const metadata = meta({
  path: "/compare/",
  title: "phonedriver vs mobile-mcp, phone-harness and adb, measured",
  description: "Which tool should an AI agent use to control a phone? phonedriver against mobile-mcp, phone-harness and adb on tokens, passed runs, speed and telemetry, with raw data.",
  og: "/og/compare-adb.png",
});

export default function CompareIndex() {
  const b = board();
  const us = b.total("phonedriver").tokens;
  const read = op("tree_list");
  const figs: Record<string, [string, string]> = {
    adb: [ratio(read.adb / read.phonedriver), "faster screen reads"],
    "mobile-mcp": [ratio(b.total("mobile-mcp").tokens / us), "fewer tokens, nine jobs"],
    "phone-harness": [ratio(b.total("phone-harness").tokens / us), "fewer tokens, nine jobs"],
  };
  return (
    <>
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
        <h1>phonedriver compared with adb, mobile&#8209;mcp and phone&#8209;harness</h1>
        <p className="lede">
          On nine real phone jobs phonedriver passed {b.total("phonedriver").passed} of {b.total("phonedriver").runs} runs with {figs["mobile-mcp"][0]} fewer tokens than mobile&#8209;mcp and{" "}
          {figs["phone-harness"][0]} fewer than phone&#8209;harness, and it reads a screen {figs.adb[0]} faster than adb. Each page links its method and raw data.
        </p>
      </PageTop>
    </>
  );
}
