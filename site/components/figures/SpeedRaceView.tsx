"use client";

import { useRef, useState } from "react";
import { Btn, Fig, Seg, grouped, useClock } from "./kit";

export type Lane = { id: string; label: string; us: number; them: number; ratio: string; old: boolean };

const SLOW = 25;
const REST = 1600;
const STREAK = 50;

const ms = (v: number) => (v >= 100 ? grouped(v) : grouped(v, 1));
const trip = (t: number, len: number) => {
  const p = (t % len) / len;
  return p < 0.5 ? p * 2 : 2 - p * 2;
};

export function SpeedRaceView({ emulator, phone, phoneName }: { emulator: Lane[]; phone: Lane[]; phoneName: string }) {
  const root = useRef<HTMLDivElement>(null);
  const [dev, setDev] = useState<"emu" | "phone">("emu");
  const [real, setReal] = useState(true);
  const [playing, setPlaying] = useState(true);
  const rows = dev === "emu" ? emulator : phone;
  const slow = real ? 1 : SLOW;
  const round = Math.max(...rows.map((r) => r.them)) * slow + REST;

  const clock = useClock(
    root,
    (now) => {
      const el = root.current;
      if (!el) return;
      const t = now % round;
      el.querySelectorAll<HTMLElement>("[data-row]").forEach((row, i) => {
        const r = rows[i];
        if (!r) return;
        const us = r.us * slow;
        const them = r.them * slow;
        const run = (row.querySelector<HTMLElement>(".sr-track")!.clientWidth || 0) - 12;
        const back = t >= them;
        const a = row.querySelector<HTMLElement>(".sr-us")!;
        const b = row.querySelector<HTMLElement>(".sr-them")!;
        const n = row.querySelector<HTMLElement>(".sr-n")!;
        const streak = us < STREAK && !back;
        a.style.transform = `translate3d(${back ? 0 : trip(t, us) * run}px,0,0)`;
        row.toggleAttribute("data-streak", streak);
        b.style.transform = `translate3d(${back ? 0 : trip(t, them) * run}px,0,0)`;
        row.toggleAttribute("data-back", back);
        n.textContent = back ? r.ratio : `${Math.floor(t / us)}`;
      });
    },
    { running: playing, still: round - 1, key: `${dev}${real}` },
  );

  const pick = (d: "emu" | "phone") => {
    setDev(d);
    setPlaying(true);
  };

  return (
    <Fig
      id="speed-race"
      title="Out to the phone and back"
      controls={
        <>
          <Seg
            label="Device"
            value={dev}
            onChange={pick}
            options={[
              ["emu", "Emulator"],
              ["phone", "HONOR X8"],
            ]}
          />
          <Seg
            label="Speed"
            value={real ? "real" : "slow"}
            onChange={(v) => {
              setReal(v === "real");
              setPlaying(true);
            }}
            options={[
              ["real", "Real speed"],
              ["slow", `${SLOW}x slower`],
            ]}
          />
          {!clock.reduced && <Btn onClick={() => setPlaying((p) => !p)}>{playing ? "Pause" : "Play"}</Btn>}
        </>
      }
      caption={
        dev === "emu" ? (
          <>
            Each ball is one call, out to the phone and back: phonedriver in orange, adb in gray. The number counts phonedriver calls that finish while adb finishes one. Medians of
            50 runs after 20 warm-ups on an Android 17 emulator (API 37), 11 November 2026, from <code>docs/bench/speed.md</code>. The screenshot row is 0.1.1, which handed back
            the newest frame on hand; since 0.1.5 a screenshot first waits for frames still inside the encoder.
          </>
        ) : (
          <>
            The same race on a {phoneName} over USB, 9 November 2026: medians of 30 phonedriver calls and 20 adb calls, from <code>docs/bench/speed.md</code>. The phone is slower
            than the emulator&apos;s host, so phonedriver&apos;s own read takes 78.5 ms here, and adb&apos;s takes about the same as on the emulator.
          </>
        )
      }
    >
      <div className="sr" ref={root} data-dev={dev}>
        <div className="sr-key" aria-hidden="true">
          <span className="us">phonedriver</span>
          <span>adb</span>
          <span className="sr-key-n">{real ? "calls per adb call" : "calls per adb call, slowed"}</span>
        </div>
        <ol className="sr-rows">
          {rows.map((r) => (
            <li key={r.id} data-row="" data-back="">
              <p className="sr-name">
                {r.label}
                {r.old && <em>0.1.1</em>}
                <span>
                  {ms(r.us)} <i>vs</i> {ms(r.them)} ms
                </span>
              </p>
              <span className="sr-track" aria-hidden="true">
                <i className="sr-us" />
                <i className="sr-them" />
              </span>
              <b className="sr-n" aria-label={`${r.ratio} faster`}>
                {r.ratio}
              </b>
            </li>
          ))}
        </ol>
      </div>
    </Fig>
  );
}
