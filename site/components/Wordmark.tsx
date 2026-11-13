import { WIDTH, wordPath } from "@/lib/brand";
import { CAP, GLYPH, LOCKUP } from "@/lib/chauffeur";

const WORD = wordPath();
const BOX = { w: LOCKUP.word + WIDTH + 4, h: LOCKUP.height };

export function Wordmark() {
  return (
    <svg className="wm" viewBox={`0 ${LOCKUP.top} ${BOX.w} ${BOX.h}`} width={BOX.w / 4} height={BOX.h / 4} aria-hidden="true" focusable="false">
      <g transform={`translate(0 ${LOCKUP.tileY}) scale(${LOCKUP.tile / 140})`}>
        <rect className="wm-t" width="140" height="140" rx="31.5" />
        <path className="wm-b" d={GLYPH.phone} />
        <path className="wm-k" d={GLYPH.island} />
        <g className="wm-cap">
          <g transform={GLYPH.cap}>
            <path className="wm-k" d={CAP.crown + CAP.visor} />
            <path className="wm-b" d={CAP.band + CAP.badge} />
          </g>
        </g>
      </g>
      <path transform={`translate(${LOCKUP.word} 0)`} d={WORD} fillRule="evenodd" />
    </svg>
  );
}
