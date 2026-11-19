import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const listen = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

describe("api in a plain browser", () => {
  it("runs on the mock backend and reports demo mode", async () => {
    vi.resetModules();
    const api = await import("../api");
    expect(api.isDemo).toBe(true);
    const view = await api.backend.view();
    expect(view.cards.length).toBeGreaterThan(0);
  });
});

describe("api inside the Tauri shell", () => {
  beforeEach(() => {
    invoke.mockReset().mockResolvedValue(undefined);
    listen.mockReset().mockResolvedValue(vi.fn());
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    vi.resetModules();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it("routes every backend call to its Tauri command", async () => {
    const { backend, isDemo } = await import("../api");
    expect(isDemo).toBe(false);

    await backend.view();
    await backend.roster();
    await backend.preflight();
    await backend.createCard("t", "/w", "n");
    await backend.updateCard("c1", { title: "x", done: true });
    await backend.deleteCard("c1");
    await backend.attach("c1", "p1");
    await backend.detach("l1");
    await backend.dispatch({ card_id: "c1", station_id: "s1", prompt: "go" });
    await backend.sendPending("l1");
    await backend.followUp("p1", "more");
    await backend.replyKeys("p1", "1", ["enter"]);
    await backend.tail("p1", 50);
    await backend.savedOutput("l1");
    await backend.focus("p1");
    await backend.addArtifact("c1", "pr", "PR #1", "https://x");
    await backend.openArtifact("c1", "a1");
    await backend.readLog("c1", 10);
    await backend.appendLog("c1", "note text", { kind: "decision", sessionId: "l1" });
    await backend.appendLog("c1", "plain");

    expect(invoke.mock.calls).toEqual([
      ["board_view"],
      ["roster"],
      ["preflight"],
      ["create_card", { title: "t", cwd: "/w", notes: "n" }],
      ["update_card", { id: "c1", title: "x", done: true }],
      ["delete_card", { id: "c1" }],
      ["attach", { cardId: "c1", paneId: "p1" }],
      ["detach", { linkId: "l1" }],
      ["dispatch", { request: { card_id: "c1", station_id: "s1", prompt: "go" } }],
      ["send_pending", { linkId: "l1" }],
      ["follow_up", { paneId: "p1", text: "more" }],
      ["reply_keys", { paneId: "p1", text: "1", keys: ["enter"] }],
      ["tail", { paneId: "p1", lines: 50 }],
      ["saved_output", { linkId: "l1" }],
      ["focus", { paneId: "p1" }],
      ["add_artifact", { cardId: "c1", kind: "pr", label: "PR #1", target: "https://x" }],
      ["open_artifact", { cardId: "c1", artifactId: "a1" }],
      ["read_log", { cardId: "c1", limit: 10 }],
      ["append_log", { cardId: "c1", text: "note text", kind: "decision", sessionId: "l1" }],
      ["append_log", { cardId: "c1", text: "plain", kind: null, sessionId: null }],
    ]);
  });

  it("subscribes to board changes and unsubscribes on demand", async () => {
    const unlisten = vi.fn();
    listen.mockResolvedValue(unlisten);
    const { backend } = await import("../api");
    const cb = vi.fn();
    const off = backend.onChange(cb);
    expect(listen).toHaveBeenCalledWith("board-changed", cb);
    off();
    // The unsubscribe waits for the pending listen promise.
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });
});
