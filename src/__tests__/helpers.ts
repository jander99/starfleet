// Shared fixtures and a controllable fake backend for the UI tests.
// Not a test file itself: vitest only collects *.test.{ts,tsx}.

import { vi } from "vitest";
import type { Backend, BoardView, Card, Contact, LogEntry, Roster, Session, Station } from "../api";

/** A fixed clock so "time ago" strings are stable. */
export const NOW = 1_700_000_000_000;

export function makeSession(over: Partial<Session> = {}): Session {
  return {
    link_id: "l1",
    status: "working",
    last_status: "working",
    live: true,
    pane_id: "p1",
    agent: "claude",
    title: null,
    station_id: "enterprise.data",
    station_name: "Data",
    prompt: null,
    pending_prompt: null,
    workspace: null,
    cwd: "/work",
    created_at: NOW - 60_000,
    last_seen: NOW - 1_000,
    output_at: null,
    ...over,
  };
}

export function makeCard(over: Partial<Card> = {}): Card {
  return {
    id: "c1",
    title: "Card one",
    cwd: "/work/one",
    notes: "",
    done: false,
    created_at: NOW - 10_000,
    updated_at: NOW - 5_000,
    sessions: [],
    artifacts: [],
    state: "idle",
    kind: "repo",
    ...over,
  };
}

export function makeView(over: Partial<BoardView> = {}): BoardView {
  return {
    connected: true,
    error: null,
    herdr_version: "1.2.3",
    endpoint: "unix:///tmp/herdr.sock",
    cards: [],
    unassigned: [],
    counts: { blocked: 0, done: 0, working: 0, idle: 0, offline: 0 },
    ...over,
  };
}

export function makeStation(over: Partial<Station> = {}): Station {
  return {
    id: "enterprise.data",
    name: "Data",
    post: "Operations",
    duty: "Keeps the board honest",
    harness: "claude",
    model: null,
    args: [],
    prompt: "",
    ...over,
  };
}

export function makeRoster(over: Partial<Roster> = {}): Roster {
  return {
    locations: [
      {
        id: "enterprise",
        name: "Enterprise",
        subtitle: "Deep space",
        stations: [makeStation()],
      },
    ],
    ...over,
  };
}

export function makeContact(over: Partial<Contact> = {}): Contact {
  return {
    pane_id: "p9",
    status: "idle",
    agent: "codex",
    title: "Loose session",
    workspace: null,
    cwd: "/tmp/loose",
    ...over,
  };
}

export function makeLog(over: Partial<LogEntry> = {}): LogEntry {
  return {
    id: "e1",
    owner: "captain",
    at: NOW,
    kind: "note",
    text: "hello",
    links: [],
    session_id: null,
    ...over,
  };
}

/** Every backend method as a spy with a sane default, so a test only sets what it cares about. */
export const fakeBackend: Backend = {
  view: vi.fn(async () => makeView()),
  roster: vi.fn(async () => makeRoster()),
  preflight: vi.fn(async () => []),
  createCard: vi.fn(async () => "c-new"),
  updateCard: vi.fn(async () => {}),
  deleteCard: vi.fn(async () => {}),
  attach: vi.fn(async () => {}),
  detach: vi.fn(async () => {}),
  dispatch: vi.fn(async () => ({ link_id: "l-new", pane_id: "p-new", warning: null })),
  sendPending: vi.fn(async () => {}),
  followUp: vi.fn(async () => {}),
  replyKeys: vi.fn(async () => {}),
  tail: vi.fn(async () => "recent output"),
  savedOutput: vi.fn(async () => "saved output"),
  focus: vi.fn(async () => {}),
  addArtifact: vi.fn(async () => "a-new"),
  openArtifact: vi.fn(async () => {}),
  readLog: vi.fn(async () => [] as LogEntry[]),
  appendLog: vi.fn(async (cardId: string, text: string, opts?: { kind?: string; sessionId?: string | null }) =>
    makeLog({ id: "e-new", text, kind: opts?.kind ?? "note", session_id: opts?.sessionId ?? null }),
  ),
  onChange: vi.fn(() => () => {}),
};
