import html
import re


def esc(s):
    return html.escape(s, quote=False)


def slug(s):
    s = re.sub(r"<[^>]+>", "", s)
    s = re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")
    return s


def inline(s, links=None):
    parts = re.split(r"(`[^`]+`)", s)
    out = []
    for p in parts:
        if p.startswith("`") and p.endswith("`") and len(p) > 1:
            out.append("<code>" + esc(p[1:-1]) + "</code>")
            continue
        p = esc(p)
        p = re.sub(
            r"\[([^\]]+)\]\(([^)\s]+)\)",
            lambda m: '<a href="%s">%s</a>' % (m.group(2), m.group(1)),
            p,
        )
        p = re.sub(r"(?<![\w*])\*([^*\s][^*]*?)\*(?![\w*])", r"<em>\1</em>", p)
        out.append(p)
    return "".join(out)


def screen(code):
    lines = []
    for ln in code.split("\n"):
        e = esc(ln)
        if ln.startswith("+ "):
            e = '<span class="add">%s</span>' % e
        elif ln.startswith("- "):
            e = '<span class="del">%s</span>' % e
        elif ln.startswith("~ "):
            e = '<span class="chg">%s</span>' % e
        elif ln.startswith("$ "):
            e = '<span class="cmd">%s</span>' % e
        lines.append(e)
    return "\n".join(lines)


def table(rows):
    def cells(r):
        r = r.strip()
        if r.startswith("|"):
            r = r[1:]
        if r.endswith("|"):
            r = r[:-1]
        return [c.strip() for c in r.split("|")]

    head = cells(rows[0])
    body = [cells(r) for r in rows[2:]]
    h = "<thead><tr>" + "".join("<th scope=\"col\">%s</th>" % inline(c) for c in head) + "</tr></thead>"
    b = "<tbody>"
    for r in body:
        b += "<tr>"
        for i, c in enumerate(r):
            tag = "th scope=\"row\"" if i == 0 else "td"
            end = "th" if i == 0 else "td"
            b += "<%s>%s</%s>" % (tag, inline(c), end)
        b += "</tr>"
    b += "</tbody>"
    return '<div class="tw"><table>%s%s</table></div>' % (h, b)


def render(md, toc=None):
    lines = md.split("\n")
    out = []
    i = 0
    para = []

    def flush():
        nonlocal para
        if para:
            out.append("<p>%s</p>" % inline(" ".join(para)))
            para = []

    while i < len(lines):
        ln = lines[i]
        if ln.startswith("```"):
            flush()
            lang = ln[3:].strip()
            buf = []
            i += 1
            while i < len(lines) and not lines[i].startswith("```"):
                buf.append(lines[i])
                i += 1
            i += 1
            code = "\n".join(buf)
            body = screen(code) if lang in ("screen", "console") else esc(code)
            out.append('<pre tabindex="0" class="lang-%s"><code>%s</code></pre>' % (lang or "text", body))
            continue
        if ln.startswith("<") and not ln.startswith("<br"):
            flush()
            buf = [ln]
            i += 1
            while i < len(lines) and lines[i].strip() != "":
                buf.append(lines[i])
                i += 1
            out.append("\n".join(buf))
            continue
        m = re.match(r"^(#{2,4}) (.+)$", ln)
        if m:
            flush()
            lvl = len(m.group(1))
            text = m.group(2)
            sid = slug(text)
            if toc is not None and lvl == 2:
                toc.append((sid, re.sub(r"`", "", text)))
            out.append('<h%d id="%s">%s</h%d>' % (lvl, sid, inline(text), lvl))
            i += 1
            continue
        if ln.startswith("|") and i + 1 < len(lines) and re.match(r"^\|?\s*:?-{2,}", lines[i + 1]):
            flush()
            rows = []
            while i < len(lines) and lines[i].startswith("|"):
                rows.append(lines[i])
                i += 1
            out.append(table(rows))
            continue
        if re.match(r"^(- |\d+\. )", ln):
            flush()
            ordered = ln[0].isdigit()
            items = []
            while i < len(lines) and re.match(r"^(- |\d+\. |   |  )", lines[i]) and lines[i].strip() != "":
                if re.match(r"^(- |\d+\. )", lines[i]):
                    items.append(re.sub(r"^(- |\d+\. )", "", lines[i]))
                else:
                    items[-1] += " " + lines[i].strip()
                i += 1
            tag = "ol" if ordered else "ul"
            out.append("<%s>%s</%s>" % (tag, "".join("<li>%s</li>" % inline(x) for x in items), tag))
            continue
        if ln.startswith("> "):
            flush()
            buf = []
            while i < len(lines) and lines[i].startswith("> "):
                buf.append(lines[i][2:])
                i += 1
            out.append("<blockquote><p>%s</p></blockquote>" % inline(" ".join(buf)))
            continue
        if ln.strip() == "":
            flush()
            i += 1
            continue
        para.append(ln.strip())
        i += 1
    flush()
    return "\n".join(out)


def front(text):
    meta = {}
    if text.startswith("---\n"):
        end = text.index("\n---\n", 4)
        for ln in text[4:end].split("\n"):
            if ":" in ln:
                k, v = ln.split(":", 1)
                meta[k.strip()] = v.strip()
        text = text[end + 5:]
    return meta, text
