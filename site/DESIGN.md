# The home page below the hero: one route

The hero is fixed. This file is the plan for everything under it. Decision D44 in `docs/decisions.md` is the short form.

## Brief

- Subject: phonedriver, a CLI and MCP server that lets an AI agent see and operate a phone.
- Audience: developers who already run Claude Code, Codex or Cursor.
- Job of the page: paste one prompt into your agent (set it up).
- Complaint this pass answers: the sections under the hero have no linkage and feel detached and cold; the nine-job list should be one accordion; the 6.9x figure should come before "what the model sees".
- Viewed mostly in dark mode. Dark is designed first, light second, both checked.

## Concept card

```
Idea:        One route. The orange dotted line that carries your order to the phone in the hero
             keeps going. Every section is a stop on it, opened by the next thing you would say
             to your agent, typed along the line. The line lands on the thing that answers you.
Metaphor:    A drive with a chauffeur. You speak, the driver takes the route. It is in the name
             and in the mark (cap, gloves).
Signature:   The read. A real screenshot is scanned; above the scan line the picture becomes a
             dither of glyphs, each element sheds its dots across the gap, and they settle as
             the 14 lines the model is sent.
Emotion:     Somebody capable is handling this for you.
Motion verb: Travel like a signal: along a line, arrive, settle with a small spring. Nothing fades up.
Screenshot:  "Doesn't that eat my tokens?" arriving on a 6.9x the height of the viewport's third,
             with the three dot fields mid-fill.
```

Anchor: the hero already draws a route (`.hx-route`), a weave of glyph dots and a chauffeur. Nothing new is invented; the three are carried down the page.

Three concepts were weighed. (1) One route, chosen: it reuses the hero's own device, so the page reads as one piece. (2) A sticky phone that changes screen per chapter: strong, but every screen would have to be a real capture at phone aspect and only two exist. (3) 636 dots that re-form into every figure: a particle trope, and the dots only mean something in the token section.

## The argument, in order

| # | id | You say (on the route) | The page answers | Proof, and what the line lands on |
|---|----|------------------------|------------------|-----------------------------------|
| 0 | hero | Order me a large pepperoni pizza… | Your AI can use your phone. | the recorded run |
| 1 | `real-jobs` | Doesn't that eat my tokens? | 6.9x fewer tokens than mobile-mcp on nine real phone jobs. | the 6.9x; dot fields; the nine jobs in one accordion |
| 2 | `what-the-model-sees` | How is it that cheap? | It reads the screen as text. | the snapshot pane; the read |
| 3 | `speed` | And is it quick? | 170x faster than adb at reading the screen. | latency balls pane |
| 4 | `asking-before-it-pays` | Can it spend my money? | It asks you before it taps Pay. | the recorded approval prompt; the raised palm |
| 5 | `hidden-display` | Do I have to put my phone down? | Your screen stays yours. | the hidden screen capture |
| 6 | `how-it-works` | What is it made of? | How it works. | agent, daemon, helper |
| 7 | `limits`, `faq` | Anything I should know? | Limits and questions. | the list |
| 8 | `install` | Fine. Set it up. | Give your agent a phone. | the setup prompt pane |

Setup moves from second place to the end: the route starts with someone else's order and ends with yours. To keep the action one click away the whole way down, a dock (fixed bar) appears once the hero has left: where you are on the route, and "Copy the setup prompt".

## System (all from the hero)

- Ground: `--page`. No full-bleed bands between stops; figures are ink panes. The last stop is the one ink band.
- Type: JetBrains Mono 800 headings, mono labels, Inter Display prose. Display numerals up to 320px.
- Colour: ink, paper, one orange (`--or`) for signal only: the route, the spoken line, the winning number, lit dots.
- Shape: square corners. Dots are the only round thing.
- Route: 3px round dots at 7px pitch, mono 600 15px words knocked out of the line, arrowhead, a packet that falls the last stretch.
- Weave: the hero's ribbon of glyph points, one per stop, in the air above the stop. It unfurls as the line travels and twists with scroll.
- Chauffeur: the open palm at the approval stop, the full mark at the last stop.
- Motion: `--out` for arrivals, `--spring` for settles, linear for travel. Every stop plays once, when its line lands.

## Engine

- `lib/frame.ts`: one `requestAnimationFrame` loop for everything below the hero.
- `lib/journey-engine.ts`: the driver. Per stop: route geometry from the DOM, progress from scroll (monotonic, eased), words typed along the path, the weave's `enter`, the landing (sets `data-landed`, removes `data-wait`, sets `data-enter`, fires `life` on the stop's `[data-life]` nodes). Also the dock state.
- `lib/weave.ts`: one fixed WebGL canvas for all stops.
- `lib/views-engine.ts`: the read (dither + shed) and the hover link.
- `lib/dots-engine.ts`, `race-engine.ts`, `type-engine.ts`: unchanged roles.
- Without JavaScript or with reduced motion: every stop is its final state, the spoken line is plain text above the heading, no canvas.

## Stops, desktop

```
1  [route ................................. ↘ ]        4  h2 lede            [pane: approval ]
   h2 lede                          6.9x              facts                [   + raised palm]
   field ●●●●●●●                                   5  h2 lede      (drop)
   field ●●●●●●●●●●●●                                 [ your screen      ][ hidden screen ]
   field ●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●          6  h2   | agent ··· daemon ··· helper
   ▸ The nine jobs, one by one                        lede | works with | fourteen tools
2  h2 lede                    (drop)               7  h2   | limits, then questions (details)
   [shot]  ···shed···  [pane 14 lines]             8  ink band: h2 + links | [setup pane] + mark
           [pane: one screen later]
3  h2 lede                    (drop)
   [ink pane: 2x2 latency balls           ]
   on a real phone: 32x  61x
```

Phone: one column; the line runs across above the heading, hugs the right edge and lands on the figure's top right.

## Inner pages

Each inner page opens as one stop of the same route, through `components/PageTop.tsx`: the spoken line types itself on load (`data-intro`, a timer in place of scroll) and lands on the page's figure or on the first row of its list.

```
[route "Show me how to set it up." ............................. ↘ ]
crumbs
h1                                                               |
lede                                                             v
┊ Start ─────────────────────────────────────────────────────────
● Install            Set up phonedriver by pasting ...   start here
● Claude Code        Add phonedriver to Claude Code ...
```

| page | line | lands on |
|---|---|---|
| docs | Show me how to set it up. | the Install row, tagged "start here" |
| compare | Why not one of the others? | the first row and its number (170x) |
| blog | What did you learn building it? | the first post and its number |
| benchmarks | Prove it. | 6.9x in orange, beside the heading |
| 404 | Open this page. | the open palm |

The docs, compare and blog lists sit inside the stop on a rail (`.rail`): a dotted orange line down the left, one round stop per row, the stop fills on hover and focus. Compare and blog rows end in that page's one number (`.row-fig`), computed from the benchmark data.

Pages change with a view transition (`ViewTransition` in `app/layout.tsx`, class `vt-page`): old page fades in 0.14 s, new page is uncovered from the top, one viewport in 0.5 s on `--out`. Below 760px the header has a Menu button that opens a sheet (Popover API).

## Checklist

- [x] frame hub, journey engine, weave
- [x] page.tsx in the new order, Route markup, dock
- [x] home.css for the stops, dark first
- [x] the read
- [x] nine jobs in one accordion
- [x] palm, final stop
- [x] 390 / 768 / 1440, light and dark, reduced motion, no JS
- [x] build, check, outside critic (7.1), fix list
- [x] inner page tops, phone menu, page transition
- [x] second outside critic (7.65), fix list
- [x] third outside critic (7.65), fix list
- [x] fourth outside critic (7.8). Stopped here: what is left under 8 is the hero, which is not to be changed, and pictures the inner pages do not have.
- [x] remove one accessory: the pointing glove on inner-page rows
