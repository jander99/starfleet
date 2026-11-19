import { useEffect, useRef, useState } from "react";
import { backend, errorText, type Card, type LogEntry } from "../api";
import { rounds } from "../log";
import { ErrorNote, timeAgo } from "../ui";

const KIND_LABEL: Record<string, string> = {
  note: "note",
  decision: "decision",
  prompt: "order",
  result: "result",
  artifact: "artifact",
};

function Entry({ e }: { e: LogEntry }) {
  return (
    <li className={`log-entry kind-${e.kind}`}>
      <div className="log-head">
        <strong>{e.owner}</strong>
        <span className="muted">
          {KIND_LABEL[e.kind] ?? e.kind} · {timeAgo(e.at)}
        </span>
      </div>
      <p className="log-text">{e.text}</p>
    </li>
  );
}

/** The Captain's Log for one card: the timeline, plus a box to add a note. */
export function CardConsole({ card, onClose }: { card: Card; onClose: () => void }) {
  const [entries, setEntries] = useState<LogEntry[] | null>(null);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);

  const [busy, setBusy] = useState(false);
  // Only the newest read may write `entries`: a slow earlier read must not
  // overwrite a newer one, and nothing writes after unmount.
  const reads = useRef(0);
  const load = () => {
    const mine = ++reads.current;
    return backend.readLog(card.id).then(
      (e) => mine === reads.current && setEntries(e),
      (e) => mine === reads.current && setError(errorText(e)),
    );
  };
  useEffect(() => {
    load();
    return () => {
      reads.current++;
    };
  }, [card.id]);

  const add = () => {
    if (busy || !text.trim()) return;
    setBusy(true);
    backend
      .appendLog(card.id, text)
      .then(() => (setText(""), setError(null), load()), (e) => setError(errorText(e)))
      .finally(() => setBusy(false));
  };

  const all = entries ?? [];
  const grouped = rounds(all);
  // Collapse every round but the newest, unless asked for all.
  const shown = showAll ? grouped : grouped.slice(-1);
  const hidden = grouped.length - shown.length;

  return (
    <aside className="console">
      <header>
        <h2>{card.title}</h2>
        <button className="ghost" onClick={onClose} aria-label="Close">
          ✕
        </button>
      </header>
      <p className="muted">
        Captain&apos;s Log · {card.state} · {card.cwd}
      </p>
      {entries === null && !error && <p className="muted">Loading the log…</p>}
      {hidden > 0 && (
        <button className="ghost" onClick={() => setShowAll(true)}>
          Show {hidden} earlier {hidden === 1 ? "round" : "rounds"}
        </button>
      )}
      {entries !== null && all.length === 0 && <p className="empty">Nothing logged yet. A note here outlives the agents.</p>}
      {shown.map((r, i) => (
        <ol className="log-round" key={r[0]?.id ?? i}>
          {r.map((e) => (
            <Entry key={e.id} e={e} />
          ))}
        </ol>
      ))}
      <footer className="log-write">
        <textarea
          value={text}
          placeholder="Add a note or decision to the log…"
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && (e.metaKey || e.ctrlKey) && add()}
        />
        <button onClick={add} disabled={busy || !text.trim()}>
          Log it
        </button>
      </footer>
      <ErrorNote text={error} />
    </aside>
  );
}
