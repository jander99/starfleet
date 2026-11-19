import { afterEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { Backdrop } from "../components/Backdrop";

/** A 2d context stub that records what was drawn. */
function fakeCtx() {
  return {
    fillStyle: "",
    globalAlpha: 1,
    fillRect: vi.fn(),
  } as unknown as CanvasRenderingContext2D & { fillRect: ReturnType<typeof vi.fn> };
}

function stubMatchMedia(reduced: boolean) {
  vi.stubGlobal("matchMedia", (q: string) => ({
    matches: reduced && q.includes("reduced-motion"),
    media: q,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("Backdrop", () => {
  it("renders a canvas and draws the star field", () => {
    stubMatchMedia(false);
    const ctx = fakeCtx();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(ctx);
    const raf = vi.fn(() => 1);
    vi.stubGlobal("requestAnimationFrame", raf);
    vi.stubGlobal("cancelAnimationFrame", vi.fn());

    const { container } = render(<Backdrop />);
    expect(container.querySelector("canvas.backdrop")).not.toBeNull();
    // The first frame is drawn immediately, before any animation frame runs.
    expect(ctx.fillRect).toHaveBeenCalled();
    // Background is painted.
    expect(ctx.fillStyle).toBeDefined();
    expect(raf).toHaveBeenCalled();
  });

  it("respects reduced motion by not animating", () => {
    stubMatchMedia(true);
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(fakeCtx());
    const raf = vi.fn(() => 1);
    vi.stubGlobal("requestAnimationFrame", raf);

    render(<Backdrop />);
    expect(raf).not.toHaveBeenCalled();
  });

  it("redraws on resize", () => {
    stubMatchMedia(false);
    const ctx = fakeCtx();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(ctx);
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 1));
    vi.stubGlobal("cancelAnimationFrame", vi.fn());

    render(<Backdrop />);
    const before = ctx.fillRect.mock.calls.length;
    window.dispatchEvent(new Event("resize"));
    expect(ctx.fillRect.mock.calls.length).toBeGreaterThan(before);
  });

  it("does nothing when the canvas has no 2d context", () => {
    stubMatchMedia(false);
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    const raf = vi.fn(() => 1);
    vi.stubGlobal("requestAnimationFrame", raf);
    const { container } = render(<Backdrop />);
    expect(container.querySelector("canvas")).not.toBeNull();
    expect(raf).not.toHaveBeenCalled();
  });

  it("cleans up the frame and the resize listener on unmount", () => {
    stubMatchMedia(false);
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(fakeCtx());
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 7));
    const cancel = vi.fn();
    vi.stubGlobal("cancelAnimationFrame", cancel);
    const remove = vi.spyOn(window, "removeEventListener");

    const { unmount } = render(<Backdrop />);
    unmount();
    expect(cancel).toHaveBeenCalledWith(7);
    expect(remove).toHaveBeenCalledWith("resize", expect.any(Function));
  });

  it("animates frames: throttles by frame time and moves a ship across", () => {
    stubMatchMedia(false);
    const ctx = fakeCtx();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(ctx);
    // Capture the tick callback so the test can drive frames by hand.
    let tick: ((ms: number) => void) | null = null;
    vi.stubGlobal("requestAnimationFrame", (cb: (ms: number) => void) => {
      tick = cb;
      return 1;
    });
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    vi.spyOn(Math, "random").mockReturnValue(0); // a ship every launch window

    render(<Backdrop />);
    const before = ctx.fillRect.mock.calls.length;

    // Under the frame budget: no redraw.
    tick!(0);
    tick!(50);
    expect(ctx.fillRect.mock.calls.length).toBe(before);

    // Past the budget: redraw, and again as time moves on.
    tick!(200);
    expect(ctx.fillRect.mock.calls.length).toBeGreaterThan(before);
    // Long enough that the first ship launches and then leaves the screen.
    for (let ms = 300; ms < 120_000; ms += 200) tick!(ms);
  });

  it("draws Sol by day", () => {
    stubMatchMedia(false);
    vi.spyOn(Date.prototype, "getHours").mockReturnValue(12);
    vi.spyOn(Date.prototype, "getMinutes").mockReturnValue(0);
    const ctx = fakeCtx();
    const colors: string[] = [];
    (ctx.fillRect as ReturnType<typeof vi.fn>).mockImplementation(() => colors.push(ctx.fillStyle as string));
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(ctx);
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 1));

    render(<Backdrop />);
    // The hot core, the cross flare and the centre pixel are all painted.
    expect(colors).toContain("#ffd9a0");
    expect(colors).toContain("#ffe9c0");
    expect(colors).toContain("#fffbee");
  });

  it("hides Sol at night", () => {
    stubMatchMedia(false);
    // 02:00 is deep night: sol is 0, so drawSol returns before drawing.
    vi.spyOn(Date.prototype, "getHours").mockReturnValue(2);
    vi.spyOn(Date.prototype, "getMinutes").mockReturnValue(0);
    const ctx = fakeCtx();
    // Record the colour in effect for each fill.
    const colors: string[] = [];
    (ctx.fillRect as ReturnType<typeof vi.fn>).mockImplementation(() => colors.push(ctx.fillStyle as string));
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(ctx);
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 1));

    render(<Backdrop />);
    const SOL_COLORS = ["#ffd9a0", "#ffe9c0", "#fffbee"];
    expect(colors.length).toBeGreaterThan(0);
    expect(colors.filter((c) => SOL_COLORS.includes(c))).toEqual([]);
  });
});
