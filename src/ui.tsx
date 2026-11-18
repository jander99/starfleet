import { useEffect, type ReactNode } from "react";
import type { CardState, Status } from "./api";

export const STATUS_LABEL: Record<Status, string> = {
  blocked: "Needs you",
  done: "Finished",
  working: "Working",
  idle: "Idle",
  unknown: "Idle",
  offline: "Ended",
};

export function StatusPill({ status }: { status: Status }) {
  return (
    <span className={`pill status-${status}`}>
      <span className="dot" />
      {STATUS_LABEL[status]}
    </span>
  );
}

/** A card's derived state, which is coarser than a session's status. */
export const CARD_STATE_LABEL: Record<CardState, string> = {
  blocked: "Needs you",
  interrupted: "Interrupted",
  done: "Finished",
  paused: "Paused",
  working: "Working",
  idle: "Idle",
  offline: "Offline",
  empty: "No sessions",
};

export function CardStatePill({ state }: { state: CardState }) {
  return (
    <span className={`pill card-state-${state}`}>
      <span className="dot" />
      {CARD_STATE_LABEL[state]}
    </span>
  );
}

export function timeAgo(ms: number | null): string {
  if (!ms) return "";
  const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

export function Modal(props: { title: string; onClose: () => void; children: ReactNode; wide?: boolean }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && props.onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props.onClose]);
  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && props.onClose()}>
      <div className={`modal${props.wide ? " wide" : ""}`} role="dialog" aria-label={props.title}>
        <header>
          <h2>{props.title}</h2>
          <button className="ghost" onClick={props.onClose} aria-label="Close">
            ✕
          </button>
        </header>
        {props.children}
      </div>
    </div>
  );
}

export function ErrorNote({ text }: { text: string | null }) {
  return text ? <p className="error-note">{text}</p> : null;
}
