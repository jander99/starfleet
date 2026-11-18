import { describe, expect, it } from "vitest";
import { artifactStatus } from "../artifacts";
import type { Artifact } from "../api";

const a = (x: Partial<Artifact>): Artifact => ({ id: "a", kind: "pr", label: "PR", target: "t", last_status: null, checked_at: null, ...x });

describe("artifact status", () => {
  it("distinguishes never-checked, stale and fresh", () => {
    expect(artifactStatus(a({ last_status: null })).text).toBe("checking…");
    const old = artifactStatus(a({ last_status: "ci_passed", checked_at: Date.now() - 2 * 3600_000 }));
    expect(old.text).toContain("passed");
    expect(old.text).toContain("2h ago");
    expect(old.tone).toBe("muted");
    const fresh = artifactStatus(a({ last_status: "ci_passed", checked_at: Date.now() }));
    expect(fresh.tone).toBe("ok");
  });
  it("maps the status strings", () => {
    expect(artifactStatus(a({ last_status: "ci_running", checked_at: Date.now() }))).toEqual({ text: "CI running · just now", tone: "warn" });
    expect(artifactStatus(a({ last_status: "ci_failed", checked_at: Date.now() })).tone).toBe("bad");
    expect(artifactStatus(a({ last_status: "merged", checked_at: Date.now() })).tone).toBe("ok");
    expect(artifactStatus(a({ last_status: "unknown", checked_at: Date.now() })).tone).toBe("muted");
  });
});
