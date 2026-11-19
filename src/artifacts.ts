// How an artifact row reads: a short status and how old the check is, so a
// stale green never looks fresh.

import type { Artifact } from "./api";
import { timeAgo } from "./ui";

export type Tone = "ok" | "warn" | "bad" | "muted";

const STATUS_TEXT: Record<string, string> = {
  ci_running: "CI running",
  ci_pending: "CI pending",
  ci_passed: "CI passed",
  ci_failed: "CI failed",
  merged: "Merged",
  closed: "Closed",
  draft: "Draft",
  open: "Open",
  unknown: "Unknown",
};

/** The status line for one artifact: what it says, and how much to trust it. */
export function artifactStatus(a: Artifact): { text: string; tone: Tone } {
  if (a.last_status === null) return { text: "checking…", tone: "muted" };
  const what = STATUS_TEXT[a.last_status] ?? a.last_status;
  const when = a.checked_at ? timeAgo(a.checked_at) : "";
  const age = a.checked_at ? Date.now() - a.checked_at : Infinity;
  const tone: Tone =
    a.last_status === "ci_failed" || a.last_status === "closed"
      ? "bad"
      : age > 3600_000
        ? "muted" // any verdict older than an hour, or never timestamped, may be stale
        : a.last_status === "ci_running"
          ? "warn"
          : a.last_status === "ci_passed" || a.last_status === "merged"
            ? "ok"
            : "muted";
  return { text: when ? `${what} · ${when}` : what, tone };
}
