// Pure ordering logic for the attention board: what needs the Captain first.
// This mirrors the core's attention_rank (crates/core/src/view.rs) but reads a
// whole Card, so blocked can aggregate over live sessions.

import type { Card, CardState } from "./api";

/** Rank by card state: lower is more urgent. Matches the core's values. */
const STATE_RANK: Record<CardState, number> = {
  blocked: 0,
  interrupted: 1,
  done: 2,
  paused: 3,
  working: 4,
  idle: 5,
  offline: 6,
  empty: 7,
};

/** A PR whose polled status is final: the Captain has to decide what is next. */
const DECISION_STATUSES = ["ci_passed", "merged", "closed", "ci_failed"];

/** Between interrupted (1) and done (2): a PR verdict is waiting on the Captain. */
const NEEDS_DECISION = 1.5;

/**
 * How much a card needs attention: lower is more urgent.
 *
 * Blocked aggregates: card state already accounts for any live blocked
 * session, but a card shown as interrupted while an older session is still
 * blocked must not sink below plain working cards.
 */
export function attentionRank(c: Card): number {
  const blocked =
    c.state === "blocked" || c.sessions.some((s) => s.live && s.status === "blocked");
  if (blocked) return STATE_RANK.blocked;
  const needsDecision = c.artifacts.some(
    (a) => a.kind === "pr" && a.last_status !== null && DECISION_STATUSES.includes(a.last_status),
  );
  if (needsDecision) return NEEDS_DECISION;
  return STATE_RANK[c.state];
}

/** Cards sorted by attention, ties broken by the newer card first, then id. */
export function attentionSorted(cards: Card[]): Card[] {
  return [...cards].sort(
    (a, b) => attentionRank(a) - attentionRank(b) || b.updated_at - a.updated_at || a.id.localeCompare(b.id),
  );
}
