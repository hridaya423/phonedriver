"use client";

import { useEffect, useRef } from "react";
import { mountHero } from "@/lib/hero-engine";

export function HeroEngine() {
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const hero = ref.current?.closest<HTMLElement>(".hx");
    return hero ? mountHero(hero) : undefined;
  }, []);
  return <span ref={ref} hidden />;
}
