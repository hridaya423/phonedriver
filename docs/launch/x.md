# X draft

Nothing here has been posted. Slots in `[TBD: ...]` wait on a measurement or a build. `claims-ledger.md` names the repo file behind every claim. Write in your own voice before posting; these are drafts.

## Launch post

Attach `.github/assets/demo.mp4` as native video (960x660, 32 s, 1 MB). Put the link in the post text, not in a reply: X staff said in July 2026 that replies are no longer needed for links (`research.md`).

```
Claude Code driving an iPhone Simulator. 24 seconds from prompt to answer, real time, cut only at the start and end.

It is a first take. The agent hit a donation page and recovered.

Every tap returns only what changed, as text. Android and iOS, MCP and CLI, Rust, open source.

Reading an Android screen: 237 ms. adb uiautomator dump: 2,800 ms.

github.com/hridaya423/phone-use
```

Notes:

- The hook is the first two lines: a real recording with a stated length, and an admitted flaw. Both are checkable in the video.
- The number is the screen read from `docs/bench/adb.md` (busy emulator, median of 50). [TBD: re-run on a quiet machine and update both figures]
- No tags and no hashtags. Tag no one unless a person agrees to be named.
- Alt text for the video: "Split screen. Left: Claude Code in a terminal asked to find when Grace Hopper was born. Right: an iPhone Simulator opening Safari, loading Wikipedia, and the agent answering December 9, 1906."
- After posting, reply to every reply for the first two hours. The author-answered reply carries the largest published weight (`research.md`).

## Alternative hooks

Each replaces the first two paragraphs of the post. Keep the video, the one number line and the link.

**Hook B, the number first.**

```
adb uiautomator dump takes 2.8 seconds to read one Android screen.

An agent that taps 20 times spends a minute reading, not acting. phone-use reads the same screen in 237 ms and returns only what changed after each tap.
```

Check before use: "taps 20 times spends a minute" is arithmetic (20 x 2.8 s = 56 s), not a measured run. It is also an upper bound that assumes a dump after every tap. [TBD: say "an agent that dumps after every tap" in the final text]

**Hook C, the admission.**

```
This agent run has a mistake in it, and I left it in.

The typed search did not register, Safari landed on a donation page, and the agent loaded the article by URL instead. Real time, no trimming between tool calls. Every call succeeded.
```

Hook C suits a reader who has seen faked agent demos. Hook B suits the speed crowd. The recommended post uses a blend of both.

## Thread (optional, 5 posts)

Post 1 is the launch post above. Posts 2 to 5 reply to it. Keep each under 280 characters where you can, and put the media first.

**2. What the agent sees.** Image: a screenshot of the terminal showing this output (from the README, iOS Simulator).

```
ok tapped @e41 "Dark Appearance" at 337,198 · settled in 607ms
~ @e41 switch "Dark Appearance" off → on
+ @e68 button "L4S"
```

Text: "The agent never polls or sleeps. Each action waits for the screen to settle, then returns the lines that appeared (+), left (-) or changed (~)."

**3. How it works on Android.**

Text: "Nothing is installed on the phone. The daemon pushes a small dex jar and starts it with app_process as the shell user, the way scrcpy does. It reads the tree through UiAutomation and injects touches through InputManager. No accessibility service to switch on."

**4. The numbers and where they came from.** Image: `.github/assets/speed-light.svg` exported to PNG (X does not take SVG).

Text: "Android 17 emulator, busy machine, median of 50, phone-use and adb measured back to back. Screenshot 45 vs 692 ms. Screen read 237 vs 2,800 ms. Tap then what changed 667 vs 3,112 ms. A bare tap is only 1.6x. Method and caveats in docs/bench/adb.md."

**5. What does not work yet.**

Text: "A tap misses its budget: 357 ms on the iOS Simulator, 454 to 534 ms on an iPhone and iPad, because XCTest takes about 240 ms to confirm an event. No physical Android phone is measured. iOS needs a Mac and Xcode. Web views can have an empty tree; OCR covers part of that. [TBD: final tap latency]"

**6. Ask.** Optional, use only if the thread is moving.

Text: "AGPL-3.0, no telemetry. If you have a real Android phone or an app where the snapshot comes back empty, I want the trace. github.com/hridaya423/phone-use"

Do not put the Show HN link in any post and do not ask anyone to vote. HN treats that as vote solicitation, and the same demo link gives HN readers nothing new.
