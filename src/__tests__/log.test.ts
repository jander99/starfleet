import { describe, expect, it } from "vitest";
import { rounds } from "../log";
import type { LogEntry } from "../api";

const e = (id: string, kind: string, text = id): LogEntry => ({ id, owner: "captain", at: 1, kind, text, links: [], session_id: null });

describe("log rounds", () => {
  it("groups a prompt with the entries that follow it", () => {
    const r = rounds([e("1", "prompt"), e("2", "result"), e("3", "note"), e("4", "prompt"), e("5", "result")]);
    expect(r.map((x) => x.map((y) => y.id))).toEqual([["1", "2", "3"], ["4", "5"]]);
  });

  it("keeps leading entries that belong to no round", () => {
    const r = rounds([e("0", "note"), e("1", "prompt"), e("2", "result")]);
    expect(r[0].map((x) => x.id)).toEqual(["0"]);
  });
});
