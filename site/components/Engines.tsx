"use client";

import { useEffect, useRef } from "react";
import { mountCopy } from "@/lib/copy-engine";
import { mountDots } from "@/lib/dots-engine";
import { mountEnd, mountHalt, mountPalm, mountPoint } from "@/lib/figures";
import { mountJourney } from "@/lib/journey-engine";
import { mountType } from "@/lib/type-engine";
import { mountRace } from "@/lib/race-engine";
import { mountViews } from "@/lib/views-engine";

function Engine({ mount, host }: { mount: (el: HTMLElement) => () => void; host?: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const el = host ? ref.current?.closest<HTMLElement>(host) : document.body;
    return el ? mount(el) : undefined;
  }, [mount, host]);
  return <span ref={ref} hidden />;
}

export const CopyEngine = () => <Engine mount={mountCopy} />;
export const RaceEngine = () => <Engine mount={mountRace} host=".race" />;
export const ViewsEngine = () => <Engine mount={mountViews} host=".tv" />;
export const JourneyEngine = () => <Engine mount={mountJourney} />;
export const DotsEngine = () => <Engine mount={mountDots} host=".df" />;
export const TypeEngine = () => <Engine mount={mountType} host=".ins-say" />;
export const PointEngine = ({ host }: { host: string }) => <Engine mount={mountPoint} host={host} />;
export const EndEngine = () => <Engine mount={mountEnd} host=".end-pane" />;
export const HaltEngine = () => <Engine mount={mountHalt} host=".pt-mark" />;

export const PalmEngine = () => <Engine mount={mountPalm} host=".ask-stage" />;
