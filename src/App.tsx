import { useCallback, useEffect, useState } from "react";
import { backend, errorText, isDemo, type BoardView, type Card, type Check, type Roster, type Status } from "./api";
import { Board } from "./components/Board";
import { CardDialog, DispatchDialog } from "./components/Dialogs";
import { Backdrop } from "./components/Backdrop";
import { LocationView, SectorMap } from "./components/MapView";
import { SessionDrawer, type DrawerTarget } from "./components/SessionDrawer";
import { STATUS_LABEL } from "./ui";

type Tab = "map" | "board" | "check";
type Drawer = { linkId: string } | { paneId: string } | null;

function drawerTarget(view: BoardView, drawer: Drawer): DrawerTarget | null {
  if (!drawer) return null;
  if ("linkId" in drawer) {
    for (const card of view.cards) {
      const s = card.sessions.find((x) => x.link_id === drawer.linkId);
      if (s) {
        return {
          heading: s.station_name ?? s.title ?? "Session",
          status: s.status,
          paneId: s.pane_id,
          linkId: s.link_id,
          agent: s.agent,
          cardTitle: card.title,
          prompt: s.prompt,
          pendingPrompt: s.pending_prompt,
          cwd: s.cwd,
          lastSeen: s.last_seen,
        };
      }
    }
    return null;
  }
  const c = view.unassigned.find((u) => u.pane_id === drawer.paneId);
  if (!c) {
    // It was just attached to a card: follow it there.
    const moved = view.cards.flatMap((card) => card.sessions).find((s) => s.pane_id === drawer.paneId);
    return moved ? drawerTarget(view, { linkId: moved.link_id }) : null;
  }
  return {
    heading: c.title ?? c.workspace ?? "Unassigned session",
    status: c.status,
    paneId: c.pane_id,
    linkId: null,
    agent: c.agent,
    cardTitle: "Unassigned",
    prompt: null,
    pendingPrompt: null,
    cwd: c.cwd,
    lastSeen: null,
  };
}

function Preflight({ checks, running, onRun }: { checks: Check[] | null; running: boolean; onRun: () => void }) {
  const icon = { ok: "✓", warn: "!", fail: "✕" };
  const fails = checks?.filter((c) => c.status === "fail").length ?? 0;
  const warns = checks?.filter((c) => c.status === "warn").length ?? 0;
  return (
    <div className="preflight">
      <div className="section-head">
        <h1>System check</h1>
        <button onClick={onRun} disabled={running}>
          {running ? "Checking…" : "Run again"}
        </button>
      </div>
      {checks && (
        <p className={fails ? "summary bad" : "summary good"}>
          {fails
            ? `${fails} required ${fails === 1 ? "item needs" : "items need"} attention before agents can be dispatched.`
            : warns
              ? `Ready. ${warns} optional ${warns === 1 ? "item is" : "items are"} not set up.`
              : "All systems ready."}
        </p>
      )}
      <ul className="checks">
        {(checks ?? []).map((c) => (
          <li key={c.id} className={`check ${c.status}`}>
            <span className="check-icon" aria-label={c.status}>
              {icon[c.status]}
            </span>
            <div>
              <strong>{c.label}</strong>
              <p>{c.detail}</p>
              {c.fix && <p className="fix">{c.fix}</p>}
            </div>
          </li>
        ))}
      </ul>
      {!checks && <p className="muted">Checking the environment…</p>}
    </div>
  );
}

export default function App() {
  const [view, setView] = useState<BoardView | null>(null);
  const [roster, setRoster] = useState<Roster | null>(null);
  const [fatal, setFatal] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("map");
  const [locationId, setLocationId] = useState<string | null>(null);
  const [drawer, setDrawer] = useState<Drawer>(null);
  const [dispatch, setDispatch] = useState<{ stationId: string | null; cardId: string | null } | null>(null);
  const [cardDialog, setCardDialog] = useState<{ card: Card | null } | null>(null);
  const [checks, setChecks] = useState<Check[] | null>(null);
  const [checking, setChecking] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const runChecks = useCallback(() => {
    setChecking(true);
    return backend
      .preflight()
      .then((c) => (setChecks(c), c))
      .finally(() => setChecking(false));
  }, []);

  useEffect(() => {
    const load = () => backend.view().then(setView, (e) => setFatal(errorText(e)));
    load();
    backend.roster().then(setRoster, (e) => setFatal(errorText(e)));
    // Land on the system check when something required is missing.
    runChecks().then((c) => c.some((x) => x.status === "fail") && setTab("check"), () => {});
    return backend.onChange(load);
  }, [runChecks]);

  if (fatal) return <div className="fatal">Starfleet could not start: {fatal}</div>;
  if (!view || !roster) return <div className="fatal muted">Starting…</div>;

  const location = roster.locations.find((l) => l.id === locationId) ?? null;
  const target = drawerTarget(view, drawer);
  const counters: [Status, number][] = [
    ["blocked", view.counts.blocked],
    ["done", view.counts.done],
    ["working", view.counts.working],
    ["idle", view.counts.idle],
  ];
  const checkFails = checks?.filter((c) => c.status === "fail").length ?? 0;

  return (
    <>
      <Backdrop />
      <div className="app">
        <header className="topbar">
          <div className="brand">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 2 L20 21 L12 16 L4 21 Z" />
            </svg>
            STARFLEET
          </div>
          <nav>
            {(
              [
                ["map", "Sector map"],
                ["board", "Mission board"],
                ["check", "System check"],
              ] as [Tab, string][]
            ).map(([id, label]) => (
              <button key={id} className={tab === id ? "tab active" : "tab"} onClick={() => (setTab(id), id === "map" && setLocationId(null))}>
                {label}
                {id === "check" && checkFails > 0 && <span className="tab-flag">{checkFails}</span>}
              </button>
            ))}
          </nav>
          <div className="counters">
            {counters.map(([status, n]) => (
              <button key={status} className={`counter status-${status}${n ? "" : " zero"}`} onClick={() => setTab("board")}>
                <span className="dot" />
                {STATUS_LABEL[status]} <b>{n}</b>
              </button>
            ))}
            <span className={view.connected ? "conn ok" : "conn bad"} title={view.error ?? view.endpoint}>
              {view.connected ? `Herdr ${view.herdr_version ?? ""}` : "Herdr offline"}
            </span>
          </div>
        </header>

        {isDemo && <div className="strip">Demo data. This is the UI running in a browser without the app shell or Herdr.</div>}
        {!view.connected && view.error && (
          <div className="strip bad">
            Not connected to Herdr: {view.error}. Retrying.{" "}
            <button className="ghost" onClick={() => setTab("check")}>
              Open system check
            </button>
          </div>
        )}
        {notice && (
          <div className="strip warn">
            {notice}{" "}
            <button className="ghost" onClick={() => setNotice(null)}>
              Dismiss
            </button>
          </div>
        )}

        <main className={target ? "with-drawer" : ""}>
          <div className="content">
            {tab === "map" &&
              (location ? (
                <LocationView
                  location={location}
                  view={view}
                  onBack={() => setLocationId(null)}
                  onStation={(st) => setDispatch({ stationId: st.id, cardId: null })}
                  onSession={(linkId) => setDrawer({ linkId })}
                />
              ) : (
                <SectorMap roster={roster} view={view} onOpen={setLocationId} />
              ))}
            {tab === "board" && (
              <Board
                view={view}
                onSession={(linkId) => setDrawer({ linkId })}
                onContact={(paneId) => setDrawer({ paneId })}
                onDispatch={(cardId) => setDispatch({ stationId: null, cardId })}
                onNewCard={() => setCardDialog({ card: null })}
                onEdit={(card) => setCardDialog({ card })}
              />
            )}
            {tab === "check" && <Preflight checks={checks} running={checking} onRun={runChecks} />}
          </div>
          {target && <SessionDrawer key={target.linkId ?? target.paneId} target={target} onClose={() => setDrawer(null)} />}
        </main>

        {dispatch && (
          <DispatchDialog
            roster={roster}
            cards={view.cards}
            stationId={dispatch.stationId}
            cardId={dispatch.cardId}
            onClose={() => setDispatch(null)}
            onDispatched={(result) => {
              setDispatch(null);
              setNotice(result.warning);
              setTab("board");
              setDrawer({ linkId: result.link_id });
            }}
          />
        )}
        {cardDialog && <CardDialog card={cardDialog.card} onClose={() => setCardDialog(null)} />}
      </div>
    </>
  );
}
