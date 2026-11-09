// Simulated backend for running the UI in a plain browser, without Tauri or
// Herdr. Behaviour is deliberately simple: prompts work for a few seconds and
// then finish.

import type { Backend, BoardView, Card, Check, Contact, Roster, Session, Status } from "./api";

const station = (id: string, name: string, post: string, duty: string) => ({
  id,
  name,
  post,
  duty,
  agent: "claude",
  model: null,
  args: [],
  prompt: `(${duty})`,
});

const ROSTER: Roster = {
  locations: [
    {
      id: "academy",
      name: "Starfleet Academy",
      subtitle: "San Francisco, Earth",
      stations: [
        station("academy.boothby", "Boothby", "Groundskeeper", "Explain and teach"),
        station("academy.cadet", "Cadet Detail", "Training Drills", "Docs and tests"),
      ],
    },
    {
      id: "daystrom",
      name: "Daystrom Institute",
      subtitle: "Advanced research",
      stations: [station("daystrom.research", "Dr. Richard Daystrom", "Research Lab", "Deep research")],
    },
    {
      id: "enterprise",
      name: "USS Enterprise NCC-1701-D",
      subtitle: "Main bridge",
      stations: [
        station("enterprise.picard", "Capt. Jean-Luc Picard", "Command", "Plan and decide"),
        station("enterprise.riker", "Cmdr. William Riker", "First Officer", "Implement the feature"),
        station("enterprise.data", "Lt. Cmdr. Data", "Operations", "Analyse the codebase"),
        station("enterprise.laforge", "Lt. Cmdr. Geordi La Forge", "Engineering", "Fix, tune and refactor"),
        station("enterprise.worf", "Lt. Worf", "Tactical", "Security review"),
        station("enterprise.crusher", "Dr. Beverly Crusher", "Sickbay", "Diagnose failures"),
        station("enterprise.troi", "Counselor Deanna Troi", "Counselor", "UX and wording review"),
      ],
    },
    {
      id: "ds9",
      name: "Deep Space Nine",
      subtitle: "Operations centre",
      stations: [
        station("ds9.sisko", "Capt. Benjamin Sisko", "Command", "Plan and decide"),
        station("ds9.kira", "Maj. Kira Nerys", "First Officer", "Code review"),
        station("ds9.obrien", "Chief Miles O'Brien", "Operations", "Infrastructure and CI"),
        station("ds9.dax", "Lt. Cmdr. Jadzia Dax", "Science", "Explore options"),
        station("ds9.bashir", "Dr. Julian Bashir", "Infirmary", "Debug"),
        station("ds9.odo", "Constable Odo", "Security", "Audit dependencies and secrets"),
        station("ds9.quark", "Quark", "Quark's", "Evaluate third-party tools"),
      ],
    },
  ],
};

export function createMockBackend(): Backend {
  const now = Date.now();
  let seq = 100;
  const listeners = new Set<() => void>();
  const notify = () => listeners.forEach((cb) => cb());
  const transcripts = new Map<string, string[]>();

  const stationName = (id: string | null) =>
    ROSTER.locations.flatMap((l) => l.stations).find((s) => s.id === id)?.name ?? null;

  const session = (
    paneId: string,
    stationId: string | null,
    status: Status,
    prompt: string | null,
    ageMin: number,
    extra: Partial<Session> = {},
  ): Session => ({
    link_id: `l${seq++}`,
    status,
    last_status: status === "offline" ? "done" : status,
    live: status !== "offline",
    pane_id: status === "offline" ? null : paneId,
    agent: "claude",
    title: stationName(stationId) ?? "claude",
    station_id: stationId,
    station_name: stationName(stationId),
    prompt,
    pending_prompt: null,
    workspace: null,
    cwd: null,
    created_at: now - ageMin * 60_000,
    last_seen: now - 5_000,
    ...extra,
  });

  const cards: Card[] = [
    {
      id: "c1",
      title: "order-up-api: cache key deletion",
      cwd: "/home/me/meta/microservices/order-up-api",
      notes: "Can the Redis library delete one key or a whole cache?",
      done: false,
      created_at: now - 3 * 3600_000,
      updated_at: now - 60_000,
      sessions: [
        session("w1:p2", "enterprise.laforge", "blocked", "Add a pattern delete to the cache wrapper.", 12),
        session("w1:p1", "daystrom.research", "done", "Can the library delete cache keys by pattern?", 34),
      ],
    },
    {
      id: "c2",
      title: "Homelab dashboards and metrics",
      cwd: "/home/me/meta/operations/homelab",
      notes: "",
      done: false,
      created_at: now - 26 * 3600_000,
      updated_at: now - 20 * 60_000,
      sessions: [
        session("w2:p1", "ds9.obrien", "working", "Fix the Loki retention config and verify in Grafana.", 6),
        session("w2:p0", "ds9.kira", "offline", "Review the Grafana provisioning PR.", 600),
      ],
    },
    {
      id: "c3",
      title: "Refresh personal site",
      cwd: "/home/me/meta/ui/site",
      notes: "",
      done: true,
      created_at: now - 30 * 24 * 3600_000,
      updated_at: now - 29 * 24 * 3600_000,
      sessions: [],
    },
  ];
  let unassigned: Contact[] = [
    {
      pane_id: "w7:p1",
      status: "working",
      agent: "opencode",
      title: "scratch",
      workspace: "scratch",
      cwd: "/home/me/scratch",
    },
  ];
  transcripts.set("w1:p2", ["> Add a pattern delete to the cache wrapper.", "Edit src/cache.ts?", "  1. Yes   2. Yes, always   3. No"]);
  transcripts.set("w1:p1", ["> Can the library delete cache keys by pattern?", "Finished. Yes: single keys and whole caches; pattern delete needs SCAN."]);
  transcripts.set("w2:p1", ["> Fix the Loki retention config and verify in Grafana.", "Reading loki/config.yaml ..."]);
  transcripts.set("w7:p1", ["opencode session started by hand"]);

  const allSessions = () => cards.flatMap((c) => c.sessions);
  const findSession = (paneId: string) => allSessions().find((s) => s.pane_id === paneId);
  const setStatus = (paneId: string, status: Status, line?: string) => {
    const s = findSession(paneId);
    if (s) s.status = s.last_status = status;
    const c = unassigned.find((u) => u.pane_id === paneId);
    if (c) c.status = status;
    if (line) transcripts.set(paneId, [...(transcripts.get(paneId) ?? []), line]);
    notify();
  };
  const run = (paneId: string, text: string) => {
    setStatus(paneId, "working", `> ${text}`);
    setTimeout(() => setStatus(paneId, "done", "Finished. Summary: simulated work complete."), 5000);
  };

  const view = (): BoardView => {
    const counts = { blocked: 0, done: 0, working: 0, idle: 0, offline: 0 };
    const tally = (s: Status) => {
      if (s === "unknown" || s === "idle") counts.idle++;
      else counts[s]++;
    };
    cards.forEach((c) => c.sessions.forEach((s) => (s.live || !c.done) && tally(s.status)));
    unassigned.forEach((u) => tally(u.status));
    const sorted = [...cards].sort((a, b) => Number(a.done) - Number(b.done) || b.updated_at - a.updated_at);
    return {
      connected: true,
      error: null,
      herdr_version: "demo data",
      endpoint: "simulated",
      cards: structuredClone(sorted),
      unassigned: structuredClone(unassigned),
      counts,
    };
  };

  const card = (id: string) => {
    const c = cards.find((x) => x.id === id);
    if (!c) throw "that card no longer exists";
    return c;
  };

  const checks: Check[] = [
    { id: "environment", label: "Environment", status: "ok", detail: "browser demo (no Tauri shell)", fix: null },
    { id: "tool.herdr", label: "Herdr", status: "ok", detail: "herdr 0.9.5 (/home/me/.local/bin/herdr)", fix: null },
    { id: "herdr.server", label: "Herdr server", status: "ok", detail: "0.9.5, protocol 22", fix: null },
    { id: "integration.claude", label: "Herdr integration for claude", status: "ok", detail: "installed and current", fix: null },
    { id: "tool.claude", label: "Claude Code", status: "ok", detail: "2.1.0 (/home/me/.local/bin/claude)", fix: null },
    { id: "tool.opencode", label: "OpenCode", status: "warn", detail: "`opencode` was not found in linux", fix: "Optional, for ChatGPT and Gemini: https://opencode.ai" },
    { id: "claude_mem", label: "claude-mem worker", status: "ok", detail: "answering on port 37700", fix: null },
  ];

  return {
    view: async () => view(),
    roster: async () => ROSTER,
    preflight: async () => checks,
    createCard: async (title, cwd, notes) => {
      if (!title.trim()) throw "a card needs a title";
      if (!cwd.trim().startsWith("/") && !cwd.trim().startsWith("~")) throw `\`${cwd}\` is not a folder in linux`;
      const id = `c${seq++}`;
      cards.push({ id, title: title.trim(), cwd: cwd.trim(), notes: notes.trim(), done: false, created_at: Date.now(), updated_at: Date.now(), sessions: [] });
      notify();
      return id;
    },
    updateCard: async (id, patch) => {
      Object.assign(card(id), patch, { updated_at: Date.now() });
      notify();
    },
    deleteCard: async (id) => {
      cards.splice(cards.indexOf(card(id)), 1);
      notify();
    },
    attach: async (cardId, paneId) => {
      const contact = unassigned.find((u) => u.pane_id === paneId);
      if (!contact) throw "that session is no longer running in Herdr";
      unassigned = unassigned.filter((u) => u.pane_id !== paneId);
      card(cardId).sessions.unshift(
        session(paneId, null, contact.status, null, 0, { agent: contact.agent, title: contact.title, workspace: contact.workspace, cwd: contact.cwd }),
      );
      notify();
    },
    detach: async (linkId) => {
      for (const c of cards) {
        const s = c.sessions.find((x) => x.link_id === linkId);
        if (!s) continue;
        c.sessions = c.sessions.filter((x) => x !== s);
        if (s.live && s.pane_id) {
          unassigned.push({ pane_id: s.pane_id, status: s.status, agent: s.agent, title: s.title, workspace: s.workspace, cwd: s.cwd });
        }
      }
      notify();
    },
    dispatch: async (request) => {
      if (!request.prompt.trim()) throw "write a prompt for the agent first";
      const paneId = `w${seq++}:p1`;
      const s = session(paneId, request.station_id, "idle", request.prompt.trim(), 0, { agent: request.agent || "claude" });
      const c = card(request.card_id);
      c.sessions.unshift(s);
      c.updated_at = Date.now();
      run(paneId, request.prompt.trim());
      return { link_id: s.link_id, pane_id: paneId, warning: null };
    },
    sendPending: async () => {},
    followUp: async (paneId, text) => {
      if (findSession(paneId)?.status === "blocked") throw "the agent is waiting on a question; answer that first (reply keys, or jump to the pane)";
      run(paneId, text);
    },
    replyKeys: async (paneId) => {
      setStatus(paneId, "working", "(answered)");
      setTimeout(() => setStatus(paneId, "done", "Finished. Summary: simulated work complete."), 4000);
    },
    tail: async (paneId) => (transcripts.get(paneId) ?? []).join("\n"),
    focus: async () => {},
    onChange: (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
  };
}
