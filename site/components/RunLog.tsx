import run from "@/lib/run.json";

type Row = { t: number; k: string; text?: string; name?: string; lines?: string[]; more?: number };

const ROWS = run.rows as Row[];
const video = (r: number) => (r <= run.lapse ? r / run.speed : run.lapse / run.speed + (r - run.lapse));

function Line({ text }: { text: string }) {
  const diff = /^([~+-]) (.*)$/.exec(text);
  return (
    <span className="lg-l">
      {diff ? (
        <>
          <i>{diff[1]}</i> {diff[2]}
        </>
      ) : (
        text
      )}
    </span>
  );
}

function Entry({ row }: { row: Row }) {
  switch (row.k) {
    case "you":
      return (
        <li className="lg-you" data-t={row.t}>
          <i>❯</i>
          <p>{row.text}</p>
        </li>
      );
    case "say":
      return (
        <li className="lg-say" data-t={row.t}>
          <i>●</i>
          <p>{row.text}</p>
        </li>
      );
    case "call":
      return (
        <li className="lg-call" data-t={row.t}>
          <b>{row.name}</b> {row.text}
        </li>
      );
    case "ask":
      return (
        <li className="lg-ask" data-t={row.t} data-focus="decline">
          {row.lines!.map((l, i) => (
            <span className="lg-l" key={i}>
              {l}
            </span>
          ))}
          <span className="lg-l lg-pick">
            <i data-at="answer">❯</i>✔ Your answer: Allow
          </span>
          <span className="lg-l lg-pick">
            <i data-at="accept">❯</i>
            <span>Accept</span>
            <i data-at="decline">❯</i>
            <span className="lg-chose">Decline</span>
          </span>
        </li>
      );
    default:
      return (
        <li className={row.k === "declined" ? "lg-reply lg-no" : "lg-reply"} data-t={row.t}>
          {row.lines!.map((l, i) => (
            <Line text={l} key={i} />
          ))}
          {row.more ? <span className="lg-l lg-more">… +{row.more} lines</span> : null}
        </li>
      );
  }
}

export function RunLog() {
  const last = ROWS[ROWS.length - 1].t;
  return (
    <div className="hx-log" role="group" aria-label="A recorded Claude Code session driving a phone through phonedriver">
      <p className="lg-head">
        <span>Claude Code · Opus 5.5</span>
        <span>recorded run</span>
      </p>
      <div className="lg-view" tabIndex={0} role="log" aria-label="What the agent said, each phonedriver call, and the start of each reply">
        <p className="lg-idle" aria-hidden="true">
          ❯ <i />
        </p>
        <ol>
          {ROWS.map((row, i) => (
            <Entry row={row} key={i} />
          ))}
          <li className="lg-done" data-t={last + 0.1}>
            ✻ {run.worked}
          </li>
        </ol>
      </div>
      <div className="lg-bar">
        <p className="lg-now">
          <span className="lg-chap">{run.chapters[run.chapters.length - 1][1]}</span>
          <span className="lg-time">
            <span className="lg-clock">0:00</span>
            <span className="lg-speed">{run.speed}×</span>
          </span>
        </p>
        <div className="lg-ctl">
          <button className="lg-play" type="button" aria-label="Play the recording">
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path className="i-play" d="M4 2.5v11l9.500-5.500Z" />
              <path className="i-pause" d="M3.500 2.500h3.200v11H3.500ZM9.300 2.500h3.200v11H9.300Z" />
              <path className="i-again" d="M13 8a5 5 0 1 1-1.600-3.670M13 2v3h-3" />
            </svg>
          </button>
          <span className="lg-track">
            <input type="range" min={0} max={run.length} step={0.1} defaultValue={0} aria-label="Position in the recording" />
            {run.chapters.slice(1).map(([t]) => (
              <i key={t as number} style={{ left: `${(video(t as number) / run.length) * 100}%` }} />
            ))}
          </span>
        </div>
      </div>
      <p className="lg-note">
Screen recording and session log of one run on {run.date}: dominos.co.uk in Chrome on an Android emulator, {run.calls} calls in {run.worked.replace("Worked for ", "")}. Shown at {run.speed}× until phonedriver asks, then in real time.
      </p>
    </div>
  );
}
