import { benchmarksMd } from "@/lib/bench";
import { markdown } from "@/lib/content";

export const dynamic = "force-static";

export const GET = () => markdown(benchmarksMd());
