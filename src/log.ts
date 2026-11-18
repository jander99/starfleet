// Grouping for the Captain's Log timeline: one round per prompt, so old
// rounds can collapse while the newest stays open.

import type { LogEntry } from "./api";

/** Split entries into rounds, each starting at a prompt entry. */
export function rounds(entries: LogEntry[]): LogEntry[][] {
  const out: LogEntry[][] = [];
  for (const e of entries) {
    if (e.kind === "prompt" || out.length === 0) out.push([e]);
    else out[out.length - 1].push(e);
  }
  return out;
}
