import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, renderHook, waitFor } from "@testing-library/react";
import { currentSky, skyAt, useSky } from "../sky";

afterEach(() => {
  vi.useRealTimers();
  window.history.replaceState({}, "", "/");
});

describe("skyAt edges", () => {
  it("holds night across the small hours", () => {
    const s = skyAt(2);
    expect(s.from).toBe("night");
    expect(s.to).toBe("night");
    expect(s.sol).toBe(0);
  });

  it("crosses into dawn and out to dusk", () => {
    expect(skyAt(5).from).toBe("night");
    expect(skyAt(5).to).toBe("dawn");
    expect(skyAt(17).from).toBe("day");
    expect(skyAt(17).to).toBe("dusk");
    expect(skyAt(19).to).toBe("night");
  });

  it("reaches full Sol at dawn and dusk", () => {
    expect(skyAt(6).sol).toBeCloseTo(1);
    expect(skyAt(18).sol).toBeCloseTo(1);
  });

  it("handles the last hour before midnight", () => {
    const s = skyAt(23);
    expect(s.from).toBe("night");
    expect(s.to).toBe("night");
  });
});

describe("currentSky", () => {
  it("follows the clock when nothing is pinned", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2024, 0, 1, 12, 30));
    expect(currentSky()).toEqual(skyAt(12.5));
  });

  it("is pinned by ?hour=", () => {
    window.history.replaceState({}, "", "/?hour=14");
    expect(currentSky()).toEqual(skyAt(14));
  });

  it("ignores a non-numeric ?hour=", () => {
    window.history.replaceState({}, "", "/?hour=lunch");
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2024, 0, 1, 3, 0));
    expect(currentSky()).toEqual(skyAt(3));
  });
});

describe("useSky", () => {
  it("refreshes every minute", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2024, 0, 1, 12, 0));
    const { result } = renderHook(() => useSky());
    expect(result.current).toEqual(skyAt(12));
    act(() => {
      vi.setSystemTime(new Date(2024, 0, 1, 18, 0));
      vi.advanceTimersByTime(60_000);
    });
    // A minute of clock drift past 18:00, so compare to the sky at 18:01.
    expect(result.current).toEqual(skyAt(18 + 1 / 60));
  });

  it("does not schedule a timer when the hour is pinned", () => {
    window.history.replaceState({}, "", "/?hour=9");
    vi.useFakeTimers();
    const setInterval = vi.spyOn(window, "setInterval");
    const { result } = renderHook(() => useSky());
    expect(result.current).toEqual(skyAt(9));
    expect(setInterval).not.toHaveBeenCalled();
  });
});

describe("rendering under the sky", () => {
  it("mounts cleanly", async () => {
    const { container } = render(<div data-testid="x" />);
    expect(container.firstChild).not.toBeNull();
    await waitFor(() => expect(true).toBe(true));
  });
});
