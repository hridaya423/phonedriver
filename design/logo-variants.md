# phone-use logo variants

Brief: drop the soft editorial look of commit 05ed22f. The brand must feel hard, heavy, fast and engineered, like a tool you trust to drive a real device. Registers drawn from: industrial signage, military equipment marking, motorsport, aerospace telemetry, hazard labelling.

Eight directions, each drawn as SVG and rendered with headless Chrome. Every file is `NN-slug.*` in `design/logo-variants/`:

| File | What it shows |
|---|---|
| `NN-slug.png` / `.svg` | Card, 1200 by 675: hero lockup, mono black, reverse, app tile at 4 sizes, mono avatar squares |
| `NN-slug-16.png`, `NN-slug-32.png` | The symbol rendered at real 16 px and 32 px |
| `NN-slug-tests.png` | Eight cells at 3x pixel zoom: 16 px on light, reverse, black, white; 32 px on light, reverse; 28 px app tile; 32 px black |
| `NN-slug-lockup.svg`, `NN-slug-symbol.svg` | Clean lockup and symbol sources |
| `contact-sheet.png` | All eight cards with their 16 and 32 px test strips |

Text in every SVG is outlines. Fonts are SIL OFL 1.1 and are in `NOTICE`. Variant 05's letters are drawn by hand on a grid and use no font.

Variants 01 to 08 are kept as explored. The chosen direction was refined when it became the system (see "The pick").

## 01, Slab punch, Big Shoulders Display

- **Architecture.** Lockup, symbol left of wordmark.
- **Typography.** Big Shoulders Display Black, caps, +1.5 tracking, hyphen in the accent.
- **Symbol.** Geometric reduction. A portrait slab with two 45 degree cuts (top right, bottom left) and a square punched through; a small accent square sits in the hole as the contact point.
- **Color tokens.** Ink `#0B0B0C`, bone `#EFEEE9`, accent `#FF4F00`. Single color black: hole stays empty. Reverse: bone slab on ink.
- **Application notes.** Strong at signage and README banners. At 16 px the slab is 9 px wide and reads as a dark chip with a hole. Square avatar needs a tile. Embroidery fine: two shapes.
- **Signals.** A device, a slab of steel, an impact.
- **Rejects.** Rounded phone icons, outline glyphs, friendly dev-tool softness.

## 02, Stencil crate, Saira Stencil One

- **Architecture.** Lockup with monogram emblem.
- **Typography.** Saira Stencil One, caps, +2 tracking, hyphen in the accent.
- **Symbol.** Monogram. PU inside a four-corner stencil frame; the frame gaps are the stencil bridges.
- **Color tokens.** Olive drab `#2D3827`, bone `#E4E0D0`, accent `#D9441A`.
- **Application notes.** Reads instantly as equipment marking. At 16 px the stencil bridges close up and PU turns to noise. Needs a bigger floor of 24 px.
- **Signals.** Military supply, ruggedness.
- **Rejects.** Clean software polish. Too costume-like for a developer tool.

## 03, Redline italic, Archivo Black wide

- **Architecture.** Lockup, tile left of wordmark.
- **Typography.** Archivo width 125 weight 900, caps, skewed 10.6 degrees, one horizontal slit through the word.
- **Symbol.** Letterform-derived. A red parallelogram with a P knocked out and cut by one slit.
- **Color tokens.** Black `#0A0A0A`, white, red `#FF1F1F`.
- **Application notes.** Fast and legible at 32 px. At 16 px the slit disappears and the P is a red blob. Single color works.
- **Signals.** Speed, motorsport.
- **Rejects.** Calm. Sits too close to esports and car-brand marks.

## 04, Telemetry reticle, JetBrains Mono

- **Architecture.** Lockup with a spec line.
- **Typography.** JetBrains Mono ExtraBold caps, +3.2 tracking, with a Medium spec line under a rule.
- **Symbol.** Abstract gesture. Four corner brackets and four edge ticks around a solid square.
- **Color tokens.** Near-black `#0A0C0E`, grey `#E9EAEA`, orange `#FF6A13`.
- **Application notes.** The spec line is unreadable below 200 px wide, so it needs a no-tagline lockup. The symbol holds at 32 px, goes soft at 16 px.
- **Signals.** Precision, instrumentation.
- **Rejects.** Heaviness. The reticle is the camera-focus icon of every camera app.

## 05, Cut caps, custom

- **Architecture.** Lockup: letterform-as-symbol plus a custom wordmark.
- **Typography.** Caps drawn on a 15 by 20 grid with the top-right and bottom-left corner of every letter cut at 45 degrees; the hyphen is a bar in the accent. Heavier than first drawn: vertical strokes 5, horizontals 4.
- **Symbol.** Letterform-derived. The P of the wordmark, its square counter filled with the accent.
- **Color tokens.** Ink `#0B0B0C`, bone `#EFEEE9`, accent `#FFD400`.
- **Application notes.** The best small-size mark of the eight: a P with a yellow square survives 16 px. Needs no font license. At wordmark size the square steps read as an 8-bit game font.
- **Signals.** Owned, engineered, cut steel.
- **Rejects.** Stock type. But also not heavy industrial: the wordmark is lighter than 01.

## 06, Data plate, Chakra Petch

- **Architecture.** Emblem, plate over a hazard-stripe band.
- **Typography.** Chakra Petch Bold, caps, +3 tracking.
- **Symbol.** Literal. An equipment data plate with chamfered corners, PU knocked out, over hazard stripes.
- **Color tokens.** Gunmetal `#16181A`, grey `#E7E7E3`, amber `#F5C400`.
- **Application notes.** Stripes turn to grey mud at 16 px. Great for stickers. The wordmark is thin for the brief.
- **Signals.** Warning label, serial plate.
- **Rejects.** Elegance. Reads as construction safety, not a driver of devices.

## 07, Selected line, Barlow Condensed

- **Architecture.** Lockup, frame left of wordmark.
- **Typography.** Barlow Condensed Black, caps, +1.5 tracking, hyphen in the accent.
- **Symbol.** Literal. A square frame of three text lines; the middle line is a solid band with text knocked out.
- **Color tokens.** Black `#0B0B0C`, white, cobalt `#1F3BFF`.
- **Application notes.** Clear at 32 px, mush at 16 px. Single color holds.
- **Signals.** Text on a screen, selection.
- **Rejects.** Mass. It reads as a document or list icon, not a device.

## 08, Shear, Oxanium

- **Architecture.** Lockup, two-slab symbol.
- **Typography.** Oxanium ExtraBold, caps, +2 tracking, hyphen in the accent.
- **Symbol.** Abstract gesture. Two slabs, the lower shifted right: one line before and after, the diff.
- **Color tokens.** Ink `#0B0B0C`, bone `#EFEEE9`, green `#00B368`.
- **Application notes.** Holds at 16 px. The green says "success", not "tough".
- **Signals.** Change, diff.
- **Rejects.** Weight. Resembles a pause or menu glyph at small size.

## The pick: 01, Slab punch

**Why it wins against the brief.** It is the only direction whose symbol and wordmark are both heavy: a solid slab and the blackest condensed face in the set. It reads as a device in silhouette (a tall slab, two cut corners, one punched square), so a stranger gets "something that goes through a phone" without a glyph of a phone. The punch is the product's one act, a tap, and it is the only color on the mark.

**Tests.**
- Silhouette: passes. A tall slab with two diagonal cuts has no near neighbour in the set. 05 reads as a location pin, 08 as pause.
- Small size: at real 16 px the slab and hole survive as a dark chip with a window. On a yellow tile it still reads (`design/brand/png/favicon-16.png`). 05 wins here by half a step.
- Single color: passes, the punch becomes a hole through the slab.
- Reverse: passes.
- Distinctiveness: stronger than 03, 04 and 07, which each have well-known cousins (esports, camera focus, list icon).

**Why it beat the runner-up, 05 Cut caps.** 05 has the better 16 px mark and owns its letters. But at wordmark size its square steps read as an 8-bit game font, and a P with a cut tail reads as a map pin. The brief asks for heavy and industrial; 01's wordmark carries far more mass. 05's one real advantage, the cut rule, was kept: the 45 degree top-right and bottom-left cut moved from 05's letters to every panel, box and plate in the system.

**Refinements made in the system** (so `design/brand/` differs from card 01):
- Slab widened from 36 to 38 and the hole from 14 to 16 units, so the tap is readable at 16 px.
- The hole is filled solid with the accent. The accent sits inside black, so it holds on both bone and ink. In the card it floated as a small square.
- Accent changed from orange `#FF4F00` to contact yellow `#FFD400`: black on yellow is 13.7:1, and yellow is the hi-vis color of hazard and equipment marking. Orange was the colour of the old brand.
- The hyphen is ink, not accent. One accent per lockup.

**Risks the owner should know.** Big Shoulders Black is a stock face (OFL), not custom. At 16 px the slab is not pixel-snapped, so its edges blur; a hinted 16 px redraw is a later task. Yellow on bone is 1.3:1, so yellow is never used for type or thin lines on bone, only as a field.
