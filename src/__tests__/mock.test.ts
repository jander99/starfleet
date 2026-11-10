import { describe, expect, it, vi } from "vitest";
import { errorText } from "../api";
import { createMockBackend } from "../mock";

describe("errorText", () => {
  it("handles strings, errors and other values", () => {
    expect(errorText("boom")).toBe("boom");
    expect(errorText(new Error("bad"))).toBe("bad");
    expect(errorText(42)).toBe("42");
  });
});

describe("mock backend", () => {
  it("creates and deletes cards, notifying listeners", async () => {
    const b = createMockBackend();
    const cb = vi.fn();
    const off = b.onChange(cb);
    const before = (await b.view()).cards.length;

    const id = await b.createCard("t", "/tmp", "n");
    expect((await b.view()).cards.length).toBe(before + 1);

    await b.deleteCard(id);
    expect((await b.view()).cards.length).toBe(before);
    expect(cb).toHaveBeenCalled();
    off();
  });

  it("serves a roster", async () => {
    const r = await createMockBackend().roster();
    expect(r.locations.length).toBeGreaterThan(0);
  });
});
