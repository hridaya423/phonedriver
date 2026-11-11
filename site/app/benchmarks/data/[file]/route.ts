import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DATA_FILES } from "@/lib/bench";
import { ROOT } from "@/lib/site";

export const dynamic = "force-static";
export const dynamicParams = false;
export const generateStaticParams = () => Object.keys(DATA_FILES).map((file) => ({ file }));

export async function GET(_: Request, { params }: { params: Promise<{ file: string }> }) {
  const { file } = await params;
  const type = file.endsWith(".csv") ? "text/csv" : "application/json";
  return new Response(readFileSync(join(ROOT, DATA_FILES[file])), { headers: { "Content-Type": `${type}; charset=utf-8` } });
}
