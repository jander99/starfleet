import type { BoardView, Location, Roster, Session, Station, Status } from "../api";
import { StatusPill } from "../ui";
import { useSky, type Frame } from "../sky";
import dawn from "../assets/earth/earth-dawn.png";
import day from "../assets/earth/earth-day.png";
import dusk from "../assets/earth/earth-dusk.png";
import night from "../assets/earth/earth-night.png";

const EARTH: Record<Frame, string> = { dawn, day, dusk, night };

/** Live sessions that were dispatched from a station at this location. */
function sessionsAt(view: BoardView | null, prefix: string): Session[] {
  if (!view) return [];
  return view.cards.flatMap((c) => c.sessions).filter((s) => s.live && s.station_id?.startsWith(prefix));
}

function tally(sessions: Session[]): [Status, number][] {
  const order: Status[] = ["blocked", "done", "working"];
  return order.map((s) => [s, sessions.filter((x) => x.status === s).length] as [Status, number]).filter(([, n]) => n > 0);
}

const SITES: Record<string, { x: number; y: number; labelY: number }> = {
  academy: { x: 150, y: 190, labelY: 62 },
  daystrom: { x: 360, y: 300, labelY: 66 },
  enterprise: { x: 610, y: 150, labelY: 92 },
  ds9: { x: 800, y: 380, labelY: 112 },
};

function Shapes() {
  return (
    <defs>
      {/* Academy: a colonnaded hall */}
      <g id="shape-academy">
        <path d="M-58 34 H58 M-52 34 V-6 M52 34 V-6 M-58 -6 H58 L0 -40 Z" />
        <path d="M-34 34 V-6 M-16 34 V-6 M16 34 V-6 M34 34 V-6 M0 34 V-6" />
        <path d="M-66 42 H66" />
      </g>
      {/* Daystrom: a research tower with a lattice core */}
      <g id="shape-daystrom">
        <path d="M0 -46 L40 -23 V23 L0 46 L-40 23 V-23 Z" />
        <path d="M0 -46 V46 M-40 -23 L40 23 M40 -23 L-40 23" />
        <circle r="13" />
      </g>
      {/* Enterprise: saucer, neck, secondary hull, nacelles */}
      <g id="shape-enterprise">
        <ellipse cx="-34" cy="-24" rx="104" ry="30" />
        <ellipse cx="-34" cy="-24" rx="62" ry="17" />
        <ellipse cx="-34" cy="-26" rx="16" ry="5" />
        <path d="M10 2 L34 26 M36 -4 L58 22" />
        <ellipse cx="72" cy="38" rx="52" ry="15" />
        <path d="M70 24 L96 -14 M92 26 L112 -12" />
        <rect x="52" y="-30" width="118" height="15" rx="7.5" />
        <path d="M70 -30 V-15" />
      </g>
      {/* Deep Space Nine: rings, spokes and docking pylons, from above */}
      <g id="shape-ds9">
        <circle r="86" />
        <circle r="78" />
        <circle r="42" />
        <circle r="15" />
        <path d="M0 -15 V-42 M13 7.5 L36.4 21 M-13 7.5 L-36.4 21" />
        <path d="M0 -42 V-78 M36.4 21 L67.5 39 M-36.4 21 L-67.5 39" />
        <path d="M-20 -84 Q0 -112 20 -84 M83 25 Q97 56 63 59 M-83 25 Q-97 56 -63 59" />
      </g>
    </defs>
  );
}

/** Earth in pixel art, lit for the local time, bleeding off the bottom-left corner of the window. */
function EarthLimb() {
  const sky = useSky();
  return (
    <div className="earth-limb" aria-hidden="true">
      <img src={EARTH[sky.from]} alt="" />
      {sky.mix > 0 && <img src={EARTH[sky.to]} alt="" style={{ opacity: sky.mix }} />}
    </div>
  );
}

export function SectorMap(props: { roster: Roster; view: BoardView | null; onOpen: (locationId: string) => void }) {
  return (
    <div className="map-wrap">
      <EarthLimb />
      <svg className="map" viewBox="0 0 1000 600" role="group" aria-label="Sector map">
        <Shapes />

        {props.roster.locations.map((loc) => {
          const site = SITES[loc.id];
          if (!site) return null;
          const counts = tally(sessionsAt(props.view, `${loc.id}.`));
          return (
            <g
              key={loc.id}
              className="site"
              transform={`translate(${site.x} ${site.y})`}
              role="button"
              tabIndex={0}
              aria-label={`${loc.name}, ${loc.stations.length} stations`}
              onClick={() => props.onOpen(loc.id)}
              onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && props.onOpen(loc.id)}
            >
              <g className="site-body">
                <rect className="hit" x="-125" y="-120" width="320" height="250" />
                <use href={`#shape-${loc.id}`} className="depth" transform="translate(7 9)" />
                <use href={`#shape-${loc.id}`} className="hull" />
                <text className="site-name" y={site.labelY + 18}>
                  {loc.name}
                </text>
                <text className="site-sub" y={site.labelY + 36}>
                  {loc.subtitle} · {loc.stations.length} {loc.stations.length === 1 ? "station" : "stations"}
                </text>
                {counts.map(([status, n], i) => (
                  <g key={status} className={`badge status-${status}`} transform={`translate(${i * 34 - (counts.length - 1) * 17} ${site.labelY + 56})`}>
                    <rect x="-14" y="-11" width="28" height="20" rx="10" />
                    <text y="4">{n}</text>
                  </g>
                ))}
              </g>
            </g>
          );
        })}
      </svg>
      <p className="hint">Choose where the work goes. Each location has stations, and each station starts an agent with its own brief.</p>
    </div>
  );
}

export function LocationView(props: {
  location: Location;
  view: BoardView | null;
  onBack: () => void;
  onStation: (station: Station) => void;
  onSession: (linkId: string) => void;
}) {
  const { location } = props;
  return (
    <div className="location">
      <header className="location-head">
        <button className="ghost" onClick={props.onBack}>
          ← Sector map
        </button>
        <div>
          <h1>{location.name}</h1>
          <p className="muted">{location.subtitle}</p>
        </div>
      </header>
      <div className="stations">
        {location.stations.map((st) => {
          const live = sessionsAt(props.view, st.id).filter((s) => s.station_id === st.id);
          return (
            <div key={st.id} className="station">
              <button className="console" onClick={() => props.onStation(st)}>
                <span className="post">{st.post}</span>
                <span className="officer">{st.name}</span>
                <span className="duty">{st.duty}</span>
                <span className="chip">
                  {st.agent}
                  {st.model ? ` · ${st.model}` : ""}
                </span>
              </button>
              {live.length > 0 && (
                <ul className="station-sessions">
                  {live.map((s) => (
                    <li key={s.link_id}>
                      <button className="link-row" onClick={() => props.onSession(s.link_id)}>
                        <StatusPill status={s.status} />
                        <span className="ellipsis">{s.prompt ?? s.title}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
