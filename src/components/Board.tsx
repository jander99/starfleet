import { useState } from "react";
import { backend, errorText, type BoardView, type Card, type Contact, type Session } from "../api";
import { ErrorNote, StatusPill, timeAgo } from "../ui";

function SessionRow({ session, onOpen }: { session: Session; onOpen: () => void }) {
  return (
    <li>
      <button className="session-row" onClick={onOpen}>
        <StatusPill status={session.status} />
        <span className="session-main">
          <span className="session-title">
            {session.station_name ?? session.title ?? session.agent ?? "Session"}
            <span className="muted"> · {session.agent ?? "agent"} · {timeAgo(session.created_at)}</span>
          </span>
          {session.prompt && <span className="session-prompt ellipsis">{session.prompt}</span>}
          {session.pending_prompt && <span className="held">Prompt held back, waiting on the agent</span>}
          {!session.live && session.output_at && <span className="muted">Output saved</span>}
        </span>
      </button>
    </li>
  );
}

function CardPanel(props: {
  card: Card;
  onSession: (linkId: string) => void;
  onDispatch: (cardId: string) => void;
  onEdit: (card: Card) => void;
}) {
  const { card } = props;
  const [error, setError] = useState<string | null>(null);
  const act = (p: Promise<unknown>) => p.then(() => setError(null), (e) => setError(errorText(e)));
  return (
    <article className={`card${card.done ? " is-done" : ""}`}>
      <header>
        <div className="card-title">
          <h3>{card.title}</h3>
          <code className="ellipsis" title={card.cwd}>
            {card.cwd}
          </code>
        </div>
        <div className="card-actions">
          <button className="ghost" onClick={() => props.onEdit(card)}>
            Edit
          </button>
          <button className="ghost" onClick={() => act(backend.updateCard(card.id, { done: !card.done }))}>
            {card.done ? "Reopen" : "Mark done"}
          </button>
        </div>
      </header>
      {card.notes && <p className="notes">{card.notes}</p>}
      {card.sessions.length > 0 ? (
        <ul className="sessions">
          {card.sessions.map((s) => (
            <SessionRow key={s.link_id} session={s} onOpen={() => props.onSession(s.link_id)} />
          ))}
        </ul>
      ) : (
        <p className="empty">No sessions yet.</p>
      )}
      <ErrorNote text={error} />
      {!card.done && (
        <footer>
          <button onClick={() => props.onDispatch(card.id)}>Dispatch an agent</button>
        </footer>
      )}
    </article>
  );
}

function ContactRow({ contact, cards, onOpen }: { contact: Contact; cards: Card[]; onOpen: () => void }) {
  const [error, setError] = useState<string | null>(null);
  return (
    <li className="contact">
      <button className="session-row" onClick={onOpen}>
        <StatusPill status={contact.status} />
        <span className="session-main">
          <span className="session-title">
            {contact.title ?? contact.workspace ?? contact.pane_id}
            <span className="muted"> · {contact.agent ?? "agent"}</span>
          </span>
          {contact.cwd && <code className="ellipsis">{contact.cwd}</code>}
        </span>
      </button>
      <select
        aria-label="Attach this session to a card"
        value=""
        disabled={cards.length === 0}
        onChange={(e) =>
          e.target.value &&
          backend.attach(e.target.value, contact.pane_id).then(() => setError(null), (err) => setError(errorText(err)))
        }
      >
        <option value="">{cards.length ? "Attach to a card…" : "Create a card to attach"}</option>
        {cards.map((c) => (
          <option key={c.id} value={c.id}>
            {c.title}
          </option>
        ))}
      </select>
      <ErrorNote text={error} />
    </li>
  );
}

export function Board(props: {
  view: BoardView;
  onSession: (linkId: string) => void;
  onContact: (paneId: string) => void;
  onDispatch: (cardId: string) => void;
  onNewCard: () => void;
  onEdit: (card: Card) => void;
}) {
  const open = props.view.cards.filter((c) => !c.done);
  const done = props.view.cards.filter((c) => c.done);
  const [showDone, setShowDone] = useState(false);
  return (
    <div className="board">
      <section className="board-main">
        <div className="section-head">
          <h1>Mission board</h1>
          <button onClick={props.onNewCard}>New card</button>
        </div>
        {open.length === 0 && (
          <p className="empty big">No open cards. A card is one piece of work, tied to a folder, with any number of agent sessions.</p>
        )}
        <div className="cards">
          {open.map((c) => (
            <CardPanel key={c.id} card={c} onSession={props.onSession} onDispatch={props.onDispatch} onEdit={props.onEdit} />
          ))}
        </div>
        {done.length > 0 && (
          <>
            <button className="ghost toggle" onClick={() => setShowDone(!showDone)}>
              {showDone ? "Hide" : "Show"} {done.length} done {done.length === 1 ? "card" : "cards"}
            </button>
            {showDone && (
              <div className="cards">
                {done.map((c) => (
                  <CardPanel key={c.id} card={c} onSession={props.onSession} onDispatch={props.onDispatch} onEdit={props.onEdit} />
                ))}
              </div>
            )}
          </>
        )}
      </section>
      <aside className="tray">
        <h2>Unassigned contacts</h2>
        <p className="muted">Agents running in Herdr that are not on any card.</p>
        {props.view.unassigned.length === 0 ? (
          <p className="empty">{props.view.connected ? "None. Every running agent is on a card." : "Herdr is not connected."}</p>
        ) : (
          <ul className="sessions">
            {props.view.unassigned.map((u) => (
              <ContactRow key={u.pane_id} contact={u} cards={open} onOpen={() => props.onContact(u.pane_id)} />
            ))}
          </ul>
        )}
      </aside>
    </div>
  );
}
