import { useEffect, useRef, useState } from "react";
import { backend, errorText, type Status } from "../api";
import { ErrorNote, StatusPill, timeAgo } from "../ui";

export interface DrawerTarget {
  heading: string;
  status: Status;
  paneId: string | null;
  linkId: string | null;
  agent: string | null;
  cardTitle: string | null;
  prompt: string | null;
  pendingPrompt: string | null;
  cwd: string | null;
  lastSeen: number | null;
}

const REPLY_KEYS: { label: string; text: string | null; keys: string[] }[] = [
  { label: "1", text: "1", keys: [] },
  { label: "2", text: "2", keys: [] },
  { label: "3", text: "3", keys: [] },
  { label: "Enter", text: null, keys: ["enter"] },
  { label: "Esc", text: null, keys: ["esc"] },
];

export function SessionDrawer({ target, onClose }: { target: DrawerTarget; onClose: () => void }) {
  const [tail, setTail] = useState<string | null>(null);
  const [tailError, setTailError] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const output = useRef<HTMLPreElement>(null);
  const { paneId } = target;

  // Poll the output while the drawer is open; a tail is cheap and this view is
  // the only one that needs it.
  useEffect(() => {
    if (!paneId) return;
    let alive = true;
    const load = () =>
      backend.tail(paneId, 200).then(
        (text) => alive && (setTail(text), setTailError(null)),
        (e) => alive && setTailError(errorText(e)),
      );
    load();
    const timer = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [paneId]);

  useEffect(() => {
    const el = output.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [tail]);

  const act = (p: Promise<unknown>, done?: string) =>
    p.then(
      () => (setError(null), setNote(done ?? null)),
      (e) => (setNote(null), setError(errorText(e))),
    );
  const blocked = target.status === "blocked";

  return (
    <aside className="drawer" aria-label="Session">
      <header>
        <div>
          <h2>{target.heading}</h2>
          <p className="muted">
            {[target.cardTitle, target.agent, paneId].filter(Boolean).join(" · ")}
            {!paneId && target.lastSeen ? ` · last seen ${timeAgo(target.lastSeen)}` : ""}
          </p>
        </div>
        <button className="ghost" onClick={onClose} aria-label="Close">
          ✕
        </button>
      </header>
      <div className="drawer-bar">
        <StatusPill status={target.status} />
        <span className="spacer" />
        {paneId && (
          <button className="ghost" onClick={() => act(backend.focus(paneId), "Brought to the front in Herdr.")}>
            Jump to pane
          </button>
        )}
        {target.linkId && (
          <button className="ghost" onClick={() => act(backend.detach(target.linkId!)).then(onClose)}>
            {paneId ? "Detach from card" : "Remove from card"}
          </button>
        )}
      </div>

      {target.prompt && (
        <details className="brief">
          <summary>Original prompt</summary>
          <p>{target.prompt}</p>
        </details>
      )}

      {paneId ? (
        <pre className="output" ref={output} tabIndex={0}>
          {tailError ?? tail ?? "Loading output…"}
        </pre>
      ) : (
        <p className="empty">This session is no longer running in Herdr, so there is no output to show.</p>
      )}

      {paneId && target.pendingPrompt && target.linkId && (
        <div className="banner">
          <p>The prompt was held back because the agent was waiting on a question.</p>
          <button disabled={blocked} onClick={() => act(backend.sendPending(target.linkId!), "Prompt sent.")}>
            Send the prompt now
          </button>
        </div>
      )}

      {paneId && blocked && (
        <div className="reply">
          <span className="muted">Answer the agent:</span>
          {REPLY_KEYS.map((k) => (
            <button key={k.label} onClick={() => act(backend.replyKeys(paneId, k.text, k.keys))}>
              {k.label}
            </button>
          ))}
        </div>
      )}

      {paneId && (
        <form
          className="followup"
          onSubmit={(e) => {
            e.preventDefault();
            act(backend.followUp(paneId, message).then(() => setMessage("")));
          }}
        >
          <textarea
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            rows={3}
            placeholder={blocked ? "Answer the agent's question first" : "Send a follow-up message"}
            disabled={blocked}
          />
          <button type="submit" className="primary" disabled={blocked || !message.trim()}>
            Send
          </button>
        </form>
      )}
      <ErrorNote text={error} />
      {note && <p className="muted note">{note}</p>}
    </aside>
  );
}
