import { describe, expect, it } from "vitest";
import { skyAt } from "../sky";

describe("skyAt", () => {
  it("is night at midnight with Sol hidden", () => {
    const s = skyAt(0);
    expect(s.from).toBe("night");
    expect(s.sol).toBe(0);
  });

  it("is full day at noon", () => {
    const s = skyAt(12);
    expect(s.from).toBe("day");
    expect(s.to).toBe("day");
    expect(s.sol).toBeCloseTo(0.55);
  });

  it("blends dawn into day", () => {
    const s = skyAt(7);
    expect(s.from).toBe("dawn");
    expect(s.to).toBe("day");
    expect(s.mix).toBeCloseTo(0.5);
  });

  it("wraps hours outside 0-24", () => {
    expect(skyAt(36)).toEqual(skyAt(12));
    expect(skyAt(-12)).toEqual(skyAt(12));
  });
});
