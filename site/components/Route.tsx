/** One stop's spoken line. The paragraph is the text itself; the SVG is drawn and typed by `journey-engine`. */
export function Route({ n, say, short }: { n: number; say: string; short?: string }) {
  const id = `rt${n}`;
  return (
    <>
      <svg className="rt" aria-hidden="true" data-say={say} data-short={short}>
        <defs>
          <path id={`${id}-p`} />
          <text id={`${id}-t`} className="rt-t">
            <textPath href={`#${id}-p`} />
          </text>
          <mask id={`${id}-m`} maskUnits="userSpaceOnUse">
            <use href={`#${id}-p`} className="rt-reveal" />
            <use href={`#${id}-t`} className="rt-knock" />
          </mask>
        </defs>
        <use href={`#${id}-p`} className="rt-dots" mask={`url(#${id}-m)`} />
        <use href={`#${id}-t`} className="rt-text" />
        <path className="rt-head" d="M-7 -4.5 0 0 -7 4.5" />
        <circle className="rt-packet" r="4" />
      </svg>
      <p className="rt-say wrap">{say}</p>
    </>
  );
}
