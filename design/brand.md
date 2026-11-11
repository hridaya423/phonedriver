# phonedriver brand

The identity is decisions D38, D39 and D40: an invisible chauffeur, drawn lettering, and a halftone display treatment. The README charts (`loop`, `speed`, `jobs`, `hero`) still use the Sodium tokens below until they are redrawn.

## The idea: the invisible chauffeur

phonedriver drives a phone for an agent you cannot see. The mark is that driver: a peaked cap floating over a phone, and a white glove that taps it. Nobody is in between. The glove is drawn as the pointing-hand cursor wearing a chauffeur's glove, because the hand cursor is how a screen already draws "something is using this".

## The chauffeur

Its source is `site/lib/chauffeur.ts`; nothing else draws it.

- **Cap**: a peaked cap seen from the front, tilted 7° anticlockwise: a winged crown, a band in the accent, a chin strap with two buttons, a visor with one shine line, and a phone-shaped badge.
- **Phone**: a plain squircle slab with an island pill. No face, no eyes. In the full cut it shows one switch, turned on.
- **Pointing glove**: the hand cursor's outline (index up, three curled fingers, thumb out) with a driving glove's details: a knuckle hole per finger, a keyhole opening on the back, a strap with a snap, a flared cuff. White with an ink keyline 15% of a finger wide; the strap is orange. The holes, the keyhole and the cuff's opening are ink, because the glove is empty.
- **Open palm** (site hero only): the same glove turned over, fingers straight, thumb on the other side, two palm creases in place of the knuckle holes and keyhole. It means "held, waiting for the human" and appears only while an approval question is open.
- **Holding fingers** (site hero only): four white fingertips wrapped over the phone's edge from behind, each with a fingertip seam.
- Three cuts, chosen by size. **Full** (96 px and up): cap, phone, switch, pointing glove. **Text** (48 to 95 px): cap with its details and phone. **Micro** (under 48 px): cap without strap, shine or buttons, and phone.
- Icon: the figure in ink on an orange tile (corner 22.5%) with a paper band; the phone rises from the tile's bottom edge and the glove reaches in from the corner. Used for the favicon (micro), app icons and avatars (full).
- Lockup: the icon tile, then the wordmark. The tile is 160 units on the lettering's 180-unit line. The bare cap-over-phone figure is never set next to the wordmark: at that size it reads as a letter `i`.
- On dark, without a tile: cap and phone in paper, the visor dark with a paper rim.

On the site the hero's phone is the same figure at full size, and its screen is a recording of a real run (D42). The chauffeur holds the phone up from behind the way a driver holds a name board at arrivals: the cap floats above it and gloved fingers wrap both edges. While it waits it drums its fingers. When the run starts the cap dips each time the agent reads the screen, and one hand lets go and comes round to tap where the log says a tap landed. At the payment the tap is held: the hand pulls back and opens into a raised palm and the cap straightens until the recorded answer arrives. The human in the recording declines, so the hand goes back to holding the phone. Hovering the header lockup tips its cap.

Do not: give the phone a face or eyes; draw a body, head or arms; colour the gloves anything but white; drop the keyline; tilt the cap the other way; use the full cut under 96 px.

## Lettering

`phonedriver` is drawn, not typeset. Its source is `site/lib/brand.ts`.

- Units: x-height 100, ascender 140, descender 40, stems 24, horizontal strokes 19, bowl width 80, letter gap 13.
- Bowls are squircle-cornered rectangles (corner 38, counter corner 13, curve tension 0.7), the shape of a phone screen. The chauffeur's shapes use the same squircle.
- `p` and `d` are the same glyph turned 180°. The `i` has a round tittle.
- Lowercase only. Never set the name in a font to stand in for the lettering; in prose write `phonedriver` in code font.

## Display treatment

For the README banner, social images and the site footer, the wordmark is drawn as a halftone: dots grow left to right (radius ramp over the first 62% of the width) and the solid letters fade in from 50% to 76%. Dots in roughly the first third are orange, mixed into ink with a 4x4 Bayer dither, and are clipped to the letter shapes. It says what the product does, read a screen's pixels as text. It plays once (banner on load, footer when it scrolls into view) and never under reduced motion. Do not use it under about 240 px wide.

## Colour

Ink `#0b0b0c`, paper `#f3f2ed`, orange `#ff5b14`. The site's other tokens are in `site/app/globals.css`.

## Files

`cd site && node scripts/brand.ts` writes all of these.

| File | Use |
|---|---|
| `design/logo/chauffeur.svg`, `chauffeur-dark.svg` | The full figure without a tile, on light or dark |
| `design/logo/icon.svg`, `png/icon-180.png`, `-512` | Icon tile, full cut |
| `design/logo/png/icon-16.png`, `-32` | Icon tile, micro cut |
| `design/logo/lockup.svg`, `lockup-dark.svg` | Icon tile and wordmark |
| `design/logo/display.svg`, `display-dark.svg` | Halftone wordmark, static |
| `site/public/favicon.svg` | Site favicon, micro cut |
| `.github/assets/banner-light.svg`, `banner-dark.svg` | README banner, 1280 by 400, animated once, icon beside the tagline |
| `.github/assets/social-preview.png`, `site/public/assets/og-home.png` | GitHub social preview (1280 by 640) and the home page's share image (1200 by 630) |
| `.github/assets/demo.gif` | README demo: a real Claude Code run, recorded by `design/build/demo_record.py`, composited by `design/build/demo_compose.py` |
| `.github/assets/loop-*.svg`, `speed-*.svg`, `jobs-*.svg`, `hero-*.svg` | Charts, still Sodium, built by `python3 design/build/build.py` |

## Sodium (charts only)

The charts are built on the [Sodium](https://design.how/systems/sodium) design system (MIT, Arlan Marat, design.how): white and warm near-black grounds, 1px rules, square corners, one weight. `design/build/build.py` builds them from the tokens in `design/build/sodium.py`; text is converted to outlines with the fonts in `design/build/fonts/`. The rest of this file describes those charts.

### Chart colour

Tokens are Sodium's. Use roles, never new hex values.

| Role | Light | Dark | Use |
|---|---|---|---|
| `--page` | `#ffffff` | `#141412` | Ground |
| `--panel` | `#fafaf7` | `#1c1b19` | Panel header rows |
| `--well` | `#f4f3ee` | `#22211e` | Code surfaces |
| `--line` | `#e2dfd5` | `#2c2a25` | Frames and column rules |
| `--line-faint` | `#efede6` | `#22211e` | Rules between rows |
| `--text-1` | `#161512` | `#f5f3ee` | Type |
| `--text-2` | `#55524a` | `#c4c0b6` | Secondary text |
| `--text-3` | `#6b675e` | `#a19c90` | Refs, captions, axis labels |
| `--selected` | `#fff4bf` | `#3a3107` | The selected row (the tapped ref) |
| `--accent` | `#ffd400` | `#ffd400` | One chart series |
| `--accent-text` | `#7a5d00` | `#ffd400` | Yellow-family text, such as the `~` of a changed line |
| `--ok` | `#2c6a3a` | `#8cc79a` | Success and the new value in a diff |
| `--data-1` to `--data-5` | `#ffd400`, `#8c7a3c`, `#9a937c`, `#5b4a1a`, `#2e3a4a` | `#ffd400`, `#ebdc9c`, `#8f8a76`, `#8a6d1f`, `#5d7290` | Chart series |

Yellow is `--data-1` for phonedriver in charts and `--data-5` for the other series. Yellow on white is 1.4:1, so yellow is never text on light: use `--accent-text`. Yellow appears once per composition, on the thing that was selected. The demo GIF follows the same rule: its one yellow is the diff line, and its first frame is the finished state.

### Type

One weight, 400, everywhere. No bold, no italic.

| Role | Face | Size and line | Tracking |
|---|---|---|---|
| Display | Inter Display | 56/1.02 | -0.04em |
| Title | Inter Display | 32/1.1 | -0.035em |
| Figure | Inter Display | 40/1 | -0.035em |
| Body | Inter Display | 15/24 | 0 |
| Label | Commit Mono, capitals | 13/16 | 0.05em |
| Label small | Commit Mono, capitals | 12/16 | 0.06em |
| Code | Commit Mono | 13/20 | 0 |

Sentences and headlines are Inter Display in sentence case. Mono capitals are for words a person scans (a label, an axis, a column head), never a sentence. Refs, commands and output lines are Commit Mono as written, with the product's own punctuation. The loop and charts draw Sodium at 1.2x so the text stays readable when GitHub scales the image to about 830 px.

### Layout

- 4px grid. Inset 12px. Header rows 38px, rows 40px.
- Radius 0. Every line is 1px. A frame is only a region's outer edge; rules divide its parts (`--line-faint` between rows, `--line` between columns and under the header).
- A name goes in a panel header row, never in a small label above a headline.
- Several numbers sit on one ruled row (a status strip), not in equal cards.
- A selected row takes the `--selected` wash. No coloured bar on an edge.
- A status is a square mark and a word: a filled square for ok.
- A dot gutter (4px pitch) fills an empty gutter 20px or wider, at most once per image. It is never behind content.
- Charts: bars are flat fills starting on the axis, values printed at the bar end, a log axis with 1px ticks, no legend box.

### Voice in images

Use the product's own output, verbatim from the repo: snapshot lines, `~` diffs, measured figures from `docs/bench/history/m1.md`. Say what happens, in plain words. No hype words, no emoji, no em dash, no invented numbers.

### Do

- Use the same two typefaces and one weight in every image.
- Check every image at 830 px wide, light and dark, before committing.

### Do not

- Round a corner, add a shadow, glow, gradient or blur.
- Draw a grid or dot field behind content.
- Put yellow text on white, or add a second yellow thing next to the first.
- Mark a row with a coloured edge bar, or a status with a round dot.
- Use the old direction (slab, cut corners, hi-vis fields, Big Shoulders). It is gone.
