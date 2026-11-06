# X plan (round 2)

Nothing here has been posted. Slots: `[TBD: ...]` waits on a measurement or a build. `claims-ledger.md` names the repo file behind each claim. The demo is being re-recorded, so no line below depends on how the old clip went.

## How the ranking rules shape the post

Source for every row: `research.md`, round 2, X section (the 2026 `xai-org/x-algorithm` release, `home-mixer/params/param.rs`).

| Rule | Weight or value | What the post does |
|---|---|---|
| Score is the sum of weight times predicted probability for each action. | n/a | Write for the actions with the largest weights, and avoid the negative ones. |
| Reply | 5.0 (+15.0 on an original post by a mutual follow) | End with a real question. Prime a few mutuals to try the tool and reply with what they saw. |
| Share via copy link | 20.0 | Make the post easy to send: one clip, one claim, one link, readable without context. |
| Quote | 5.0 | State one number that people can quote. |
| Follow author | 4.0 | Keep your profile showing what you build. |
| Like | 0.708. Repost 1.0. | Lower weights. The first likes still matter for the cold-start slot (next row). |
| Cold start: original post, author under 50,000 followers, post under 2 hours old, under 200 impressions. A post can be lifted to about rank 15 in a feed. The pick is Thompson sampling on likes per home view. | n/a | Post when your own followers are online. The first 200 impressions need to turn into likes. The first line and the first frame carry that. |
| Click dwell time 0.385. Click 0.274. Open link 0.2. | positive | A link in the post is not penalized in the 2026 code. It earns small positive weights when people click and stay. |
| Link penalty | Bier 2026-04-12: "Just post the link in the body of the post and provide sufficient context". Bier 2026-07-28 and Musk 2026-07-29: no penalty (verified through one news article). | Put the link in the post, with context. This departs from the "link in a reply" idea. The reasons: the code has no link multiplier, X staff say to use the body, and a reply from you is a reply that non-followers do not see (`OONRetweetReplyFilter`). If you want a hedge anyway, repeat the link in your first reply. |
| Out-of-network scaling | x0.75, and replies and reposts from accounts the viewer does not follow are dropped | Reach beyond followers comes from the main post only. Put everything the launch needs in it. |
| Author diversity | second post from you in one feed batch keeps 0.625, third 0.4375, floor 0.25 | Post once. Do not post a second standalone post that day. |
| Video | Video quality view counts only above 10 s and has weight 0. Video value comes through dwell. | Use a native MP4 of 15 to 30 s. |
| Negative actions | not interested -53.07, mute -58.8, block -31.2, report -234.0 | No tags of unrelated accounts. No hashtags. No "like and retweet" asks. Nothing that reads as bait. |
| Engagement-spammer and "slop" labels exist (`author_rules.rs`) | size of penalty unknown | The question must be real, and you must reply to answers. Do not ask for replies of "yes" or "1". |
| Premium multiplier | none in the 2026 code | Do not rely on Premium for reach. |
| 48-hour age cut | n/a | Day-one replies matter most. |

No source in the code or the research supports effects for tagging, pinned posts, delete and repost, aspect ratio, or posting hour. Do not claim them.

## The post

Attach the re-recorded demo as native video. `.github/assets/demo.mp4` is the old clip (960x660, 32 s, 1 MB). [TBD: replace with the re-recorded clip; confirm length, resolution and that it needs no sound]

Draft in your own voice. The first two lines must work alone in the feed.

```
adb takes 2.8 seconds to read one Android screen.
phone-use takes 237 ms on the same emulator, and every tap returns only what changed.

Claude Code driving a phone with it, real time:
[video]

Android + iOS, MCP + CLI, Rust, AGPL.
github.com/hridaya423/phone-use

Which app should I point it at next?
```

Notes:

- The number is `docs/bench/adb.md`: Settings screen read, 237.0 ms against 2,800.5 ms, emulator on a busy machine, median of 50. [TBD: paired quiet-machine re-run; update both figures]
- "real time" is true only if the re-recorded clip is real time and uncut between tool calls. [TBD: confirm for the new clip]
- The question gives replies a reason to exist. Pick a real one. Replies like "WhatsApp" or "my banking app" are answers you can act on, and also a place to say what the tool cannot do yet.
- Alt text: describe the clip in one sentence once it exists. [TBD: after the re-record]

## Before posting (mutuals boost, cold start)

1. Pick 5 to 10 mutuals (people you follow who follow you back) who work on agents or mobile. Send each a direct message a day before: what the tool is, the repo link, and "tell me what breaks". Do not send a script. Do not ask for likes. The +15 reply weight applies to a post from a mutual, and replies need to be real: the code labels engagement spammers.
2. Post when your followers are online. For most developer audiences that is Monday 13:00 to 15:00 UTC (9 to 11 am ET). Vendor studies agree on weekday mornings, with no primary source.
3. Do not post the HN link, ask for votes anywhere, or tag anyone.

## First two hours

- Reply to every reply, and quote-post nobody. Replies you answer keep the post alive. The 2023 weights put a reply the author answers at 75 against 0.5 for a like; the 2026 list dropped that head, so treat the habit as good practice and not a proven multiplier.
- When someone quotes the post, reply to the quote with a fact.
- Do not delete and repost.
- If the post gets a report or a "not interested" wave, do nothing; do not argue.

## Alternative hooks

Each replaces the first two lines. Keep the video, link and question.

**Hook B, the diff first.**

```
A phone tap, as text:
ok tapped @e41 "Dark Appearance" · settled in 607ms
~ @e41 switch "Dark Appearance" off → on

The agent gets only the lines that changed. phone-use, open source, Android + iOS.
```

(The reply text is the README's iOS Simulator sample.) This hook shows the mechanism. It suits readers who skip numbers.

**Hook C, the screenshot number.**

```
Screenshot of an Android emulator: 45 ms with phone-use, 692 ms with adb.
Reading the screen: 237 ms against 2,800 ms.
```

(`docs/bench/adb.md`. Same caveats and the same [TBD] as the main post.)

## Thread (optional depth, 4 replies to your own post)

Replies from non-followers do not travel, so this thread serves people who open the post. Keep each reply self-contained and put no link in more than one.

1. **How it works on Android.** "Nothing is installed on the phone. A small dex jar starts with app_process as the shell user, like scrcpy. Tree through UiAutomation, touches through InputManager. No accessibility service to switch on."
2. **Speed table.** Image: `.github/assets/speed-light.svg` exported to PNG (X does not take SVG). "Android 17 emulator, busy machine, median of 50, adb measured back to back. A bare tap is only 1.6x. Method and caveats: docs/bench/adb.md."
3. **What does not work yet.** "A tap misses its 15 ms budget: about 34 ms on the emulator now, 357 ms on the iOS Simulator, 454 to 534 ms on an iPhone and iPad (XCTest takes about 240 ms to confirm an event). No physical Android phone is measured. iOS needs a Mac and Xcode. Web views can have an empty tree. phone-use is not the cheapest in tokens: in my one-run benchmark raw adb used fewer. [TBD: rerun with the shell tool]"
4. **Ask.** "If you have a real Android phone or an app where the snapshot comes back empty, I want the trace. AGPL-3.0, no telemetry. github.com/hridaya423/phone-use"

Reply 3 is the one a skeptical reader quotes. It is better coming from you.
