import Link from "next/link";
import type { Metadata } from "next";
import { PageTop } from "@/components/PageTop";
import { HAND, hand } from "@/lib/chauffeur";

export const metadata: Metadata = { title: { absolute: "Page not found | phonedriver" }, description: "This page does not exist.", robots: { index: false } };

const PALM = hand({ line: "#0b0b0c", fill: "#fff", accent: "#ff5b14" }, "open");

export default function NotFound() {
  return (
    <PageTop
      say="Open this page."
      mark={<svg className="pt-palm" viewBox={`${HAND.x} ${HAND.y} ${HAND.w} ${HAND.h}`} aria-hidden="true" dangerouslySetInnerHTML={{ __html: PALM }} />}
    >
      <p className="lab">404 · no element matched</p>
      <h1>Nothing on this screen.</h1>
      <p className="lede">
        Try the <Link href="/docs/">docs</Link>, the <Link href="/benchmarks/">benchmarks</Link> or the <Link href="/">home page</Link>.
      </p>
    </PageTop>
  );
}
