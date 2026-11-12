import type { BoardView, Location, Roster, Session, Station, Status } from "../api";
import { StatusPill } from "../ui";
import { useSky, type Frame } from "../sky";
import dawn from "../assets/earth/earth-dawn.png";
import day from "../assets/earth/earth-day.png";
import dusk from "../assets/earth/earth-dusk.png";
import night from "../assets/earth/earth-night.png";
import academy from "../assets/sites/academy.png";
import daystrom from "../assets/sites/daystrom.png";
import enterprise from "../assets/sites/enterprise.png";
import ds9 from "../assets/sites/ds9.png";

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

/** Map units per sprite pixel; one scale for every sprite keeps the pixels the same size. */
const SPRITE_SCALE = 2;

/** Where each location sits on the map, its pixel-art sprite and that sprite's size in pixels. */
const SITES: Record<string, { x: number; y: number; sprite: string; w: number; h: number }> = {
  academy: { x: 150, y: 190, sprite: academy, w: 120, h: 80 },
  daystrom: { x: 360, y: 300, sprite: daystrom, w: 80, h: 112 },
  enterprise: { x: 610, y: 150, sprite: enterprise, w: 160, h: 80 },
  ds9: { x: 800, y: 380, sprite: ds9, w: 112, h: 112 },
};

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
        {props.roster.locations.map((loc) => {
          const site = SITES[loc.id];
          if (!site) return null;
          const counts = tally(sessionsAt(props.view, `${loc.id}.`));
          const w = site.w * SPRITE_SCALE;
          const h = site.h * SPRITE_SCALE;
          const labelY = h / 2 + 6;
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
                <rect className="hit" x={-w / 2 - 10} y={-h / 2 - 10} width={w + 20} height={h + 80} />
                <image className="sprite" href={site.sprite} x={-w / 2} y={-h / 2} width={w} height={h} />
                <text className="site-name" y={labelY + 18}>
                  {loc.name}
                </text>
                <text className="site-sub" y={labelY + 36}>
                  {loc.subtitle} · {loc.stations.length} {loc.stations.length === 1 ? "station" : "stations"}
                </text>
                {counts.map(([status, n], i) => (
                  <g key={status} className={`badge status-${status}`} transform={`translate(${i * 34 - (counts.length - 1) * 17} ${labelY + 56})`}>
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
