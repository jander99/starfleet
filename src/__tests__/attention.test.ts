import { describe, expect, it } from "vitest";
import { attentionRank, attentionSorted } from "../attention";
import type { Card, Session } from "../api";

const session = (s: Partial<Session>): Session => ({
  link_id: s.link_id ?? "l1", status: "idle", last_status: "idle", live: true,
  pane_id: "w1:p1", agent: "claude", title: null, station_id: null, station_name: null,
  prompt: null, pending_prompt: null, workspace: null, cwd: null,
  created_at: 1, last_seen: 1, output_at: null, ...s,
});
const card = (c: Partial<Card> & { id: string; state: Card["state"] }): Card => ({
  title: c.id, cwd: "/w", notes: "", done: false, created_at: 1, updated_at: 1,
  sessions: [], artifacts: [], kind: "repo", ...c,
});

describe("attention ordering", () => {
  it("puts blocked first even when the newest session is interrupted", () => {
    const c = card({
      id: "c1", state: "interrupted",
      sessions: [
        session({ link_id: "old", created_at: 1, status: "blocked" }),
        session({ link_id: "new", created_at: 2, live: false, status: "offline", end_reason: "interrupted" }),
      ],
    });
    expect(attentionRank(c)).toBe(0);
  });

  it("treats a finished PR as a decision, ahead of plain working", () => {
    const pr = card({ id: "pr", state: "working", artifacts: [{ id: "a", kind: "pr", label: "PR", target: "https://github.com/o/r/pull/1", last_status: "ci_passed", checked_at: 1 }] });
    const work = card({ id: "w", state: "working" });
    expect(attentionRank(pr)).toBeLessThan(attentionRank(work));
  });

  it("sorts stably and keeps done cards last", () => {
    const list = [card({ id: "a", state: "idle" }), card({ id: "b", state: "blocked" }), card({ id: "c", state: "interrupted" })];
    expect(attentionSorted(list).map((c) => c.id)).toEqual(["b", "c", "a"]);
    const tie = [card({ id: "x", state: "idle", updated_at: 1 }), card({ id: "y", state: "idle", updated_at: 2 })];
    expect(attentionSorted(tie).map((c) => c.id)).toEqual(["y", "x"]);
  });
});

import { CARD_STATE_LABEL } from "../ui";

describe("card state pill", () => {
  it("labels every card state, paused and interrupted differently", () => {
    expect(CARD_STATE_LABEL.blocked).toBe("Needs you");
    expect(CARD_STATE_LABEL.interrupted).toBe("Interrupted");
    expect(CARD_STATE_LABEL.paused).toBe("Paused");
    expect(CARD_STATE_LABEL.working).toBe("Working");
    // Every core state has a label — the core's CardState is the same set.
    expect(Object.keys(CARD_STATE_LABEL).sort()).toEqual(
      ["blocked", "done", "empty", "idle", "interrupted", "offline", "paused", "working"],
    );
  });
});
