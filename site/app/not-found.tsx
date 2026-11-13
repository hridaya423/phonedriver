import Link from "next/link";
import type { Metadata } from "next";
import { PageTop } from "@/components/PageTop";
import { HaltEngine } from "@/components/Engines";

export const metadata: Metadata = { title: { absolute: "Page not found | phonedriver" }, description: "This page does not exist.", robots: { index: false } };

export default function NotFound() {
  return (
    <PageTop
      say="Open this page."
      mark={
        <>
          <canvas className="pt-halt" aria-hidden="true" />
          <HaltEngine />
        </>
      }
    >
      <p className="lab">404 · no element matched</p>
      <h1>Nothing on this screen.</h1>
      <p className="lede">
        Try the <Link href="/docs/">docs</Link>, the <Link href="/benchmarks/">benchmarks</Link> or the <Link href="/">home page</Link>.
      </p>
    </PageTop>
  );
}
