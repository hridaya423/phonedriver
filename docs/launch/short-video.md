# Short video: 20 to 30 s vertical cut (Shorts, TikTok, Reels)

Nothing here is posted. Source: the demo recording. Two states of it exist:

- Recorded 2026-11-08: `.github/assets/demo.mp4` (1280x720, 46.3 s, Claude Code on Opus 5.5 beside an Android 17 emulator) and the vertical cut `.github/assets/demo-vertical.mp4` (1080x1920, 29.9 s, built by `design/build/demo_compose.py` from the raw recordings with `design/build/demo/edit-short.json`: the Uber Eats scene at 36x, the approval prompt at real speed, the end card). The prompt scene ends on Decline: Claude says "I didn't place the order ... declined at the permission prompt". The captions and the speed badge are burned in. Contact sheet of every second: `docs/launch/demo-contact-sheet.png`.

Do not cut this from `demo.mp4`. The emulator is about 270 px wide in that file; at 1080x1920 it would be a 4x upscale and read as mush. Re-cut from the raw emulator `screenrecord` segments and a fresh terminal capture (`design/build/demo_compose.py` is the composer) at 1080x1920 H.264, 30 fps, AAC audio (silence is fine, a quiet track is better than none on TikTok).

## The rule that matters

Say "sped up 10x" on screen the first time the clock runs. The best comparable post, a Claude iPhone MCP with 180 points, drew its top objection about a sped-up demo (`research.md`, round 2). Putting the speed in the frame removes that objection and costs nothing.

## Layout (1080x1920)

- Safe zone: keep captions inside the middle 1080x1400. The top 250 px and the bottom 420 px are covered by platform UI on TikTok and Reels (verify against `research-2026-11.md`, video section).
- Top 55%: the emulator, cropped to its own pane, centered, large.
- Lower 30%: the Claude Code terminal, cropped to the last 6 lines, large monospace, 40 px or more. The tool-call lines are the story.
- Captions: one line at a time, white on a dark bar, 56 px, at most 6 words. Burned in, because most viewers have sound off.

## Script (28 s)

| Time | Picture | Burned-in caption | Voiceover (optional, your own voice, no TTS) |
|---|---|---|---|
| 0.0 to 2.0 | Hard open on the emulator at Uber Eats' home page, the prompt typed in the terminal below. | "Claude is ordering me dinner." | "I asked Claude to order butter chicken and naan." |
| 2.0 to 6.0 | Cookie banner rejected, address set, search "Indian". Clock badge "10x speed" in the corner. | "On a real Android emulator. 10x speed." | "It drives the phone itself. Tap, type, scroll." |
| 6.0 to 11.0 | Patiala Indian Grill and Bar, 4.8 stars, scroll the menu. | "It picked the best-rated place: 4.8 stars." | "It read the ratings and picked the best one." |
| 11.0 to 15.0 | Butter Chicken $20.95 added, Garlic Naan $7.95 added, cart shows $28.90. | "Both items. $28.90." | none, let the cart sit for a beat |
| 15.0 to 19.0 | The agent taps Place order. The Claude Code form appears: the app, the button, the lines. Cursor on Allow. | "Then it hit Place order. And stopped." | "phonedriver asks me before it taps Pay." |
| 19.0 to 22.0 | Allow, for this session, Decline: the three choices fill the frame. If the new recording ends on Decline, show "The human declined; nothing was tapped". | "Allow. Allow for session. Decline." | "I decide. Not the model." |
| 22.0 to 26.0 | Cut to the hero chart: "Up to 98x faster than adb", the four bars. | "98x faster than adb at reading the screen." | "It also reads the screen 98 times faster than adb." |
| 26.0 to 28.0 | End card: `phonedriver` wordmark, `phonedriver.dev`, "open source". | "phonedriver. Open source. Free." | "It's open source. Link in bio." |

If the approval scene is missing from the new recording, the fallback ending for 15.0 to 22.0 is the real one from today's clip: the agent reaches Uber's sign-in page and stops. Caption: "Checkout needs me to sign in. Claude stopped and asked." That is true of today's clip and names no feature that was not shown. Do not caption it "approval guard": signed out, Uber Eats never shows Place order, so the guard did not fire (`progress.md`, "README demo retake").

## Hook options for the first 2 seconds (pick one, test a second on another platform)

1. "Claude is ordering me dinner." (default: a result the viewer can picture)
2. "I gave Claude my phone. It can't spend my money without asking." (the safety hook; strongest if the approval prompt is in the first frame)
3. "adb reads a phone screen in 2.3 seconds. This takes 24 milliseconds." (the technical hook, for TikTok's developer pocket and Shorts)

## Title, description and tags per platform

| Platform | Title or first line | Description | Tags |
|---|---|---|---|
| YouTube Shorts | "Claude orders dinner on a phone and stops at checkout" | "phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones. It asks you before it taps Pay or Place order. phonedriver.dev" | `#Shorts #ClaudeCode #MCP` |
| TikTok | "Claude orders dinner. Then it has to ask me." | One sentence plus `phonedriver.dev` in the bio link. Links in captions are dead on TikTok. | `#claudecode #mcp #aiagents #coding` |
| Instagram Reels | Same as TikTok. | Same. Reels allows no clickable link in the caption: use the bio link. | 3 to 5 tags at most |

## What not to do

- Do not use "real time", "no cuts" or "fully autonomous". The clip is sped up 10x and the prompt told the agent where to stop.
- Do not use a stock voice or an AI-generated narrator. Your own voice or no voice.
- Do not post the same upload to all three on the same minute. Post Shorts first, then TikTok and Reels a few hours apart, so the comment replies are answerable.
- No engagement asks in the caption ("comment BUTTER for the link"). It is bait and the platforms demote it.

## Checklist

- [ ] The new recording exists and the terminal reads `phonedriver`.
- [ ] 1080x1920, 20 to 30 s, burned-in captions, "10x speed" visible.
- [ ] Every caption number traces to `claims-ledger.md` (4.8 stars, $20.95, $7.95, $28.90, 98x).
- [ ] Looked at frames at 1, 5, 10, 15, 20, 25 s on a phone-sized preview.
- [ ] No personal data in the frame (Chrome signed out, no account, no address of yours).
