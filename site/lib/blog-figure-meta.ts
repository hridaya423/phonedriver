export type FigureId =
  | "recycled-row"
  | "pipe-vs-cold"
  | "settle-scope"
  | "speed-race"
  | "tap-waterfall"
  | "bursty-stream"
  | "guard-playground"
  | "elicitation-circuit"
  | "token-snowball"
  | "shortcut-trap"
  | "grader-bugs";

/** The plain description of each blog figure, for the Markdown exports and readers without the visual. */
export const FIGURE_FALLBACK: Record<FigureId, string> = {
  "recycled-row":
    "[Visual: Android ListView reusing node ID -4294967273 for \"Row 13\" and then \"Row 29\" as the list scrolls. A tool that follows the node ID taps Row 29 and reports ok; phonedriver refuses the ref, because inside a list a relabeled row is another item.]",
  "pipe-vs-cold":
    "[Visual: adb spawning a fresh process, app_process JVM, UiAutomation connect, and 1 s idle wait on every call (2,163.7 ms) vs. phonedriver sending JSON over an open control socket beside an H.264 stream (12.7 ms).]",
  "settle-scope":
    "[Visual: Two-phase settle thresholds in pt² — blinking caret 96, first_change_area 128, Material switch toggle 432, indeterminate spinner 480, quiet threshold 4,096, and screen transition 13,000 to 254,000.]",
  "speed-race":
    "[Visual: Round-trip latency race comparing phonedriver vs. adb on an Android 17 emulator (12.7 vs. 2,163.7 ms list read down to 28.7 vs. 69.4 ms paced tap) and a HONOR X8 phone (78.5 vs. 2,506.6 ms read; 16.4 vs. 1,005.7 ms screenshot).]",
  "tap-waterfall":
    "[Visual: Settled-tap phase waterfall over a 100 ms tunnel (prepare 131 ms, input 130 ms, settle 318 ms, tree read 166 ms, stabilize 254 ms, host render 0.2 ms) vs. the 120 ms overlapped probe read that shifted p50 from 918 ms to 843 ms.]",
  "bursty-stream":
    "[Visual: Timeline contrasting an assumed steady video stream against the emulator's 10 fps bursts followed by up to 5 s of silence that broke 18 of 40 idle screenshots, and the 0.1.5 freshness barrier.]",
  "guard-playground":
    "[Visual: Interactive approval-guard classifier testing labels across 11 languages, money-verb + amount patterns, intentional skips like Add to cart and bare Subscribe, PHONEDRIVER_ASK toggles, and the icon-only blind spot.]",
  "elicitation-circuit":
    "[Visual: MCP elicitation flow routing a held Buy with GPay tap around the model straight to the human via input_required, contrasted with the autonomous: true bypass path.]",
  "token-snowball":
    "[Visual: Turn-by-turn context compounding (2.8x calls times 2.5x context = 6.9x tokens: 657,542 vs. 4,567,200) and a 9-job breakdown comparing phonedriver, phone-harness, mobile-mcp, and adb.]",
  "shortcut-trap":
    "[Visual: Side-by-side comparison of setting an alarm (bypassed by raw adb via SET_ALARM in 2 calls / 18,607 tokens) vs. changing Chrome's search engine (blocked from shell by private Preferences, requiring UI interaction).]",
  "grader-bugs":
    "[Visual: Five benchmark grader bugs caught by hand-testing pass and fail states before running any model ($36.00 sentence split, Europe/London false pass, Messages pins table, missing Chrome Preferences crash, and Chrome's renamed HTTPS preference key).]",
};
