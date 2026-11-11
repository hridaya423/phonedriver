import Link from "next/link";
import { displaySvg, METRICS, ORANGE, WIDTH } from "@/lib/brand";
import { REPO } from "@/lib/site";
import { FootMark } from "./FootMark";
import { Wordmark } from "./Wordmark";

const DISPLAY = displaySvg({ ink: "currentColor", accent: ORANGE, pitch: 12, animate: "vars", id: "fm" });
const VIEW = `0 ${METRICS.top} ${WIDTH} ${METRICS.height}`;

export function Footer() {
  return (
    <footer className="foot">
      <div className="wrap">
        <div className="fgrid">
          <div>
            <p className="fbrand">
              <Wordmark />
              <span className="vh">phonedriver</span>
            </p>
            <p className="small">Give your AI agent a phone. Free and open source under AGPL-3.0.</p>
          </div>
          <nav aria-label="Learn">
            <p className="lab">Learn</p>
            <Link href="/docs/install/">Install</Link>
            <Link href="/docs/">Docs</Link>
            <Link href="/benchmarks/">Benchmarks</Link>
            <Link href="/compare/">Compare</Link>
            <Link href="/blog/">Blog</Link>
          </nav>
          <nav aria-label="Project">
            <p className="lab">Project</p>
            <a href={REPO}>GitHub</a>
            <Link href="/changelog/">Changelog</Link>
            <a href={`${REPO}/discussions`}>Discussions</a>
            <a href={`${REPO}/blob/main/LICENSE`}>License: AGPL-3.0</a>
            <a href={`${REPO}/blob/main/docs/decisions.md`}>Design decisions</a>
            <a href="/llms.txt">llms.txt</a>
            <a href="/blog/feed.xml">RSS</a>
          </nav>
        </div>
        <p className="small legal">
          phonedriver sends <Link href="/docs/no-telemetry/">no telemetry</Link>. This site sets no cookies and loads no third-party scripts. The phonedriver lettering is drawn for this project. JetBrains Mono and Inter Display are
          under the SIL Open Font License 1.1.
        </p>
        <FootMark svg={DISPLAY} sweep={Math.round(WIDTH * 0.62)} view={VIEW} />
      </div>
    </footer>
  );
}
