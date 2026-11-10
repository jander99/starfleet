import { useEffect, useRef } from "react";
import { currentSky } from "../sky";

/** One logical pixel is this many CSS pixels. */
const PX = 3;
const FRAME_MS = 100;

// Slow parallax drift, in logical pixels per second (a camera in orbit).
const LAYERS = [
  { density: 1 / 900, speed: 0.35, colors: ["#3a4666", "#46557a"] },
  { density: 1 / 1500, speed: 0.8, colors: ["#7d8bb0", "#9fb0d8", "#b9a98a"] },
  { density: 1 / 4200, speed: 1.6, colors: ["#e8eefc", "#cfe0ff", "#ffe6c4"] },
];

function rng(seed: number) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

interface Star {
  x: number;
  y: number;
  color: string;
  twinkle: number;
}
interface Ship {
  x: number;
  y: number;
  vx: number;
  vy: number;
}

function makeStars(w: number, h: number): Star[][] {
  const rand = rng(90125);
  return LAYERS.map((l, li) =>
    Array.from({ length: Math.round(w * h * l.density) }, () => ({
      x: Math.floor(rand() * w),
      y: Math.floor(rand() * h),
      color: l.colors[Math.floor(rand() * l.colors.length)],
      twinkle: li > 0 && rand() < 0.12 ? 3 + rand() * 6 : 0,
    })),
  );
}

/** Background traffic: a lone pixel crossing left to right, a few degrees off level. */
function launch(h: number): Ship {
  const angle = ((Math.random() - 0.5) * 8 * Math.PI) / 180;
  const speed = 6 + Math.random() * 6;
  return { x: -2, y: Math.floor(h * (0.05 + Math.random() * 0.6)), vx: Math.cos(angle) * speed, vy: Math.sin(angle) * speed };
}

/** Sol: a hot core with a small cross flare. */
function drawSol(ctx: CanvasRenderingContext2D, cx: number, cy: number, k: number, t: number) {
  if (k <= 0.02) return;
  const pulse = 1 + Math.sin(t * 0.6) * 0.08;
  const reach = Math.round((2 + 5 * k) * pulse);
  ctx.globalAlpha = 0.35 * k;
  ctx.fillStyle = "#ffd9a0";
  ctx.fillRect(cx - reach, cy, reach * 2 + 1, 1);
  ctx.fillRect(cx, cy - reach, 1, reach * 2 + 1);
  ctx.globalAlpha = 0.55 * k;
  ctx.fillStyle = "#ffe9c0";
  ctx.fillRect(cx - 1, cy - 1, 3, 3);
  ctx.globalAlpha = Math.min(1, 0.4 + k);
  ctx.fillStyle = "#fffbee";
  ctx.fillRect(cx, cy, 1, 1);
  ctx.globalAlpha = 1;
}

/** Pixel star field behind the whole app, shared by every view. */
export function Backdrop() {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;

    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let w = 0;
    let h = 0;
    let stars: Star[][] = [];
    let ship: Ship | null = null;
    let nextShip = 15 + Math.random() * 25;
    let last = 0;
    let raf = 0;
    const t0 = performance.now();

    const resize = () => {
      w = Math.ceil(window.innerWidth / PX);
      h = Math.ceil(window.innerHeight / PX);
      canvas.width = w;
      canvas.height = h;
      stars = makeStars(w, h);
    };

    const draw = (ms: number) => {
      const t = still ? 0 : (ms - t0) / 1000;
      ctx.fillStyle = "#05070d";
      ctx.fillRect(0, 0, w, h);

      LAYERS.forEach((l, li) => {
        const shift = t * l.speed;
        for (const s of stars[li]) {
          const x = Math.floor((((s.x - shift) % w) + w) % w);
          ctx.globalAlpha = s.twinkle ? 0.55 + 0.45 * Math.sin(t * s.twinkle + s.x) : 1;
          ctx.fillStyle = s.color;
          ctx.fillRect(x, s.y, 1, 1);
        }
      });
      ctx.globalAlpha = 1;

      if (ship) {
        const x = Math.round(ship.x);
        const y = Math.round(ship.y);
        ctx.fillStyle = "#dfe8ff";
        ctx.fillRect(x, y, 1, 1);
        ctx.globalAlpha = 0.45;
        ctx.fillRect(x - 1, Math.round(ship.y - ship.vy / Math.max(ship.vx, 1)), 1, 1);
        ctx.globalAlpha = 1;
      }

      drawSol(ctx, Math.round(w * 0.84), Math.round(h * 0.16), currentSky().sol, t);
    };

    const tick = (ms: number) => {
      raf = requestAnimationFrame(tick);
      if (ms - last < FRAME_MS) return;
      const dt = Math.min((ms - last) / 1000, 1);
      last = ms;
      if (ship) {
        ship.x += ship.vx * dt;
        ship.y += ship.vy * dt;
        if (ship.x > w + 2 || ship.y < -2 || ship.y > h + 2) ship = null;
      } else if ((nextShip -= dt) <= 0) {
        ship = launch(h);
        nextShip = 25 + Math.random() * 50;
      }
      draw(ms);
    };

    resize();
    draw(t0);
    const onResize = () => (resize(), draw(performance.now()));
    window.addEventListener("resize", onResize);
    if (!still) raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", onResize);
    };
  }, []);

  return <canvas ref={ref} className="backdrop" aria-hidden="true" />;
}
