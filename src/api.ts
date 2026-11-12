// The contract between the UI and the Rust core. Types mirror
// crates/core/src/view.rs, stations.rs and preflight.rs.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createMockBackend } from "./mock";

export type Status = "blocked" | "done" | "working" | "idle" | "unknown" | "offline";

export interface Session {
  link_id: string;
  status: Status;
  last_status: Status;
  live: boolean;
  pane_id: string | null;
  agent: string | null;
  title: string | null;
  station_id: string | null;
  station_name: string | null;
  prompt: string | null;
  pending_prompt: string | null;
  workspace: string | null;
  cwd: string | null;
  created_at: number;
  last_seen: number | null;
  /** When the end of the output was last saved; null if nothing is saved. */
  output_at: number | null;
}

export interface Card {
  id: string;
  title: string;
  cwd: string;
  notes: string;
  done: boolean;
  created_at: number;
  updated_at: number;
  sessions: Session[];
}

export interface Contact {
  pane_id: string;
  status: Status;
  agent: string | null;
  title: string | null;
  workspace: string | null;
  cwd: string | null;
}

export interface Counts {
  blocked: number;
  done: number;
  working: number;
  idle: number;
  offline: number;
}

export interface BoardView {
  connected: boolean;
  error: string | null;
  herdr_version: string | null;
  endpoint: string;
  cards: Card[];
  unassigned: Contact[];
  counts: Counts;
}

export interface Station {
  id: string;
  name: string;
  post: string;
  duty: string;
  agent: string;
  model: string | null;
  args: string[];
  prompt: string;
}

export interface Location {
  id: string;
  name: string;
  subtitle: string;
  stations: Station[];
}

export interface Roster {
  locations: Location[];
}

export interface Check {
  id: string;
  label: string;
  status: "ok" | "warn" | "fail";
  detail: string;
  fix: string | null;
}

export interface DispatchRequest {
  card_id: string;
  station_id: string;
  prompt: string;
  agent?: string | null;
  model?: string | null;
}

export interface DispatchResult {
  link_id: string;
  pane_id: string;
  warning: string | null;
}

export interface CardPatch {
  title?: string;
  cwd?: string;
  notes?: string;
  done?: boolean;
}

export interface Backend {
  view(): Promise<BoardView>;
  roster(): Promise<Roster>;
  preflight(): Promise<Check[]>;
  createCard(title: string, cwd: string, notes: string): Promise<string>;
  updateCard(id: string, patch: CardPatch): Promise<void>;
  deleteCard(id: string): Promise<void>;
  attach(cardId: string, paneId: string): Promise<void>;
  detach(linkId: string): Promise<void>;
  dispatch(request: DispatchRequest): Promise<DispatchResult>;
  sendPending(linkId: string): Promise<void>;
  followUp(paneId: string, text: string): Promise<void>;
  replyKeys(paneId: string, text: string | null, keys: string[]): Promise<void>;
  tail(paneId: string, lines: number): Promise<string>;
  /** Output saved when the session's agent last stopped working. */
  savedOutput(linkId: string): Promise<string | null>;
  focus(paneId: string): Promise<void>;
  /** Calls `cb` whenever the board view may have changed. Returns an unsubscribe. */
  onChange(cb: () => void): () => void;
}

const tauriBackend: Backend = {
  view: () => invoke("board_view"),
  roster: () => invoke("roster"),
  preflight: () => invoke("preflight"),
  createCard: (title, cwd, notes) => invoke("create_card", { title, cwd, notes }),
  updateCard: (id, patch) => invoke("update_card", { id, ...patch }),
  deleteCard: (id) => invoke("delete_card", { id }),
  attach: (cardId, paneId) => invoke("attach", { cardId, paneId }),
  detach: (linkId) => invoke("detach", { linkId }),
  dispatch: (request) => invoke("dispatch", { request }),
  sendPending: (linkId) => invoke("send_pending", { linkId }),
  followUp: (paneId, text) => invoke("follow_up", { paneId, text }),
  replyKeys: (paneId, text, keys) => invoke("reply_keys", { paneId, text, keys }),
  tail: (paneId, lines) => invoke("tail", { paneId, lines }),
  savedOutput: (linkId) => invoke("saved_output", { linkId }),
  focus: (paneId) => invoke("focus", { paneId }),
  onChange: (cb) => {
    const pending = listen("board-changed", cb);
    return () => {
      pending.then((unlisten) => unlisten());
    };
  },
};

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** In a plain browser (`npm run dev`) the UI runs on simulated data. */
export const isDemo = !inTauri;
export const backend: Backend = inTauri ? tauriBackend : createMockBackend();

/** Errors from the core arrive as strings; anything else is unexpected. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
