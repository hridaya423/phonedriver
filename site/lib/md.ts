export const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

export const slug = (s: string) =>
  s
    .replace(/<[^>]+>/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

export function inline(s: string): string {
  return s
    .split(/(`[^`]+`)/)
    .map((p) => {
      if (p.length > 1 && p.startsWith("`") && p.endsWith("`")) return `<code>${esc(p.slice(1, -1))}</code>`;
      return esc(p)
        .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, '<a href="$2">$1</a>')
        .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
        .replace(/(?<![\w*])\*([^*\s][^*]*?)\*(?![\w*])/g, "<em>$1</em>");
    })
    .join("");
}

function screen(code: string) {
  const cls: Record<string, string> = { "+ ": "add", "- ": "del", "~ ": "chg", "$ ": "cmd" };
  return code
    .split("\n")
    .map((ln) => {
      const c = cls[ln.slice(0, 2)];
      return c ? `<span class="${c}">${esc(ln)}</span>` : esc(ln);
    })
    .join("\n");
}

function table(rows: string[]) {
  const cells = (r: string) => r.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((c) => c.trim());
  const head = `<thead><tr>${cells(rows[0]).map((c) => `<th scope="col">${inline(c)}</th>`).join("")}</tr></thead>`;
  const body = rows
    .slice(2)
    .map((r) => `<tr>${cells(r).map((c, i) => (i ? `<td>${inline(c)}</td>` : `<th scope="row">${inline(c)}</th>`)).join("")}</tr>`)
    .join("");
  return `<div class="tw"><table>${head}<tbody>${body}</tbody></table></div>`;
}

export type Toc = { id: string; text: string }[];

export function render(md: string, toc?: Toc): string {
  const lines = md.split("\n");
  const out: string[] = [];
  let para: string[] = [];
  const flush = () => {
    if (para.length) out.push(`<p>${inline(para.join(" "))}</p>`);
    para = [];
  };
  let i = 0;
  while (i < lines.length) {
    const ln = lines[i];
    if (ln.startsWith("```")) {
      flush();
      const lang = ln.slice(3).trim();
      const buf: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith("```")) buf.push(lines[i++]);
      i++;
      const code = buf.join("\n");
      const body = lang === "screen" || lang === "console" ? screen(code) : esc(code);
      const copy = code.split("\n").filter((l) => l.startsWith("$ ")).map((l) => l.slice(2)).join("\n") || code;
      out.push(`<div class="code"><pre tabindex="0" class="lang-${lang || "text"}"><code>${body}</code></pre><button type="button" class="cp" data-copy="${esc(copy).replace(/"/g, "&quot;")}">Copy</button></div>`);
      continue;
    }
    if (ln.startsWith("<") && !ln.startsWith("<br")) {
      flush();
      const buf = [ln];
      i++;
      while (i < lines.length && lines[i].trim() !== "") buf.push(lines[i++]);
      out.push(buf.join("\n"));
      continue;
    }
    const h = /^(#{2,4}) (.+)$/.exec(ln);
    if (h) {
      flush();
      const lvl = h[1].length;
      const id = slug(h[2]);
      if (toc && lvl === 2) toc.push({ id, text: h[2].replace(/`/g, "") });
      out.push(`<h${lvl} id="${id}">${inline(h[2])}</h${lvl}>`);
      i++;
      continue;
    }
    if (ln.startsWith("|") && i + 1 < lines.length && /^\|?\s*:?-{2,}/.test(lines[i + 1])) {
      flush();
      const rows: string[] = [];
      while (i < lines.length && lines[i].startsWith("|")) rows.push(lines[i++]);
      out.push(table(rows));
      continue;
    }
    if (/^(- |\d+\. )/.test(ln)) {
      flush();
      const tag = /^\d/.test(ln) ? "ol" : "ul";
      const items: string[] = [];
      while (i < lines.length && /^(- |\d+\. |   |  )/.test(lines[i]) && lines[i].trim() !== "") {
        if (/^(- |\d+\. )/.test(lines[i])) items.push(lines[i].replace(/^(- |\d+\. )/, ""));
        else items[items.length - 1] += " " + lines[i].trim();
        i++;
      }
      out.push(`<${tag}>${items.map((x) => `<li>${inline(x)}</li>`).join("")}</${tag}>`);
      continue;
    }
    if (ln.startsWith("> ")) {
      flush();
      const buf: string[] = [];
      while (i < lines.length && lines[i].startsWith("> ")) buf.push(lines[i++].slice(2));
      out.push(`<blockquote><p>${inline(buf.join(" "))}</p></blockquote>`);
      continue;
    }
    if (ln.trim() === "") {
      flush();
      i++;
      continue;
    }
    para.push(ln.trim());
    i++;
  }
  flush();
  return out.join("\n");
}

export function front(text: string): { meta: Record<string, string>; body: string } {
  const meta: Record<string, string> = {};
  if (!text.startsWith("---\n")) return { meta, body: text };
  const end = text.indexOf("\n---\n", 4);
  for (const ln of text.slice(4, end).split("\n")) {
    const k = ln.indexOf(":");
    if (k > -1) meta[ln.slice(0, k).trim()] = ln.slice(k + 1).trim().replace(/^"(.*)"$/, "$1");
  }
  return { meta, body: text.slice(end + 5) };
}

export const plain = (s: string) => s.replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1").replace(/`/g, "");

/** Keeps hyphenated names such as phone-harness on one line. */
export const unbroken = (s: string): string => s.replace(/(\w)-(?=\w)/g, "$1\u2011");
