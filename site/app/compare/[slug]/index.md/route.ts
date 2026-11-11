import { COMPARE, markdown, page, pageMd } from "@/lib/content";

export const dynamic = "force-static";
export const dynamicParams = false;
export const generateStaticParams = () => COMPARE.map(([slug]) => ({ slug }));

export async function GET(_: Request, { params }: { params: Promise<{ slug: string }> }) {
  return markdown(pageMd(page("compare", (await params).slug)));
}
