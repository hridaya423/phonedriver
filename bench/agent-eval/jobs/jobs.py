#!/usr/bin/env python3
"""Seeds, independent checks and cleanup for the `jobs` and `jobs-mixed` sets in ../tasks.yaml.

Usage: jobs.py seed|check|clean <task>
Env: SERIAL (adb serial), ADB_SERVER_SOCKET (inherited), ANSWER_FILE and TRANSCRIPT_FILE (check only).
A check reads device state (private app state as root, content providers, dumpsys, appops, MediaStore, the
testbed event log) and, for the bug-report jobs, matches the agent's report against the planted-bug list in
BUGS (a bug counts as found when one line or sentence of the report matches every regex of its list, case-insensitive). Exit 0 is a pass; each failure reason is printed on its own line and the exit code is 1.
"""

import json
import os
import re
import shlex
import struct
import subprocess
import sys
import time
import zlib

CONTACTS = "content://com.android.contacts"
BASELINE = "/data/local/tmp/jobs-baseline"
TESTBED = "dev.phoneuse.testbed"
CHROME_PREFS = "/data/data/com.android.chrome/app_chrome/Default/Preferences"
CLOCK_DIR = "/data/user_de/0/com.google.android.deskclock"
BUGLE_DB = "/data/user/0/com.google.android.apps.messaging/databases/bugle_db"
SHELL_USE = r"phone-use__shell|\badb\b|\bshell\(|run-as|logcat|dumpsys|\bcmd\b|settings put"
THREADS = {
    "Alice Chen": "+15551110001",
    "Bob Park": "+15551110002",
    "Carol Diaz": "+15551110003",
}

BUGS = {
    "qa-signup": {
        "welcome-drops-last-char-of-email": [
            r"e-?mail|address|welcome",
            r"last (char|letter)|truncat|cut off|(missing|drops?|dropped|lacks?|omits?) (the )?(last|final)|without the (last|final)|one (char|letter)",
        ],
        "create-button-dead-after-an-error": [
            r"create|button|submit|sign.?up|tap",
            r"nothing|no (response|effect|reaction|feedback)|ignor|unrespons|not respond|doesn.t (work|respond)",
            r"second|2nd|again|retry|resubmit|after|re-?tap|follow",
        ],
    },
    "qa-shop": {
        "total-stale-after-decrease": [
            r"total",
            r"decreas|minus|reduc|lower|fewer|down|remov",
            r"not (update|chang|recalc|refresh)|stale|unchang|still|wrong|incorrect|stuck|doesn.t|same",
        ],
        "save10-takes-a-flat-10-off": [
            r"promo|code|save10|discount",
            r"flat|\$\s?10|10\.00|10 dollars|instead of|not 10 ?%|rather than",
            r"wrong|incorrect|bug|should|instead|rather|but|flat",
        ],
    },
}


def sh(cmd):
    out = subprocess.run(
        ["adb", "-s", os.environ["SERIAL"], "shell", cmd],
        capture_output=True,
        text=True,
        timeout=90,
    )
    return out.stdout + out.stderr


def su(cmd):
    return sh(f"su 0 sh -c {shlex.quote(cmd)}")


def adb(*argv):
    return subprocess.run(
        ["adb", "-s", os.environ["SERIAL"], *argv], capture_output=True, text=True, timeout=90
    )


def rows(text):
    out = []
    for line in text.splitlines():
        m = re.match(r"Row: \d+ (.*)$", line)
        if m:
            parts = re.split(r", (?=[A-Za-z_][A-Za-z0-9_]*=)", m.group(1))
            out.append(dict(p.split("=", 1) for p in parts if "=" in p))
    return out


def query(uri, projection, where=None):
    cmd = f"content query --uri {uri} --projection {projection}"
    if where:
        cmd += f" --where {shlex.quote(where)}"
    return rows(sh(cmd))


def digits(s):
    d = re.sub(r"\D", "", s)
    return d[1:] if len(d) == 11 and d[0] == "1" else d


def answer():
    path = os.environ.get("ANSWER_FILE")
    return open(path).read() if path else ""


def tool_text(transcript):
    out = []
    for line in transcript.splitlines():
        try:
            v = json.loads(line)
        except ValueError:
            continue
        if v.get("type") != "assistant":
            continue
        for b in v["message"]["content"]:
            if b.get("type") == "tool_use":
                out.append(b["name"] + " " + json.dumps(b.get("input", {})))
    return out


def shell_used():
    path = os.environ.get("TRANSCRIPT_FILE")
    if not path:
        return None
    return next((t[:160] for t in tool_text(open(path).read()) if re.search(SHELL_USE, t)), None)


def log_events():
    out = []
    for line in sh("logcat -d -s PUTB:I").splitlines():
        m = re.search(r"PUTB\s*:\s*(\{.*\})\s*$", line)
        if m:
            try:
                out.append(json.loads(m.group(1)))
            except ValueError:
                pass
    return out


def found_bugs(report, bugs):
    chunks = [c for c in re.split(r"[\n.!?;]+", report) if c.strip()]
    return {
        name: any(all(re.search(p, c, re.I) for p in pats) for c in chunks)
        for name, pats in bugs.items()
    }


def has_steps(report):
    return len(re.findall(r"^\s*(?:\d+[.)]|[-*•])\s+\S", report, re.M)) >= 3


def check_report(task):
    report, bad = answer(), []
    for name, hit in found_bugs(report, BUGS[task]).items():
        if not hit:
            bad.append(f"planted bug not reported: {name}")
    if not has_steps(report):
        bad.append("the report has no steps (a numbered or bulleted list of at least 3 lines)")
    return bad


def check_qa_signup():
    bad = check_report("qa-signup")
    taps = [e for e in log_events() if e.get("ev") == "signup_tap"]
    if not any(not e["accepted"] and "error" in e for e in taps):
        bad.append("log: the sign-up form was never submitted invalid, so the error path was not tested")
    if not any(e.get("ignored") for e in taps):
        bad.append("log: no tap after a rejected submit, the dead-button bug was not exercised")
    if not any(e["accepted"] for e in taps):
        bad.append("log: no successful sign-up was reached")
    return bad


def check_qa_shop():
    bad = check_report("qa-shop")
    ev = log_events()
    qty = [e["qty"] for e in ev if e.get("ev") == "shop_qty"]
    if not any(b < a for a, b in zip(qty, qty[1:])):
        bad.append("log: the quantity was never decreased")
    if not any(e.get("ev") == "shop_promo" and e["code"].upper() == "SAVE10" for e in ev):
        bad.append("log: SAVE10 was never applied")
    return bad


def relay_code():
    return sh(f"run-as {TESTBED} cat files/relay_code").strip()


def contact_phones(name):
    raw = query(f"{CONTACTS}/raw_contacts", "_id:display_name:deleted", f"display_name='{name}' AND deleted=0")
    phones = []
    for r in raw:
        for d in query(f"{CONTACTS}/data", "mimetype:data1", f"raw_contact_id={r['_id']}"):
            if d["mimetype"].endswith("/phone_v2"):
                phones.append(digits(d["data1"]))
    return raw, phones


def check_locker(name, forbid_shell):
    code, bad = relay_code(), []
    raw, phones = contact_phones(name)
    if not re.fullmatch(r"\d{7}", code):
        return [f"cannot read the pickup code from the app (got {code!r})"]
    if len(raw) != 1:
        bad.append(f"{len(raw)} contacts named {name}, wanted 1")
    elif phones != [code]:
        bad.append(f"{name} has phones {phones}, the pickup code is {code}")
    used = shell_used() if forbid_shell else None
    if used:
        bad.append(f"the agent used the shell, the code is on screen: {used}")
    return bad


def check_chrome_prefs():
    sh("input keyevent KEYCODE_HOME")
    time.sleep(5)
    prefs = json.loads(su(f"cat {CHROME_PREFS}"))
    dse = prefs.get("default_search_provider_data", {}).get("template_url_data", {})
    bad = []
    if "duckduckgo" not in json.dumps(dse).lower():
        bad.append(f"default search engine is {dse.get('short_name')!r}, wanted DuckDuckGo")
    if prefs.get("https_only_mode_enabled") is not True:
        bad.append(f"https_only_mode_enabled is {prefs.get('https_only_mode_enabled')!r}")
    return bad


def pref_minutes(xml, key_pattern):
    m = re.search(rf'name="[^"]*(?:{key_pattern})[^"]*"[^>]*?(?:value="|>)(\d+)', xml, re.I)
    if not m:
        return None
    v = int(m.group(1))
    return v if v <= 60 else v // 60 if v <= 3600 else v // 60000


def check_clock_settings():
    files = su(f"cat {CLOCK_DIR}/shared_prefs/*.xml")
    bad = []
    tokyo = su(f"grep -rIlE 'Tokyo' {CLOCK_DIR}").strip()
    london = su(f"grep -rIlE 'London' {CLOCK_DIR}").strip()
    if not tokyo:
        bad.append("no Tokyo world clock found in the Clock app's files")
    if not london:
        bad.append("no London world clock found in the Clock app's files")
    snooze, silence = pref_minutes(files, "snooze"), pref_minutes(files, "silence")
    if snooze != 15:
        bad.append(f"snooze length is {snooze} minutes, wanted 15")
    if silence != 5:
        bad.append(f"silence after is {silence} minutes, wanted 5")
    return bad


def conversations():
    out = su(f'sqlite3 -header -separator "|" {BUGLE_DB} "select * from conversations"').splitlines()
    if len(out) < 2:
        return []
    cols = out[0].split("|")
    return [dict(zip(cols, line.split("|"))) for line in out[1:]]


def flag(row, word):
    return any(
        word in k.lower() and v not in ("", "0", "NULL", "null") for k, v in row.items()
    )


def check_messages():
    rows_ = conversations()
    bad = []
    for name, number in THREADS.items():
        mine = [r for r in rows_ if any(name in v or number[-10:] in v for v in r.values())]
        if len(mine) != 1:
            bad.append(f"{len(mine)} conversations for {name}")
            continue
        pinned, archived = flag(mine[0], "pin"), flag(mine[0], "archiv")
        want = {"Alice Chen": (False, False), "Bob Park": (True, False), "Carol Diaz": (False, True)}[name]
        if (pinned, archived) != want:
            bad.append(f"{name}: pinned={pinned} archived={archived}, wanted pinned={want[0]} archived={want[1]}")
    return bad


def media_images():
    return query(
        "content://media/external/images/media",
        "_display_name:width:height:date_added",
    )


def check_photos():
    since = int(sh(f"cat {BASELINE}-photos").strip() or 0)
    imgs, bad = media_images(), []
    if not any(i["_display_name"] == "jobs-harbor.png" and i["width"] == "1200" for i in imgs):
        bad.append("the original jobs-harbor.png is gone or changed")
    copies = [
        i for i in imgs
        if i["_display_name"] != "jobs-harbor.png" and int(i["date_added"]) >= since
    ]
    if not any(abs(int(i["width"]) - int(i["height"])) <= 1 and 0 < int(i["width"]) <= 800 for i in copies):
        bad.append(f"no new square copy no larger than 800 px; new images {[(i['_display_name'], i['width'], i['height']) for i in copies]}")
    return bad


def appops_ignored():
    return set(sh("cmd appops query-op RUN_ANY_IN_BACKGROUND ignore").split())


def check_wakelock():
    base = set(sh(f"cat {BASELINE}-ignored").split())
    now = appops_ignored() - base
    bad = []
    if TESTBED not in now:
        bad.append(f"{TESTBED} (holds the jobs:drain wakelock) is not restricted")
    others = now - {TESTBED}
    if others:
        bad.append(f"other apps were restricted: {sorted(others)}")
    return bad


CHECKS = {
    "qa-signup": check_qa_signup,
    "qa-shop": check_qa_shop,
    "relay-code": lambda: check_locker("Locker 7", True),
    "chrome-prefs": check_chrome_prefs,
    "clock-settings": check_clock_settings,
    "messages-pin-archive": check_messages,
    "photos-crop": check_photos,
    "wakelock-restrict": check_wakelock,
    "log-code": lambda: check_locker("Locker 9", False),
}


def insert_contact(name, phone):
    sh(f"content insert --uri {CONTACTS}/raw_contacts --bind account_type:n: --bind account_name:n:")
    rid = max(int(r["_id"]) for r in query(f"{CONTACTS}/raw_contacts", "_id"))
    given, family = name.split(" ", 1)

    def data(**kv):
        binds = " ".join(f"--bind {k}:s:{shlex.quote(v)}" for k, v in kv.items())
        sh(f"content insert --uri {CONTACTS}/data --bind raw_contact_id:i:{rid} {binds}")

    data(mimetype="vnd.android.cursor.item/name", data1=name, data2=given, data3=family)
    data(mimetype="vnd.android.cursor.item/phone_v2", data1=phone, data2="2")


def delete_contacts(*names):
    for n in names:
        where = shlex.quote(f"display_name='{n}'")
        sh(f"content delete --uri '{CONTACTS}/raw_contacts?caller_is_syncadapter=true' --where {where}")


def png(width, height):
    def chunk(tag, body):
        c = struct.pack(">I", len(body)) + tag + body
        return c + struct.pack(">I", zlib.crc32(tag + body))

    raw = b"".join(
        b"\x00" + bytes(
            v for x in range(width)
            for v in ((x * 255 // width), (y * 255 // height), 128 if (x // 100 + y // 100) % 2 else 40)
        )
        for y in range(height)
    )
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )


def launch_testbed(tab):
    sh(f"am force-stop {TESTBED}; am start -n {TESTBED}/.MainActivity --es tab {tab}")
    time.sleep(2)


def seed(task):
    if task in ("relay-code", "log-code"):
        delete_contacts("Locker 7" if task == "relay-code" else "Locker 9")
        sh(f"am force-stop {TESTBED}; run-as {TESTBED} rm -f files/relay_code")
        if task == "log-code":
            sh("logcat -c")
            launch_testbed("relaylog")
        sh("input keyevent KEYCODE_HOME")
    elif task == "messages-pin-archive":
        delete_contacts(*THREADS)
        for name, number in THREADS.items():
            insert_contact(name, number)
        for i, (name, number) in enumerate(THREADS.items()):
            adb("emu", "sms", "send", number, f"Hi from {name.split()[0]}, message {i + 1}")
    elif task == "photos-crop":
        path = "/tmp/jobs-harbor.png"
        open(path, "wb").write(png(1200, 800))
        adb("push", path, "/sdcard/Pictures/jobs-harbor.png")
        sh("am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file:///sdcard/Pictures/jobs-harbor.png")
        sh(f"date +%s > {BASELINE}-photos")
    elif task == "wakelock-restrict":
        sh(f"cmd appops reset {TESTBED}")
        sh(f"cmd appops query-op RUN_ANY_IN_BACKGROUND ignore > {BASELINE}-ignored")
        launch_testbed("drain")
        sh("input keyevent KEYCODE_HOME")
    elif task in ("qa-signup", "qa-shop"):
        sh("logcat -c")
        launch_testbed("signup" if task == "qa-signup" else "shop")
    elif task == "chrome-prefs":
        sh("pm clear com.android.chrome")
    elif task == "clock-settings":
        sh("pm clear com.google.android.deskclock")


def clean(task):
    if task in ("relay-code", "log-code"):
        delete_contacts("Locker 7" if task == "relay-code" else "Locker 9")
        sh(f"am force-stop {TESTBED}; run-as {TESTBED} rm -f files/relay_code")
    elif task == "messages-pin-archive":
        delete_contacts(*THREADS)
        sh("content delete --uri content://sms")
    elif task == "photos-crop":
        sh("rm -f /sdcard/Pictures/jobs-*.png /sdcard/Pictures/*Edited* /sdcard/Pictures/*-edited*")
        sh(f"rm -f {BASELINE}-photos")
    elif task == "wakelock-restrict":
        sh(f"cmd appops reset {TESTBED}; am force-stop {TESTBED}; rm -f {BASELINE}-ignored")
    elif task in ("qa-signup", "qa-shop"):
        sh(f"am force-stop {TESTBED}")
    elif task == "chrome-prefs":
        sh("pm clear com.android.chrome")
    elif task == "clock-settings":
        sh("pm clear com.google.android.deskclock")


def main(argv):
    verb, task = argv
    if verb == "check":
        bad = CHECKS[task]()
        print("\n".join(bad))
        return 1 if bad else 0
    {"seed": seed, "clean": clean}[verb](task)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
