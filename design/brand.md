# phone-use brand

The direction is hard, heavy, fast, engineered. The sheet is `.github/assets/brand-light.svg` and `brand-dark.svg`. The exploration is `design/logo-variants.md`.

## Files

`design/brand/`:

| File | Use |
|---|---|
| `phone-use-lockup.svg` | Primary lockup, for light grounds (ink and yellow) |
| `phone-use-lockup-reverse.svg` | Primary lockup, for dark grounds (bone and yellow) |
| `phone-use-lockup-black.svg`, `-white.svg` | Single colour; the punch is a hole |
| `phone-use-stacked*.svg` | Same four, stacked for square space |
| `phone-use-mark*.svg` | Symbol alone, same four |
| `phone-use-icon.svg`, `favicon.svg` | Yellow tile with the slab, for app icon, favicon and avatar |
| `png/` | Banner light and dark, favicon at 16, 32 and 180 px |

## Mark

A phone slab, 38 by 64 units, with the top-right and bottom-left corners cut by 12. A 16 unit square is punched through it at x 11 to 27, y 30 to 46, and filled with contact yellow. The slab is ink on light grounds and bone on dark grounds. Minimum size: 16 px tall as the tile, 24 px tall as the bare slab. Lockup minimum width 96 px. Clear space: the width of the punch (16 units) on every side.

## The cut

Every panel, box and plate has its top-right and bottom-left corners cut at 45 degrees. No radius anywhere. The app tile is square; the platform applies its own mask.

## Colour

| Token | Hex | Role |
|---|---|---|
| Ink | `#0B0B0C` | Type and slab on light; ground on dark |
| Bone | `#F2F2EE` | Ground on light; type and slab on dark |
| Contact | `#FFD400` | The tap, and fields that carry ink type. Never type or thin lines on bone |
| Steel | `#6E6E72` | Secondary text on bone (4.5:1); on ink use `#9A9A96` (7.0:1) |

Diff colours, for terminal output and charts only: added `#00793F` on bone and `#2FD47F` on ink; removed `#C4160C` and `#FF4A3D`; changed `#7A5C00` and contact yellow.

Contrast: ink on bone 17.5:1, ink on contact 13.7:1, contact on bone 1.3:1 (fields only).

One yellow thing per surface. No gradients, shadow, glow or texture.

## Type

Headings and wordmark: Big Shoulders Display, weight 800 to 900, capitals only. Code, labels, numbers: JetBrains Mono, weights 500 and 800. Both are SIL OFL 1.1, free for web and embedding, and are in `NOTICE`. On the web fall back to `Impact, "Arial Narrow", sans-serif` for headings and `ui-monospace, Menlo, monospace` for code. Scale: 13, 15, 17, 22, 38, 70 px.

## Imagery and iconography

No photographs, no illustration, no stock. Technical drawings only: terminal output, the loop diagram, the speed chart. Icons are filled, square-cornered, one weight, on a 24 px grid, with the cut applied to the one corner pair when a shape has corners.

## Motion

Cuts, not fades. Elements slide in along the cut axis (up and to the right) in 120 ms with `cubic-bezier(.2,0,0,1)`; the punch lights last. Nothing loops. Honour `prefers-reduced-motion` by showing the end state.

## Assets in the README

The banner, loop, speed chart, divider and brand sheet each come in light and dark and are chosen with `<picture>`. They are generated; every number in the chart comes from `docs/bench/m1.md`.
