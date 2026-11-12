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

  it("keeps a session's output once its agent stops working", async () => {
    vi.useFakeTimers();
    try {
      const b = createMockBackend();
      const all = async () => (await b.view()).cards.flatMap((c) => c.sessions);

      // A closed session from the seed data still has its record.
      const offline = (await all()).find((s) => !s.live && s.output_at);
      expect(await b.savedOutput(offline!.link_id)).toContain("Finished");

      const card = (await b.view()).cards[0];
      const { link_id } = await b.dispatch({ card_id: card.id, station_id: "enterprise.data", prompt: "map it" });
      expect((await all()).find((s) => s.link_id === link_id)!.output_at).toBeNull();
      await vi.advanceTimersByTimeAsync(6000);
      expect((await all()).find((s) => s.link_id === link_id)!.output_at).not.toBeNull();
      expect(await b.savedOutput(link_id)).toContain("> map it");

      await expect(b.savedOutput("nope")).rejects.toBeDefined();
    } finally {
      vi.useRealTimers();
    }
  });

  it("serves a roster", async () => {
    const r = await createMockBackend().roster();
    expect(r.locations.length).toBeGreaterThan(0);
  });
});
