# LinkedIn draft

Nothing here has been posted. Slots in `[TBD: ...]` wait on a measurement or a build. `claims-ledger.md` names the repo file behind every claim. LinkedIn is cutting reach on posts it classes as AI slop (`research.md`), so write the final text by hand and keep your own details in it.

The first two lines fit inside the 140-character mobile cut-off before "see more". Attach `.github/assets/demo.mp4` as native video. Put the link in the body as the last line. The only study of this (Ordinal, 2023 to early 2026) found little or no reach loss for links on personal profiles, but its method is partly unclear. If the first hour of reach is poor, move the link to the first comment on the next post and compare.

## Post

```
Reading one Android screen with adb takes 2.8 seconds.
Reading it with phone-use takes 237 milliseconds, on the same emulator.

I built phone-use so an AI agent can use a phone without waiting on the plumbing. It is a Rust CLI and MCP server for Android and iOS. Every tap returns only the lines that changed on screen, as text, so the agent does not read the whole screen again.

The video is Claude Code driving an iPhone Simulator: 24 seconds from prompt to answer, in real time, cut only at the start and end. It is a first take. The agent landed on a donation page and recovered by loading the article by URL.

Two things I got wrong, so you do not have to find them:

My first agent evaluation scored 9 of 15. I read all six failures. Each was a bug in my check or my device reset, not in the tool. After I fixed the checks, the rerun scored 15 of 15, with a median of 6 tool calls.

A tap still misses its latency budget. It takes 357 ms on the iOS Simulator and 454 to 534 ms on an iPhone and an iPad, because XCTest takes about 240 ms to confirm an event. I have not measured a physical Android phone at all.

The speed numbers come from an emulator on a busy machine, with adb measured back to back. The method and caveats are in the repo.

It is AGPL-3.0 and sends no telemetry. If you have a real Android phone, or an app where the screen comes back empty, I want your trace.

github.com/hridaya423/phone-use
```

Notes:

- Length: about 1,500 characters, inside the 3,000 limit. Short paragraphs, no one-sentence-per-line run, no emoji, no hashtags.
- Replace "I built phone-use so an AI agent can use a phone without waiting on the plumbing" with your real reason. [USER: one true sentence on why you built it]
- Two more candidates for the two-line opening, if you prefer a story: "My first agent evaluation scored 9 of 15. The tool was fine. My checks were wrong." Or: "I left a mistake in my demo video on purpose."
- Native video: the file is 960x660 H.264 MP4, which LinkedIn accepts. The feed autoplays muted. The terminal text in the clip is readable on mute, but add one short caption overlay with the task: "Open Safari, search Wikipedia for Grace Hopper, say when she was born." [TBD: caption added to a copy of the clip; do not overwrite `demo.mp4`]
- Add a document or image only if you have one beyond the video. Candidate: `speed-light.svg` exported to PNG.
- [TBD: token benchmark] Add one sentence with the result once it exists. Do not add it before.
- Reply to every comment in the first hour. Saves and substantive comments outweigh likes (vendor sources, `research.md`).
