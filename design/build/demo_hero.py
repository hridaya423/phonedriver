"""Builds the site hero from one run recorded by demo_record.py. Nothing is drawn or re-enacted: the phone picture is
the emulator's own screen recording and every line of the log is the text Claude Code logged for that run.
Outputs:
  <assets>/run.mp4         the phone, START to LAPSE at SPEED times real speed, then real time to END
  <assets>/run-poster.jpg  the frame one second after phonedriver asked
  <json>                   the log (prompt, the model's lines, each tool call and the head of its reply, the approval
                           question and the key presses that answered it), the logged tap coordinates, and the marks
The captions are the page's own words, not the log's. Times are seconds after the mark "enter". A tap's coordinates are the ones phonedriver replied with; its time inside
a batch is spread evenly between the call and its reply, because the log holds one time per call.
Usage: python3 demo_hero.py <run dir> <claude session .jsonl> <assets dir> <out .json>"""
import datetime
import json
import re
import subprocess
import sys
from pathlib import Path

RUN, SESSION, ASSETS, OUT = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4])
SPEED, LAPSE, END = 30, 385.5, 398.6
SIZE = (540, 1212)
FPS = 30
HEAD = 3
WIDTH = 150
CAPTIONS = [[0, "Reads the screen as text"], [153, "Picks the nearest store"], [221, "Fills the basket"], [323, "Guest checkout"]]
ASKS, SAYS_NO = "Holds the tap on Pay and asks the human", "The human says no. Nothing is charged."

times = json.loads((RUN / "times.json").read_text())
ENTER = times["marks"]["enter"]
ANSI = re.compile(r"\x1b\[[0-9;]*m")


def ff(*args):
    subprocess.run(["ffmpeg", "-v", "error", "-y", *map(str, args)], check=True)


def duration(p):
    return float(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(p)], capture_output=True, text=True, check=True).stdout)


def phone_offset():
    """Same rule as demo_compose.py: the notification shade was pulled open at the mark "flash"; the first frame that
    differs from frame 0 is that moment in the first file."""
    seg0 = times["segments"][0]
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", str(RUN / seg0["file"]), "-vf", "fps=20,scale=32:72,format=gray", "-f", "rawvideo", "-"], capture_output=True, check=True).stdout
    n = 32 * 72
    frames = [raw[i:i + n] for i in range(0, len(raw) - n + 1, n)]
    for i, fr in enumerate(frames):
        if sum(abs(a - b) for a, b in zip(fr, frames[0])) / n > 6:
            return times["marks"]["flash"] - i / 20 - 0.15 - seg0["start"]
    raise SystemExit("no shade flash found")


def video(ask):
    delta = phone_offset()
    inputs, chains, labels = [], [], []
    placed = ENTER
    segs = times["segments"]
    for i, seg in enumerate(segs):
        f = RUN / seg["file"]
        inputs += ["-i", f]
        start = seg["start"] + delta
        d = duration(f)
        hold = max(0.0, ENTER + END - (start + d)) + 1.0 if i == len(segs) - 1 else 0.0
        end = start + d + hold
        if end <= placed:
            continue
        trim, pad = max(0.0, placed - start), max(0.0, start - placed)
        chains.append(f"[{i}:v]fps={FPS},tpad=stop_duration={hold:.3f}:stop_mode=clone,trim=start={trim:.3f},setpts=PTS-STARTPTS,tpad=start_duration={pad:.3f}:start_mode=clone,scale={SIZE[0]}:{SIZE[1]}:flags=lanczos[s{i}]")
        labels.append(f"[s{i}]")
        placed = end
    fc = ";".join(chains) + ";" + "".join(labels) + f"concat=n={len(labels)}:v=1,split=2[a][b];"
    fc += f"[a]trim=start=0:end={LAPSE},setpts=(PTS-STARTPTS)/{SPEED},fps={FPS}[m];[b]trim=start={LAPSE}:end={END},setpts=PTS-STARTPTS,fps={FPS}[t];[m][t]concat=n=2:v=1[o]"
    out = ASSETS / "run.mp4"
    ff(*inputs, "-filter_complex", fc, "-map", "[o]", "-an", "-c:v", "libx264", "-crf", "27", "-preset", "slow", "-g", "15", "-pix_fmt", "yuv420p", "-movflags", "+faststart", out)
    ff("-ss", f"{LAPSE / SPEED + ask + 1 - LAPSE:.3f}", "-i", out, "-frames:v", "1", "-q:v", "3", ASSETS / "run-poster.jpg")
    return duration(out)


def clip(s):
    return s if len(s) <= WIDTH else s[: WIDTH - 1] + "…"


def log():
    rows, taps, looks, calls = [], [], [], {}
    for line in SESSION.read_text().splitlines():
        r = json.loads(line)
        if r.get("type") not in ("user", "assistant") or "timestamp" not in r:
            continue
        t = round(datetime.datetime.fromisoformat(r["timestamp"].replace("Z", "+00:00")).timestamp() - ENTER, 2)
        content = r["message"]["content"]
        if isinstance(content, str):
            rows.append({"t": 0, "k": "you", "text": content})
            continue
        for b in content:
            if b["type"] == "text":
                rows.append({"t": t, "k": "say", "text": b["text"]})
            elif b["type"] == "tool_use":
                name = b["name"].split("__")[-1]
                calls[b["id"]] = (t, name, b["input"])
                rows.append({"t": t, "k": "call", "name": name, "text": clip(json.dumps(b["input"], ensure_ascii=False, separators=(",", ":")))})
            elif b["type"] == "tool_result":
                c = b["content"]
                text = c if isinstance(c, str) else "\n".join(x["text"] if x["type"] == "text" else "[image]" for x in c)
                text = re.sub(r"\[Image: source: [^\]]*\]\s*", "", text).replace("[image]\n", "[image] ")
                lines = [l for l in text.split("\n") if l.strip()]
                row = {"t": t, "k": "reply", "lines": [clip(l) for l in lines[:HEAD]]}
                if len(lines) > HEAD:
                    row["more"] = len(lines) - HEAD
                if lines[0].startswith("The human declined"):
                    row["k"] = "declined"
                rows.append(row)
                t0, name, _ = calls[b["tool_use_id"]]
                if name in ("snapshot", "screenshot", "launch"):
                    looks.append([t0, t])
                steps = [l for l in lines if re.match(r"\d+\. ", l)] or lines[:1]
                for i, l in enumerate(steps):
                    m = re.search(r"tapped .*?at (\d+),(\d+)", l)
                    if not m:
                        continue
                    settle = re.search(r"settled in (\d+)ms", l)
                    at = t - int(settle.group(1)) / 1000 - 0.3 if len(steps) == 1 and settle else t0 + (t - t0) * (i + 0.5) / len(steps)
                    taps.append([round(max(t0 + 0.1, at), 2), int(m.group(1)), int(m.group(2))])
    return rows, taps, looks


def question():
    """The approval question as Claude Code drew it, and where its cursor was, read from the terminal captures."""
    term = [json.loads(l) for l in (RUN / "term.jsonl").read_text().splitlines()]
    lines, focus = None, []
    for e in term:
        screen = ANSI.sub("", e["screen"]).split("\n")
        if not any("requests your input" in l for l in screen):
            continue
        t = round(e["t"] - ENTER, 2)
        if lines is None:
            a = next(i for i, l in enumerate(screen) if "requests your input" in l)
            b = next(i for i, l in enumerate(screen) if "Your answer" in l)
            lines = [l[2:].rstrip() for l in screen[a:b] if l.strip()]
        accept = next(l for l in screen if "Accept" in l)
        where = "decline" if "❯ Decline" in accept else "accept" if "❯ Accept" in accept else "answer"
        if not focus or focus[-1][1] != where:
            focus.append([t, where])
    return lines, focus


def main():
    ASSETS.mkdir(parents=True, exist_ok=True)
    rows, taps, looks = log()
    lines, focus = question()
    declined = next(r["t"] for r in rows if r["k"] == "declined")
    rows.append({"t": focus[0][0], "k": "ask", "lines": lines})
    rows.sort(key=lambda r: r["t"])
    final = ANSI.sub("", (RUN / "final.txt").read_text())
    worked = re.search(r"Worked for [0-9a-z ]+", final).group(0).strip()
    data = {
        "speed": SPEED,
        "lapse": LAPSE,
        "end": END,
        "length": round(video(focus[0][0]), 3) if "--no-video" not in sys.argv else round(LAPSE / SPEED + END - LAPSE, 3),
        "screen": [411, 923],
        "calls": sum(r["k"] == "call" for r in rows),
        "worked": worked,
        "date": datetime.datetime.fromtimestamp(ENTER, datetime.timezone.utc).strftime("%-d %b %Y"),
        "chapters": CAPTIONS + [[focus[0][0], ASKS], [declined, SAYS_NO]],
        "focus": focus,
        "declined": declined,
        "rows": rows,
        "taps": taps,
        "looks": looks,
    }
    OUT.write_text(json.dumps(data, ensure_ascii=False, separators=(",", ":")) + "\n")
    print(f"{len(rows)} rows, {len(taps)} taps, {data['calls']} calls, {data['length']} s, {worked}")


main()
