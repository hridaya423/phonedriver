import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

DEFINITION = "phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones. It shows a screen as numbered text, and every tap, type or swipe returns only the lines that changed."

HOME_DESC = "phonedriver lets an AI agent see and operate an Android or iOS phone: tap, type, swipe, with an MCP server for Claude Code and Cursor. Up to 98x faster than adb."

LLMS_NOTE = "phonedriver is a free, open-source (AGPL-3.0) CLI and MCP server. It runs on macOS, Linux and Windows and controls Android emulators and phones, the iOS Simulator and iPhones. It sends no telemetry. Numbers below come from benchmarks on an Android 17 emulator; each page links its method and raw data."

MCP_JSON = """{
  "mcpServers": {
    "phonedriver": {
      "command": "npx",
      "args": ["-y", "phonedriver", "mcp"],
      "env": { "PHONEDRIVER_DEVICE": "android:emulator-5554" }
    }
  }
}"""

_svg = ROOT / ".github/assets/hero-light.svg"
HERO_ALT = "Bar chart of phonedriver against adb at p50 on an Android emulator, up to 98 times faster."
HERO_H = 832
if _svg.exists():
    _head = _svg.read_text()[:3000]
    m = re.search(r'aria-label="([^"]+)"', _head)
    if m:
        HERO_ALT = m.group(1)
    m = re.search(r'viewBox="0 0 \d+ (\d+)"', _head)
    if m:
        HERO_H = int(m.group(1))

FAQ = [
    ("How do I let an AI agent control my phone?",
     "Install phonedriver, add it to your agent as an MCP server (`claude mcp add phonedriver -- npx -y phonedriver mcp` for Claude Code), connect an Android phone with USB debugging, start an emulator, or boot the iOS Simulator, and ask the agent in plain language, for example \"Open Settings and turn on dark mode.\" The agent gets a text view of the screen and tools to tap, type and swipe. See the [setup guides](/docs/)."),
    ("How do I let Claude control my Android phone?",
     "Add phonedriver to Claude Code as an MCP server with `claude mcp add phonedriver -- npx -y phonedriver mcp`, then connect a phone with USB debugging on or start an emulator. Nothing is installed on the phone. The device suite passes on emulators (API 26 and 37); a phone uses the same path and is not tested on hardware. See [the Claude Code guide](/docs/claude-code/)."),
    ("What is phonedriver?",
     "phonedriver is a free, open-source CLI and MCP server that gives an AI agent a text view of an Android or iOS screen and the means to tap, type and swipe. Every action waits for the screen to settle and returns only the lines that changed, so the agent does not poll or sleep. It is licensed AGPL-3.0."),
    ("Which AI agents and MCP clients work with it?",
     "Any client that starts a stdio MCP server can use it, including Claude Code and Cursor. Claude Code is the one we test against, with `claude -p` and Claude Sonnet 5.5 in our benchmarks. We have not run a Cursor session ourselves. The server has fourteen tools, and the CLI prints the same text as the MCP tools."),
    ("Does it work on a real iPhone and a real Android phone?",
     "Yes for iPhone and iPad through the CLI and MCP, tested on an iPhone 14 Plus (iOS 26.1) and an iPad Pro (iOS 27.0); it needs a Mac with Xcode and a signing team. A real Android phone uses the same path as the emulator, over USB or Wi-Fi debugging, but we have not tested it on hardware yet. Android emulators (API 26 and 37) and the iOS Simulator are tested."),
    ("What is the fastest way for an AI agent to control a phone?",
     "On an Android 17 emulator, phonedriver reads a 1,000-row list in 24.0 ms where adb takes 2,352.4 ms (98 times faster at p50, 95% CI 92.2 to 106.0), takes a screenshot in 11.6 ms against 476.3 ms (41 times) and types 100 characters in 251.1 ms against 4,840.9 ms (19 times). A bare tap is closer: 1.1 to 1.7 times faster host to reply. Emulator only; no physical phone has been measured. See [the benchmarks](/benchmarks/)."),
    ("How is phonedriver different from mobile-mcp and phone-harness?",
     "mobile-mcp is an MCP server that reads the accessibility tree and also supports cloud devices, with anonymous telemetry on by default. phone-harness is a Python CLI that drives Android through adb and iPhones through Mac mirroring and OCR. phonedriver keeps a resident agent on the Android device, waits for the screen to settle after every action and returns a diff, asks before taps that spend money, and sends no telemetry. See the [comparisons](/compare/)."),
    ("Can the agent spend my money or delete things?",
     "Not without asking you. Before a tap on a button such as `Place order`, `Pay`, `Send $20.00` or `Delete account`, phonedriver stops and asks the human through the MCP client, showing the app, the button and the amounts on screen. This stops an agent that follows its rules or is steered by screen text; it does not stop one set on bypassing it, and it matches English labels only. See [the approval guard](/docs/approval-guard/)."),
    ("Does phonedriver send telemetry?",
     "No. It talks to the device, the local daemon socket and the local adb server, and to Apple only when mounting an iOS developer disk image. A test runs the CLI, daemon and MCP server under a macOS sandbox that kills any process opening a non-local connection. Text passed with `--secret` never reaches logs, traces or replies. This website sets no cookies and runs no scripts."),
    ("Does it install anything on my phone?",
     "On Android, no app and no accessibility service. The daemon pushes a small jar over adb and starts it with `app_process` as the shell user, the way scrcpy does. On iOS, a small XCTest runner is built on your Mac with Xcode the first time it is needed."),
    ("How many tokens does it use?",
     "Its 14 MCP tools cost about 5,600 tokens on the first turn of a one-word `claude -p`, against about 5,200 for phone-harness. On three Android tasks, Claude Sonnet 5.5 used 218,357 tokens in total with phonedriver and 330,375 with phone-harness (medians of 2 to 3 runs, 34% fewer), all passing. Small samples: see [the token benchmark](/benchmarks/#tokens)."),
    ("What does it cost, and what is the license?",
     "It is free. The license is AGPL-3.0-only: if you distribute phonedriver, or run a modified version as a network service, you must publish your source under the same license. The Android agent contains files copied from scrcpy under Apache-2.0."),
    ("What does it not do well yet?",
     "A bare tap misses its 15 ms budget. An iOS tap takes 357 ms on the Simulator and 454 to 534 ms on devices. WebViews and custom views can have an empty accessibility tree, where it falls back to OCR. The real-phone jobs benchmark ran 2 passes per job on one tablet emulator, so its medians describe what happened and are not rates. Android hardware is untested."),
]

COMPARE_TABLE = [
    ["", "phonedriver", "adb", "mobile-mcp", "phone-harness"],
    ["Platforms", "Android, iOS", "Android", "Android, iOS, cloud devices", "Android over adb, iPhone through Mac mirroring"],
    ["How it reads the screen", "Pruned text tree with refs; OCR when the tree is empty", "Raw XML (`uiautomator dump`) and PNG", "Accessibility tree; screenshots when needed", "Android: adb screenshots and the tree. iPhone: OCR of the mirrored window"],
    ["Waits for the screen after an action", "Yes, returns what changed", "No", "Not documented", "Fixed sleeps and a stable-screen check"],
    ["Speed: 1,000-row list read (p50)", "24.0 ms", "2,352.4 ms", "Not measured", "2,653.7 ms"],
    ["Asks before taps that spend money", "Yes", "No", "Not documented", "Not documented"],
    ["Telemetry", "None", "None", "Anonymous, on by default; opt out with `MOBILEMCP_DISABLE_TELEMETRY=1`", "Sends script and output to PostHog by default in 0.3.3 (source read); opt out with `DO_NOT_TRACK=1`"],
    ["Interface", "MCP server and CLI", "CLI", "MCP server", "CLI that runs Python scripts"],
    ["License", "AGPL-3.0", "Apache-2.0", "Apache-2.0", "MIT"],
]

JOBS = [
    ("qa-signup", "Test the Signup tab of a testbed app and report every bug with steps", "The bugs exist only in how the screen behaves under use; no command exercises a form's validation path"),
    ("qa-shop", "Test the Shop tab (quantity, totals, a discount code) and report bugs", "Same: a stale total and a flat discount show only under use"),
    ("relay-code", "Copy a random pickup code from the Relay tab into a new contact \"Locker 7\"", "The code exists only in a TextView and the app's private storage"),
    ("chrome-prefs", "Make DuckDuckGo the default search engine and turn on \"Always use secure connections\"", "Chrome keeps both in a private Preferences file; no intent or setting reaches them"),
    ("clock-settings", "Add Tokyo and London world clocks, set snooze to 15 min and silence after 5 min", "These live in Clock's private preferences; the alarm intents set alarms and timers only"),
    ("messages-pin-archive", "Pin one conversation, archive another, leave a third", "Pin and archive are private Messages flags with no command"),
    ("photos-crop", "Crop a photo to a square and save a copy, keeping the original", "The Photos editor is the only crop tool; a shell has no image editor"),
    ("wakelock-restrict (mixed)", "Find the app holding a wakelock and set its battery usage to Restricted", "A shell finds the holder and Settings sets the restriction; any route allowed"),
    ("log-code (mixed)", "Find a pickup code in the device log and put it in a contact \"Locker 9\"", "The code is only in logcat; the contact is made in the Contacts UI; any route allowed"),
]

DEMO_CAPTION = "A real Claude Code session driving an Android emulator through phonedriver, sped up 10x: the run took 6 minutes 7 seconds, and the last 3 seconds play at real speed. Claude Code is asked to order butter chicken and garlic naan on Uber Eats. It adds both items and reaches checkout, then stops at Uber's sign-in page without entering anything and asks the user to sign in and approve the order. Signed out, Uber Eats asks for a sign-in at checkout, so nothing was ordered."
