# phone-use brand

phone-use is built on the [Sodium](https://design.how/systems/sodium) design system (MIT, Arlan Marat, design.how). Sodium is compact and ruled: white and warm near-black grounds, 1px rules, square corners, one weight, one yellow. The brand takes its tokens, type and line rules from Sodium unchanged and adds a mark, a lockup and image layouts. Where this file and Sodium disagree, Sodium wins.

The images are generated. `python3 design/build/build.py` rebuilds the banners, loop, speed chart, brand sheet, logo files and favicons from the tokens in `design/build/sodium.py`; text is converted to outlines with the fonts in `design/build/fonts/`. `design/mark-options.png` shows the three marks that were explored.

## Files

| File | Use |
|---|---|
| `design/logo/mark.svg`, `mark-dark.svg` | The mark on a light or dark ground |
| `design/logo/mark-black.svg`, `mark-white.svg` | One colour; the middle row is solid |
| `design/logo/lockup.svg`, `lockup-dark.svg` | Mark and wordmark |
| `design/logo/favicon.svg`, `app-icon.svg` | Ink tile with the mark, for favicon, app icon and avatar |
| `design/logo/png/favicon-16.png`, `-32`, `-180`, `-512` | The tile as PNG; 16 and 32 are pixel-snapped |
| `.github/assets/banner-light.svg`, `banner-dark.svg` | README banner, 1280 by 400 |
| `.github/assets/social-preview.png` | GitHub social preview, 1280 by 640 |
| `.github/assets/loop-*.svg`, `speed-*.svg`, `brand-*.svg` | Loop diagram, speed chart, brand sheet, each light and dark |

## Mark

A phone frame, 8 units wide and 16 tall on a 16-unit grid, drawn in rule weight. Two rules cut the screen into three rows. The middle row is filled with sodium yellow: the selected ref, the thing that was tapped. There are no curves, no diagonals, no shadow and no gradient.

- Rule weight is 1 unit at sizes below 48 px (1 px at 16 px) and half a unit from 48 px up, so the stroke stays a rule at large sizes. Same geometry, two cuts.
- At 16 px every edge sits on a whole pixel. Draw the 16 and 32 px files from the grid, never by scaling a larger file.
- Frame and rules are `--text-1` (`#161512`) on light and `#f5f3ee` on dark. The row is `--accent` (`#ffd400`) in both. Single colour: ink on the frame, rules and row, with the row solid.
- The tile (favicon, app icon) is `#161512` with the frame inset one unit from the top and bottom and drawn in `#f5f3ee`. It reads on light and dark browser chrome.
- Minimum size: 16 px tall. Clear space: one quarter of the mark's height on every side.
- Why not the others: a lone yellow cell on a frame (A) is a dot at 16 px and says nothing about text; brackets (C) read as a ref but not as a phone. The ruled rows (B) are the snapshot tree and the selection wash, so the mark is the product's one gesture, select a row.

## Wordmark and lockup

`phone-use`, lowercase, Inter Display 400, tracking -0.035em, no capitals and no bold. Inter reads as a name; Commit Mono is kept for refs, commands and labels, so the mark never competes with the CLI voice. Wordmark size is 7/8 of the mark's height, its x-height centred on the mark, and the gap from the frame's right edge is 3/8 of the mark's height. Minimum lockup width is 96 px. Name the project in prose as `phone-use`, in code font.

## Colour

Tokens are Sodium's. Use roles, never new hex values.

| Role | Light | Dark | Use |
|---|---|---|---|
| `--page` | `#ffffff` | `#141412` | Ground |
| `--panel` | `#fafaf7` | `#1c1b19` | Panel header rows |
| `--well` | `#f4f3ee` | `#22211e` | Code surfaces |
| `--line` | `#e2dfd5` | `#2c2a25` | Frames and column rules |
| `--line-faint` | `#efede6` | `#22211e` | Rules between rows |
| `--text-1` | `#161512` | `#f5f3ee` | Type and the mark |
| `--text-2` | `#55524a` | `#c4c0b6` | Secondary text |
| `--text-3` | `#6b675e` | `#a19c90` | Refs, captions, axis labels |
| `--selected` | `#fff4bf` | `#3a3107` | The selected row (the tapped ref) |
| `--accent` | `#ffd400` | `#ffd400` | The mark's row, the primary action, one chart series |
| `--accent-text` | `#7a5d00` | `#ffd400` | Yellow-family text, such as the `~` of a changed line |
| `--ok` | `#2c6a3a` | `#8cc79a` | Success and the new value in a diff |
| `--data-1` to `--data-5` | `#ffd400`, `#8c7a3c`, `#9a937c`, `#5b4a1a`, `#2e3a4a` | `#ffd400`, `#ebdc9c`, `#8f8a76`, `#8a6d1f`, `#5d7290` | Chart series |

Yellow is `--data-1` for phone-use in charts and `--data-5` for the other series. Yellow on white is 1.4:1, so yellow is never text on light: use `--accent-text`. Yellow appears once per composition, on the thing that was selected. In the banner the mark's row and the selected ref are the same idea, a selected row, and are the only yellow.

## Type

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

Sentences and headlines are Inter Display in sentence case. Mono capitals are for words a person scans (a label, an axis, a column head), never a sentence. Refs, commands and output lines are Commit Mono as written, with the product's own punctuation. The banner, loop and chart draw Sodium at 1.5x (banner) and 1.2x (loop, chart) so the text stays readable when GitHub scales the image to about 830 px.

## Layout

- 4px grid. Inset 12px. Header rows 38px, rows 40px.
- Radius 0. Every line is 1px. A frame is only a region's outer edge; rules divide its parts (`--line-faint` between rows, `--line` between columns and under the header).
- A name goes in a panel header row, never in a small label above a headline.
- Several numbers sit on one ruled row (a status strip), not in equal cards.
- A selected row takes the `--selected` wash. No coloured bar on an edge.
- A status is a square mark and a word: a filled square for ok.
- A dot gutter (4px pitch) fills an empty gutter 20px or wider, at most once per image. It is never behind content.
- Charts: bars are flat fills starting on the axis, values printed at the bar end, a log axis with 1px ticks, no legend box.

## Voice in images

Use the product's own output, verbatim from the repo: snapshot lines, `~` diffs, measured figures from `docs/bench/m1.md`. Say what happens, in plain words. No hype words, no emoji, no em dash, no invented numbers.

## Do

- Put the mark on `--page` or on the ink tile, with clear space.
- Use the same two typefaces and one weight in every image.
- Check every image at 830 px wide, light and dark, before committing.

## Do not

- Round a corner, add a shadow, glow, gradient or blur.
- Draw a grid or dot field behind content.
- Make the mark yellow all over, put yellow text on white, or add a second yellow thing next to the first.
- Bold the wordmark, set it in capitals, or recolour the frame.
- Mark a row with a coloured edge bar, or a status with a round dot.
- Use the old direction (slab, cut corners, hi-vis fields, Big Shoulders). It is gone.
