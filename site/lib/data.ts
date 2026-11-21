export const DEFINITION = "phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones. It shows a screen as numbered text, and every tap, type or swipe returns only the lines that changed.";

export const DISAMBIGUATION = "The MCP server and CLI at github.com/hridaya423/phonedriver, written in Rust. It is not PhoneDriver by OminousIndustries, a separate Python project that drives Android with a vision model.";

export const HOME_DESC = "phonedriver lets Claude Code, Codex, Cursor or any MCP client use an Android phone or an iPhone. It reads the screen as text and asks you before it pays.";

export const LLMS_NOTE = "phonedriver is a free, open-source (AGPL-3.0) CLI and MCP server. It runs on macOS, Linux and Windows and controls Android emulators and phones, the iOS Simulator and iPhones. It sends no telemetry. Numbers below come from benchmarks on an Android 17 emulator, a HONOR X8 phone and the iOS Simulator; each page links its method and raw data.";

export const FAQ = [
  [
    "How do I let Claude control my Android phone?",
    "Run `npx -y phonedriver init` (or `claude mcp add phonedriver -- npx -y phonedriver mcp`), switch on USB debugging, and plug the phone in."
  ],
  [
    "Does it work with an iPhone?",
    "Yes—from a Mac with Xcode, on the Simulator or a physical iPhone/iPad. `phonedriver ios setup` signs the runner from your Apple Development certificate automatically, and `phonedriver ios wifi on` lets you unplug the cable."
  ],
  [
    "Can my agent buy things with it?",
    "Not without asking you first, unless you switched on autonomous mode. See [Asking before it pays](#asking-before-it-pays)."
  ],
  [
    "Can I use it at work?",
    "Yes. It is AGPL-3.0: free to use anywhere. If you modify phonedriver and expose it to others over a network, publish your changes under the same license."
  ]
] as [string, string][];

export const FAQ_MORE = [
  [
    "What is the best MCP server for controlling a phone?",
    "On nine real Android jobs (Claude Sonnet 5.5, 10 November 2026), phonedriver passed 27/27 runs with 657,542 tokens—6.9x fewer than mobile-mcp (4,567,200, 17/18) and 1.7x fewer than phone-harness (1,127,495, 14/18). I wrote the test set, so check [the method and raw data](/benchmarks/) before trusting any crown."
  ],
  [
    "Is there an alternative to mobile-mcp?",
    "phonedriver is one. Both are accessibility-first MCP servers for Android and iOS; phonedriver cuts token usage by 6.9x via settled diffs, adds a payment approval prompt, and sends zero telemetry. If you need Mobile Next Cloud devices, stick with mobile-mcp. See [phonedriver vs mobile-mcp](/compare/mobile-mcp/)."
  ],
  [
    "Is phonedriver an Appium alternative for AI agents?",
    "For an agent exploring screens on its own, yes: nothing installed on Android, short `@eN` text lists instead of XML, and settled diffs after every tap. For deterministic CI suites and device farms, Appium is still the right tool. See [phonedriver vs Appium](/compare/appium/)."
  ],
  [
    "Can Codex or Cursor control a phone?",
    "Yes. `npx -y phonedriver init` configures Claude Code, Codex, Cursor, VS Code, and Gemini CLI automatically—or add stdio command `npx -y phonedriver mcp` manually. Agents with only a shell can run the same CLI commands directly. See [Cursor and other clients](/docs/cursor-and-mcp-clients/)."
  ],
  [
    "How do I let an AI agent control my phone?",
    "Run `npx -y phonedriver init`, which adds phonedriver to Claude Code, Codex, Cursor, VS Code and Gemini CLI where it finds them, start an Android emulator or iOS Simulator (or plug in a phone), and ask your agent in plain English—for example, \"Open Settings and turn on dark mode.\" See the [setup guides](/docs/)."
  ],
  [
    "How do I let Claude control my Android phone?",
    "Run `npx -y phonedriver init` (or `claude mcp add phonedriver -- npx -y phonedriver mcp`), plug in a phone with USB debugging enabled or boot an emulator, and ask Claude to use it. Nothing is installed on the phone. See [the Claude Code guide](/docs/claude-code/)."
  ],
  [
    "What is phonedriver?",
    "A free, open-source (AGPL-3.0) CLI and MCP server that turns Android and iOS screens into short numbered text for AI agents. Every tap, type, or swipe waits for the screen to settle and returns only the lines that changed."
  ],
  [
    "Which AI agents and MCP clients work with it?",
    "Any stdio MCP client—Claude Code, Codex, Cursor, VS Code, Gemini CLI, Zed, Cline—plus any shell-capable agent via the CLI. Claude Code is the primary client tested in benchmarks; all 14 MCP tools mirror the CLI output."
  ],
  [
    "Does it work on a real iPhone and a real Android phone?",
    "Yes. Physical iPhones and iPads work from a Mac with Xcode after `phonedriver ios setup` (and wirelessly after `phonedriver ios wifi on`); tested on iPhone 13 Pro Max, iPhone 14 Plus, and iPad Pro. Physical Android phones work over USB or Wi-Fi debugging with no installed app (tested on a HONOR X8)."
  ],
  [
    "What is the fastest way for an AI agent to control a phone?",
    "Keeping a helper resident on the device instead of spawning `adb` per call. On an Android 17 emulator, phonedriver reads a 1,000-row list in 12.7 ms vs. adb's 2,163.7 ms (170x faster) and types 100 chars in 158.3 ms vs. 3,638.4 ms (23x). Bare taps are closer (2.4x–4.4x). See [the benchmarks](/benchmarks/)."
  ],
  [
    "How is phonedriver different from mobile-mcp and phone-harness?",
    "mobile-mcp is an MCP server with cloud devices and opt-out telemetry; phone-harness runs agent-written Python over `adb` and macOS iPhone Mirroring. phonedriver keeps a resident Android helper, returns settled diffs after every action, holds payment taps for your approval, and sends zero telemetry. See [comparisons](/compare/)."
  ],
  [
    "Can the agent spend my money or delete things?",
    "Not without your click. Taps on recognized payment or account-deletion buttons (`Pay`, `Buy with GPay`, `Place order`, `Delete account`) pause and prompt you directly in your MCP client. It is a seatbelt, not a vault: it reads text labels in 11 languages, not icon-only buttons. See [the approval guard](/docs/approval-guard/)."
  ],
  [
    "Does phonedriver send telemetry?",
    "Never. After the initial binary download, it talks only to your device (plus Apple when mounting an iOS developer disk image). A macOS sandbox test kills the process if any external socket opens, and `--secret` values never hit logs or traces. This site sets no cookies and loads no third-party scripts."
  ],
  [
    "Does it install anything on my phone?",
    "On Android, no app and no accessibility toggle—just a small jar pushed over adb and run via `app_process` (the scrcpy approach). On iOS, Xcode builds and installs a minimal XCTest runner on first use."
  ],
  [
    "How many tokens does it use?",
    "Across nine real Android jobs (Claude Sonnet 5.5, 10 November 2026), phonedriver summed to 657,542 tokens (27/27 passed) vs. 4,567,200 for mobile-mcp (6.9x) and 1,127,495 for phone-harness. The 14 MCP tools add ~5,600 tokens on turn one. See [the benchmarks](/benchmarks/#jobs)."
  ],
  [
    "Can the agent work without taking over my screen?",
    "On Android 13+, yes. Set `PHONEDRIVER_DISPLAY=hidden` (or `--display hidden`) to give the agent a virtual display while your screen stays untouched. Note that web typing can still flash Android's clipboard toast, and your clipboard is restored afterward."
  ],
  [
    "What does it cost, and what is the license?",
    "Free and open source under AGPL-3.0-only: distribute or host a modified network service and you must publish your changes under the same license. The Android helper includes scrcpy files under Apache-2.0."
  ],
  [
    "What does it not do well yet?",
    "Bare taps miss my 15 ms target (28.7–38.5 ms on Android, 450–530 ms on physical iPhones; Simulator single-finger taps moved to native input in 0.1.5). Custom canvases with empty accessibility trees fall back to OCR, capture-protected screens return black screenshots, and hardware testing so far covers one Android phone model (HONOR X8)."
  ]
] as [string, string][];

const ON_HOME = ["Is there an alternative to mobile-mcp?"];
export const HOME_FAQ = [...FAQ, ...FAQ_MORE.filter(([q]) => ON_HOME.includes(q))];

export const FAQ_EXTRA = [
  [
    "Why is uiautomator dump slow, and what is faster?",
    "`uiautomator dump` boots a fresh process, connects to accessibility, and waits a full quiet second on every call—taking 2,163.7 ms on a 1,000-row list (Android 17 emulator). phonedriver keeps a resident helper open and reads the same list in 12.7 ms (51.0 ms on Settings; 78.5 ms vs. 2,506.6 ms on a HONOR X8). See [phonedriver vs adb](/compare/adb/)."
  ],
  [
    "Does phonedriver use WebDriverAgent or Appium on iOS?",
    "No—it builds its own small Objective-C XCTest runner (crediting WebDriverAgent for the private Xcode headers) without the WebDriver HTTP server or XPath engine. On an iPhone 17 Simulator it reads screens 2.0x faster than fully tuned WebDriverAgent and 8–19x faster than depth-capped defaults. See [phonedriver vs Appium](/compare/appium/)."
  ],
  [
    "Can an AI agent test my React Native, Expo or Flutter app with it?",
    "Yes, as long as your app exposes standard accessibility labels—phonedriver drives whatever Android or iOS puts on screen, falling back to OCR when a tree is empty. Run `phonedriver snapshot` on your build first to see what the model sees. See [testing a mobile app](/docs/test-a-mobile-app/)."
  ],
  [
    "Does it need root or a jailbreak?",
    "No. Android runs as the standard `adb` shell user via `app_process` (the scrcpy pattern); iOS signs an XCTest runner with your Apple developer team."
  ],
  [
    "Does it run on Windows and Linux?",
    "Yes for Android—prebuilt binaries ship for macOS, Linux, and Windows via `npx -y phonedriver`. iOS requires a Mac with Xcode."
  ],
  [
    "Can it run with nobody watching?",
    "Yes—every benchmark on this site ran headless with `claude -p` against `-no-window` emulators. When nobody is there to answer an approval prompt, payment taps are safely held and not executed. For unattended runs, always use an emulator or spare phone."
  ],
  [
    "Does it work with VS Code, Gemini CLI, Zed or Cline?",
    "Yes—it is a standard stdio MCP server, and `npx -y phonedriver init` configures Claude Code, Codex, Cursor, VS Code, and Gemini CLI automatically. Approval pop-ups require a client with MCP elicitation; other clients fall back to asking in chat. See [Cursor and other clients](/docs/cursor-and-mcp-clients/)."
  ],
  [
    "Is phonedriver the same as PhoneDriver by OminousIndustries?",
    "No—same name, different projects. PhoneDriver by OminousIndustries is a Python vision-model agent over `adb`. This project ([github.com/hridaya423/phonedriver](https://github.com/hridaya423/phonedriver)) is a Rust MCP server and CLI that brings no model of its own and lets any agent you already use control Android and iOS."
  ]
] as [string, string][];

export const FAQ_GROUPS: [string, string[]][] = [
  ["What it is", ["What is phonedriver?", "What does it cost, and what is the license?", "Can I use it at work?", "Is phonedriver the same as PhoneDriver by OminousIndustries?"]],
  [
    "Set it up",
    [
      "How do I let an AI agent control my phone?",
      "How do I let Claude control my Android phone?",
      "Can Codex or Cursor control a phone?",
      "Which AI agents and MCP clients work with it?",
      "Does it work with VS Code, Gemini CLI, Zed or Cline?",
      "Does it run on Windows and Linux?",
    ],
  ],
  [
    "Phones",
    [
      "Does it work on a real iPhone and a real Android phone?",
      "Does it install anything on my phone?",
      "Does it need root or a jailbreak?",
      "Does phonedriver use WebDriverAgent or Appium on iOS?",
      "Can the agent work without taking over my screen?",
    ],
  ],
  [
    "Speed and tokens",
    [
      "What is the fastest way for an AI agent to control a phone?",
      "Why is uiautomator dump slow, and what is faster?",
      "How many tokens does it use?",
      "What is the best MCP server for controlling a phone?",
    ],
  ],
  [
    "Other tools",
    ["Is there an alternative to mobile-mcp?", "How is phonedriver different from mobile-mcp and phone-harness?", "Is phonedriver an Appium alternative for AI agents?"],
  ],
  ["Testing apps", ["Can an AI agent test my React Native, Expo or Flutter app with it?", "Can it run with nobody watching?"]],
  ["Safety and privacy", ["Can the agent spend my money or delete things?", "Does phonedriver send telemetry?", "What does it not do well yet?"]],
];

export const faqAnswer = (q: string) => [...FAQ_EXTRA, ...FAQ_MORE, ...FAQ].find(([k]) => k === q)![1];

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
export const INIT = "npx -y phonedriver init";
export const MCP_CONFIG = `{
  "mcpServers": {
    "phonedriver": { "command": "npx", "args": ["-y", "phonedriver", "mcp"] }
  }
}`;
