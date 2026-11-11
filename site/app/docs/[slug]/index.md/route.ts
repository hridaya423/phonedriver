import { DOCS, markdown, page, pageMd } from "@/lib/content";

export const dynamic = "force-static";
export const dynamicParams = false;
export const generateStaticParams = () => DOCS.map((slug) => ({ slug }));

export async function GET(_: Request, { params }: { params: Promise<{ slug: string }> }) {
  return markdown(pageMd(page("docs", (await params).slug)));
}
