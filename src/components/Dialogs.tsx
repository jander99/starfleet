import { useState } from "react";
import { backend, errorText, type Card, type DispatchResult, type Roster } from "../api";
import { ErrorNote, Modal } from "../ui";

const NEW_CARD = "__new__";
const AGENTS = ["claude", "opencode", "codex"];

export function CardDialog(props: { card: Card | null; onClose: () => void }) {
  const { card } = props;
  const [title, setTitle] = useState(card?.title ?? "");
  const [cwd, setCwd] = useState(card?.cwd ?? "");
  const [notes, setNotes] = useState(card?.notes ?? "");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const save = async () => {
    setBusy(true);
    try {
      if (card) await backend.updateCard(card.id, { title, cwd, notes });
      else await backend.createCard(title, cwd, notes);
      props.onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <Modal title={card ? "Edit card" : "New card"} onClose={props.onClose}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <label>
          Title
          <input value={title} onChange={(e) => setTitle(e.target.value)} autoFocus placeholder="What is this piece of work?" />
        </label>
        <label>
          Working folder
          <input value={cwd} onChange={(e) => setCwd(e.target.value)} placeholder="~/meta/microservices/order-up-api" spellCheck={false} />
          <small>Every session for this card starts here. On Windows, use the path inside WSL.</small>
        </label>
        <label>
          Notes
          <textarea value={notes} onChange={(e) => setNotes(e.target.value)} rows={3} />
        </label>
        <ErrorNote text={error} />
        <div className="form-actions">
          {card &&
            (confirmDelete ? (
              <button
                type="button"
                className="danger"
                onClick={() => backend.deleteCard(card.id).then(props.onClose, (e) => setError(errorText(e)))}
              >
                Delete card, keep sessions running
              </button>
            ) : (
              <button type="button" className="ghost" onClick={() => setConfirmDelete(true)}>
                Delete…
              </button>
            ))}
          <span className="spacer" />
          <button type="button" className="ghost" onClick={props.onClose}>
            Cancel
          </button>
          <button type="submit" className="primary" disabled={busy}>
            {card ? "Save" : "Create card"}
          </button>
        </div>
      </form>
    </Modal>
  );
}

export function DispatchDialog(props: {
  roster: Roster;
  cards: Card[];
  stationId: string | null;
  cardId: string | null;
  onClose: () => void;
  onDispatched: (result: DispatchResult) => void;
}) {
  const openCards = props.cards.filter((c) => !c.done);
  const stations = props.roster.locations.flatMap((l) => l.stations);
  const [stationId, setStationId] = useState(props.stationId ?? stations[0]?.id ?? "");
  const [cardId, setCardId] = useState(props.cardId ?? openCards[0]?.id ?? NEW_CARD);
  const [newTitle, setNewTitle] = useState("");
  const [newCwd, setNewCwd] = useState("");
  const [prompt, setPrompt] = useState("");
  const [agent, setAgent] = useState("");
  const [model, setModel] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const station = stations.find((s) => s.id === stationId);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      if (!prompt.trim()) throw "write a prompt for the agent first";
      let target = cardId;
      if (target === NEW_CARD) {
        target = await backend.createCard(newTitle, newCwd, "");
        setCardId(target); // A retry after a failed dispatch reuses the card.
      }
      const result = await backend.dispatch({
        card_id: target,
        station_id: stationId,
        prompt,
        agent: agent || null,
        model: model || null,
      });
      props.onDispatched(result);
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <Modal title="Dispatch an agent" onClose={props.onClose} wide>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <div className="form-row">
          <label>
            Station
            <select value={stationId} onChange={(e) => setStationId(e.target.value)}>
              {props.roster.locations.map((l) => (
                <optgroup key={l.id} label={l.name}>
                  {l.stations.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name} · {s.duty}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
          </label>
          <label>
            Card
            <select value={cardId} onChange={(e) => setCardId(e.target.value)}>
              {openCards.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.title}
                </option>
              ))}
              <option value={NEW_CARD}>New card…</option>
            </select>
          </label>
        </div>
        {cardId === NEW_CARD && (
          <div className="form-row">
            <label>
              Card title
              <input value={newTitle} onChange={(e) => setNewTitle(e.target.value)} placeholder="What is this piece of work?" />
            </label>
            <label>
              Working folder
              <input value={newCwd} onChange={(e) => setNewCwd(e.target.value)} placeholder="~/meta/ui/site" spellCheck={false} />
            </label>
          </div>
        )}
        {station && (
          <details className="brief">
            <summary>
              Standing brief for {station.name} ({station.post})
            </summary>
            <p>{station.prompt || "No standing brief; only your prompt is sent."}</p>
          </details>
        )}
        <label>
          Prompt
          <textarea value={prompt} onChange={(e) => setPrompt(e.target.value)} rows={6} autoFocus placeholder="What should this agent do?" />
        </label>
        <div className="form-row">
          <label>
            Agent
            <select value={agent} onChange={(e) => setAgent(e.target.value)}>
              <option value="">Station default ({station?.agent})</option>
              {AGENTS.map((a) => (
                <option key={a} value={a}>
                  {a}
                </option>
              ))}
            </select>
          </label>
          <label>
            Model
            <input value={model} onChange={(e) => setModel(e.target.value)} placeholder={station?.model ?? "Agent's own default"} spellCheck={false} />
          </label>
        </div>
        <ErrorNote text={error} />
        <div className="form-actions">
          <span className="spacer" />
          <button type="button" className="ghost" onClick={props.onClose}>
            Cancel
          </button>
          <button type="submit" className="primary" disabled={busy}>
            {busy ? "Starting…" : "Dispatch"}
          </button>
        </div>
      </form>
    </Modal>
  );
}
