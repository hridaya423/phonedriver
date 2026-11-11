import { copyFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const gh = join(import.meta.dirname, "../../.github/assets");
const dst = join(import.meta.dirname, "../public/assets");
for (const name of ["demo.mp4"]) {
  if (existsSync(join(gh, name))) copyFileSync(join(gh, name), join(dst, name));
  else if (!existsSync(join(dst, name))) throw new Error(`missing ${name}`);
}
