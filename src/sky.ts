import { useEffect, useState } from "react";

export type Frame = "day" | "dusk" | "night" | "dawn";

/** Local hour -> which Earth frame is showing and how much of the next one blends in. */
const KEYS: [number, Frame][] = [
  [0, "night"],
  [4.5, "night"],
  [6, "dawn"],
  [8, "day"],
  [16, "day"],
  [18, "dusk"],
  [19.5, "night"],
  [24, "night"],
];

export interface Sky {
  /** Drawn first, fully opaque. */
  from: Frame;
  /** Drawn on top at `mix` opacity. */
  to: Frame;
  mix: number;
  /** 0 hides Sol (night); 1 is the brightest, at dawn and dusk. */
  sol: number;
}

const SOL: Record<Frame, number> = { day: 0.55, dusk: 1, night: 0, dawn: 1 };

export function skyAt(hour: number): Sky {
  const h = ((hour % 24) + 24) % 24;
  let i = KEYS.findIndex(([k]) => k > h);
  if (i < 0) i = KEYS.length - 1;
  const [h0, from] = KEYS[i - 1];
  const [h1, to] = KEYS[i];
  const mix = (h - h0) / (h1 - h0);
  return { from, to, mix, sol: SOL[from] * (1 - mix) + SOL[to] * mix };
}

/** `?hour=14` in the URL pins the sky, for checking the other times of day. */
function pinnedHour(): number | null {
  const v = new URLSearchParams(window.location.search).get("hour");
  const n = v === null ? NaN : Number(v);
  return Number.isFinite(n) ? n : null;
}

function now(): number {
  const d = new Date();
  return d.getHours() + d.getMinutes() / 60;
}

export function currentSky(): Sky {
  return skyAt(pinnedHour() ?? now());
}

/** The sky for this machine's local time, refreshed every minute. */
export function useSky(): Sky {
  const [sky, setSky] = useState(currentSky);
  useEffect(() => {
    if (pinnedHour() !== null) return;
    const id = window.setInterval(() => setSky(currentSky()), 60_000);
    return () => window.clearInterval(id);
  }, []);
  return sky;
}
