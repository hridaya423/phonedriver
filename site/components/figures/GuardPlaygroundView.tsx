"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { mountHold } from "@/lib/figures";
import { Fig } from "./kit";
import { OPT_IN, WHAT, classify, held, type Category, type Lists } from "./guard-rules";

const TRY = ["Buy with GPay", "Place order · $34.10", "Send $20.00 to Sam", "Subscribe · $9.99/month", "Delete account", "提交订单", "Comprar ahora", "Send", "Add to cart", "Subscribe", "Buy again", "Feed me now", ""];
const NAME: Record<Category, string> = { payment: "Payment", account: "Account", send: "Send", delete: "Delete", install: "Install" };

const Cart = () => (
  <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
    <path d="M3 4h2.2l2.3 10.2a1 1 0 0 0 1 .8h8.9a1 1 0 0 0 1-.8L20 7H6.3" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" />
    <circle cx="9.5" cy="19" r="1.6" fill="currentColor" />
    <circle cx="17" cy="19" r="1.6" fill="currentColor" />
  </svg>
);

export function GuardPlaygroundView({ lists }: { lists: Lists }) {
  const [label, setLabel] = useState(TRY[0]);
  const [ask, setAsk] = useState<Category[]>([]);
  const [tap, setTap] = useState(0);
  const stage = useRef<HTMLDivElement>(null);
  const v = useMemo(() => classify(lists, label), [lists, label]);
  const stop = held(v.category, ask);
  const missed = !label.trim();
  const optIn = v.category !== null && OPT_IN.includes(v.category);

  useEffect(() => {
    const el = stage.current;
    return el ? mountHold(el) : undefined;
  }, []);

  const pick = (l: string) => {
    setLabel(l);
    setTap((n) => n + 1);
  };
  const flip = (c: Category) => {
    setAsk((a) => (a.includes(c) ? a.filter((x) => x !== c) : [...a, c]));
    setTap((n) => n + 1);
  };

  const why = missed
    ? "The guard reads labels, and this button has none. No prompt does not mean safe."
    : v.category === null
      ? `Not guarded: ${v.why}.`
      : `${NAME[v.category]}: ${v.why}. ${stop ? (optIn ? `Held because PHONEDRIVER_ASK has ${v.category}.` : "Always held.") : `Held only with PHONEDRIVER_ASK=${v.category}.`}`;

  return (
    <Fig
      id="guard-playground"
      title="Would the guard hold it?"
      controls={
        <div className="gp-ask" role="group" aria-label="PHONEDRIVER_ASK">
          <span>PHONEDRIVER_ASK</span>
          {OPT_IN.map((c) => (
            <button type="button" key={c} className="fx-btn" aria-pressed={ask.includes(c)} onClick={() => flip(c)}>
              {c}
            </button>
          ))}
        </div>
      }
      caption={
        <>
          This runs the guard&apos;s own rules: <code>classify</code> from <code>crates/cli/src/daemon/guard.rs</code>, ported line for line, with the word lists read from{" "}
          <code>guard/labels.rs</code> when this page was built. It agrees with the guard on all 277 labels of the guard&apos;s eleven-language test. Payments and deleting an
          account are always held; sends, deletes and installs only when you ask.
        </>
      }
    >
      <div className="gp">
        <div className="gp-phone" ref={stage} data-halt={stop ? "" : undefined}>
          <div className="gp-screen" aria-hidden="true">
            <p className="gp-app">Checkout</p>
            <p className="gp-item">1 × Large pepperoni, for collection</p>
            <p className="gp-row">
              <span>Total</span>
              <b>£24.99</b>
            </p>
            <div className={stop ? "gp-btn held" : "gp-btn"} key={tap} data-tap="">
              {missed ? <Cart /> : label}
            </div>
          </div>
          <canvas className="rp-fig gp-palm" aria-hidden="true" />
        </div>
        <div className="gp-side">
          <p className={missed ? "fx-verdict bad" : stop ? "fx-verdict good" : "fx-verdict"} aria-live="polite">
            <b className="gp-word" key={`${label}${ask.join()}`}>
              {missed ? "Tapped. Missed." : stop ? "Held." : "Tapped."}
            </b>
            <span>{why}</span>
          </p>
          <div className="fx-term gp-term">
            <p className="cmd">$ phonedriver tap {missed ? "@e14" : `"${label}"`}</p>
            {stop && v.category ? (
              <>
                <p className="bad" key={`e${tap}`} data-in="">
                  error: &quot;{label}&quot; {WHAT[v.category]}: it needs the human&apos;s approval
                </p>
                <p className="dim">hint: Ask the user in chat whether to tap it; tap again with `--autonomous` only when they say yes or have granted full autonomy.</p>
                <p className="dim">exit 5</p>
              </>
            ) : (
              <p className="ok" key={`o${tap}`} data-in="">
                ok tapped @e14{missed ? "" : ` "${label}"`}
              </p>
            )}
          </div>
        </div>
      </div>
      <div className="gp-try">
        <label className="gp-input">
          <span className="vh">Type a button label</span>
          <input
            value={label}
            placeholder="Type a button label"
            onChange={(e) => setLabel(e.target.value)}
            onBlur={() => setTap((n) => n + 1)}
            spellCheck={false}
            autoComplete="off"
          />
        </label>
        <ul className="gp-chips">
          {TRY.map((l) => {
            const c = classify(lists, l).category;
            return (
              <li key={l || "icon"}>
                <button type="button" className="gp-chip" aria-pressed={l === label} data-held={held(c, ask) ? "" : undefined} aria-label={l || "an unlabeled cart icon"} onClick={() => pick(l)}>
                  {l || <Cart />}
                </button>
              </li>
            );
          })}
        </ul>
      </div>
    </Fig>
  );
}
