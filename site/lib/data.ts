export const DEFINITION = "phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones. It shows a screen as numbered text, and every tap, type or swipe returns only the lines that changed.";

export const HOME_DESC = "phonedriver lets Claude Code, Codex, Cursor or any MCP client use an Android phone or an iPhone. It reads the screen as text and asks you before it pays.";

export const LLMS_NOTE = "phonedriver is a free, open-source (AGPL-3.0) CLI and MCP server. It runs on macOS, Linux and Windows and controls Android emulators and phones, the iOS Simulator and iPhones. It sends no telemetry. Numbers below come from benchmarks on an Android 17 emulator and a HONOR X8 phone; each page links its method and raw data.";

export const FAQ = [
  [
    "How do I let Claude control my Android phone?",
    "Run `claude mcp add phonedriver -- npx -y phonedriver mcp`, turn on USB debugging, and plug the phone in."
  ],
  [
    "Does it work with an iPhone?",
    "Yes, from a Mac with Xcode, on the Simulator or on an iPhone or iPad signed with your Apple developer team."
  ],
  [
    "Can my agent buy things with it?",
    "Not without your answer, unless you have told it to run autonomously. See [Asking before it pays](#asking-before-it-pays)."
  ],
  [
    "Can I use it at work?",
    "Yes. It is AGPL-3.0: you can use it freely. If you change it and offer it to others as a network service, you must publish your changes."
  ]
] as [string, string][];

export const FAQ_MORE = [
  [
    "What is the best MCP server for controlling a phone?",
    "On my benchmark of nine real phone jobs (Claude Sonnet 5.5, one Android emulator, 10 November 2026), phonedriver passed 27 of 27 runs with 657,542 tokens. mobile-mcp passed 17 of 18 with 4,567,200 tokens, 6.9 times as many, and phone-harness passed 14 of 18 with 1,127,495. phonedriver also reads a screen 170 times faster than adb. I wrote the jobs, so read [the method and raw data](/benchmarks/) before you trust the ranking."
  ],
  [
    "Is there an alternative to mobile-mcp?",
    "phonedriver is one. Both are MCP servers for Android and iOS. On nine real phone jobs mobile-mcp used 6.9 times the tokens of phonedriver, and phonedriver adds a diff after every action, an approval prompt before a tap that spends money, and no telemetry, where mobile-mcp sends anonymous usage data by default. mobile-mcp supports cloud devices and phonedriver does not. See [phonedriver vs mobile-mcp](/compare/mobile-mcp/)."
  ],
  [
    "Is phonedriver an Appium alternative for AI agents?",
    "For an agent that decides each step from the screen, yes. Appium is a WebDriver server built for scripted tests: it installs a helper app on the device and answers with XML page source. phonedriver installs nothing on Android, gives the model a short numbered text screen, and returns only what changed after each action. For a fixed test suite in CI, Appium is still the usual tool. I have not benchmarked phonedriver against Appium."
  ],
  [
    "Can Codex or Cursor control a phone?",
    "Yes. Codex: `codex mcp add phonedriver -- npx -y phonedriver mcp`. Cursor and any other client that starts a stdio MCP server: command `npx`, args `-y phonedriver mcp`. An agent with only a shell can run the same commands, such as `npx -y phonedriver snapshot`. Claude Code is the client I test against; see [Cursor and other clients](/docs/cursor-and-mcp-clients/)."
  ],
  [
    "How do I let an AI agent control my phone?",
    "Install phonedriver, add it to your agent as an MCP server (`claude mcp add phonedriver -- npx -y phonedriver mcp` for Claude Code), connect an Android phone with USB debugging, start an emulator, or boot the iOS Simulator, and ask the agent in plain language, for example \"Open Settings and turn on dark mode.\" The agent gets a text view of the screen and tools to tap, type and swipe. See the [setup guides](/docs/)."
  ],
  [
    "How do I let Claude control my Android phone?",
    "Add phonedriver to Claude Code as an MCP server with `claude mcp add phonedriver -- npx -y phonedriver mcp`, then connect a phone with USB debugging on or start an emulator. Nothing is installed on the phone. It is tested on emulators (API 26 and 37) and on a HONOR X8 (Android 13). See [the Claude Code guide](/docs/claude-code/)."
  ],
  [
    "What is phonedriver?",
    "phonedriver is a free, open-source CLI and MCP server that gives an AI agent a text view of an Android or iOS screen and the means to tap, type and swipe. Every action waits for the screen to settle and returns only the lines that changed, so the agent does not poll or sleep. It is licensed AGPL-3.0."
  ],
  [
    "Which AI agents and MCP clients work with it?",
    "Any client that starts a stdio MCP server can use it, including Claude Code and Cursor. Claude Code is the one I test against, with `claude -p` and Claude Sonnet 5.5 in my benchmarks. I have not run a Cursor session myself. The server has fourteen tools, and the CLI prints the same text as the MCP tools."
  ],
  [
    "Does it work on a real iPhone and a real Android phone?",
    "Yes for iPhone and iPad through the CLI and MCP, tested on an iPhone 14 Plus (iOS 26.1) and an iPad Pro (iOS 27.0); it needs a Mac with Xcode and a signing team. A real Android phone uses the same path as the emulator, over USB or Wi-Fi debugging; a HONOR X8 (Android 13) is the one phone tested so far. Android emulators (API 26 and 37) and the iOS Simulator are tested."
  ],
  [
    "What is the fastest way for an AI agent to control a phone?",
    "On an Android 17 emulator, phonedriver reads a 1,000-row list in 12.7 ms where adb takes 2,163.7 ms (170 times faster at p50, 95% CI 162.0 to 177.4), takes a screenshot in 9.1 ms against 291.2 ms (32 times) and types 100 characters in 158.3 ms against 3,638.4 ms (23 times). A bare tap is closer: 2.4 to 4.4 times faster host to reply. On a HONOR X8 phone (Android 13), reading the screen took 78.5 ms against 2,507 ms (32 times) and a screenshot 16 ms against 1,006 ms (61 times). See [the benchmarks](/benchmarks/)."
  ],
  [
    "How is phonedriver different from mobile-mcp and phone-harness?",
    "mobile-mcp is an MCP server that reads the accessibility tree and also supports cloud devices, with anonymous telemetry on by default. phone-harness is a Python CLI that drives Android through adb and iPhones through Mac mirroring and OCR. phonedriver keeps a resident agent on the Android device, waits for the screen to settle after every action and returns a diff, asks before taps that spend money, and sends no telemetry. See the [comparisons](/compare/)."
  ],
  [
    "Can the agent spend my money or delete things?",
    "Not without asking you. Before a tap on a button such as `Place order`, `Pay`, `Send $20.00` or `Delete account`, phonedriver stops and asks the human through the MCP client, showing the app, the button and the amounts on screen. Sending, deleting and installing are asked about too when you set `PHONEDRIVER_ASK`. This stops an agent that follows its rules or is steered by screen text; it does not stop one set on bypassing it, and it reads labels in eleven languages, not icons. See [the approval guard](/docs/approval-guard/)."
  ],
  [
    "Does phonedriver send telemetry?",
    "No. It talks to the device, the local daemon socket and the local adb server, and to Apple only when mounting an iOS developer disk image. A test runs the CLI, daemon and MCP server under a macOS sandbox that kills any process opening a non-local connection. Text passed with `--secret` never reaches logs, traces or replies. This website sets no cookies and loads no third-party scripts."
  ],
  [
    "Does it install anything on my phone?",
    "On Android, no app and no accessibility service. The daemon pushes a small jar over adb and starts it with `app_process` as the shell user, the way scrcpy does. On iOS, a small XCTest runner is built on your Mac with Xcode the first time it is needed."
  ],
  [
    "How many tokens does it use?",
    "On nine real phone jobs with Claude Sonnet 5.5 on one Android emulator (10 November 2026), phonedriver used 657,542 tokens and passed 27 of 27 runs. mobile-mcp used 4,567,200 (6.9 times as many) and passed 17 of 18. phone-harness used 1,127,495 and passed 14 of 18, and used fewer tokens than phonedriver on two of the nine jobs. The 14 MCP tools cost about 5,600 tokens on the first turn of a one-word `claude -p`. Small samples: see [the benchmarks](/benchmarks/#jobs)."
  ],
  [
    "Can the agent work without taking over my screen?",
    "On Android 13 and later, yes. Set `PHONEDRIVER_DISPLAY=hidden` in the MCP config, or pass `--display hidden` to the CLI, and the agent works on a hidden second screen while yours stays as it is. It has been tested on an emulator and on a HONOR X8. Typing into a web page shows the system's clipboard note, and your clipboard is put back after."
  ],
  [
    "What does it cost, and what is the license?",
    "It is free. The license is AGPL-3.0-only: if you distribute phonedriver, or run a modified version as a network service, you must publish your source under the same license. The Android agent contains files copied from scrcpy under Apache-2.0."
  ],
  [
    "What does it not do well yet?",
    "A bare tap misses its 15 ms budget. An iOS tap takes 357 ms on the Simulator and 454 to 534 ms on devices. WebViews and custom views can have an empty accessibility tree, where it falls back to OCR. The real-phone jobs benchmark ran two or three passes per job on one tablet emulator, so its medians describe what happened and are not rates. One Android phone model, a HONOR X8, has been tested."
  ]
] as [string, string][];

export const JOBS = [
  [
    "qa-signup",
    "Test the Signup tab of a testbed app and report every bug with steps",
    "The bugs exist only in how the screen behaves under use; no command exercises a form's validation path"
  ],
  [
    "qa-shop",
    "Test the Shop tab (quantity, totals, a discount code) and report bugs",
    "Same: a stale total and a flat discount show only under use"
  ],
  [
    "relay-code",
    "Copy a random pickup code from the Relay tab into a new contact \"Locker 7\"",
    "The code exists only in a TextView and the app's private storage"
  ],
  [
    "chrome-prefs",
    "Make DuckDuckGo the default search engine and turn on \"Always use secure connections\"",
    "Chrome keeps both in a private Preferences file; no intent or setting reaches them"
  ],
  [
    "clock-settings",
    "Add Tokyo and London world clocks, set snooze to 15 min and silence after 5 min",
    "These live in Clock's private preferences; the alarm intents set alarms and timers only"
  ],
  [
    "messages-pin-archive",
    "Pin one conversation, archive another, leave a third",
    "Pin and archive are private Messages flags with no command"
  ],
  [
    "photos-crop",
    "Crop a photo to a square and save a copy, keeping the original",
    "The Photos editor is the only crop tool; a shell has no image editor"
  ],
  [
    "wakelock-restrict (mixed)",
    "Find the app holding a wakelock and set its battery usage to Restricted",
    "A shell finds the holder and Settings sets the restriction; any route allowed"
  ],
  [
    "log-code (mixed)",
    "Find a pickup code in the device log and put it in a contact \"Locker 9\"",
    "The code is only in logcat; the contact is made in the Contacts UI; any route allowed"
  ]
] as [string, string, string][];

export const DEMO_CAPTION = "One real Claude Code (Opus 5.5) session on an Android emulator, ordering on dominos.co.uk. The run took 6.5 minutes and is shown at 15x; the approval prompt plays at real speed. Nothing was ordered.";


export const NPX = "npx -y phonedriver mcp";
export const MCP_CONFIG = `{
  "mcpServers": {
    "phonedriver": { "command": "npx", "args": ["-y", "phonedriver", "mcp"] }
  }
}`;
