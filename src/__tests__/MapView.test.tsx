import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { LocationView, SectorMap } from "../components/MapView";
import { makeCard, makeRoster, makeSession, makeStation, makeView } from "./helpers";

describe("SectorMap", () => {
  it("renders a site per known location with its station count", () => {
    render(<SectorMap roster={makeRoster()} view={null} onOpen={() => {}} />);
    expect(screen.getByLabelText("Enterprise, 1 stations")).toBeInTheDocument();
    expect(screen.getByText(/Deep space · 1 station/)).toBeInTheDocument();
  });

  it("opens a location on click and on Enter/Space", () => {
    const onOpen = vi.fn();
    render(<SectorMap roster={makeRoster()} view={null} onOpen={onOpen} />);
    const site = screen.getByLabelText("Enterprise, 1 stations");
    fireEvent.click(site);
    fireEvent.keyDown(site, { key: "Enter" });
    fireEvent.keyDown(site, { key: " " });
    fireEvent.keyDown(site, { key: "a" });
    expect(onOpen.mock.calls).toEqual([["enterprise"], ["enterprise"], ["enterprise"]]);
  });

  it("shows live-session badges per location", () => {
    const view = makeView({
      cards: [
        makeCard({
          sessions: [
            makeSession({ link_id: "1", status: "blocked", station_id: "enterprise.data" }),
            makeSession({ link_id: "2", status: "working", station_id: "enterprise.ops" }),
            makeSession({ link_id: "3", status: "working", station_id: "enterprise.ops" }),
            makeSession({ link_id: "4", status: "idle", station_id: "enterprise.ops" }),
            makeSession({ link_id: "5", status: "working", station_id: "other.thing" }),
            makeSession({ link_id: "6", status: "working", station_id: "enterprise.ops", live: false }),
          ],
        }),
      ],
    });
    const { container } = render(<SectorMap roster={makeRoster()} view={view} onOpen={() => {}} />);
    const badges = [...container.querySelectorAll(".badge")];
    // blocked 1, working 2 — done 0 is filtered out, idle is not tallied.
    expect(badges.map((b) => b.textContent)).toEqual(["1", "2"]);
    expect(badges[0].classList.contains("status-blocked")).toBe(true);
    expect(badges[1].classList.contains("status-working")).toBe(true);
  });

  it("skips a location with no known site", () => {
    const roster = makeRoster({
      locations: [{ id: "nowhere", name: "Nowhere", subtitle: "", stations: [] }],
    });
    const { container } = render(<SectorMap roster={roster} view={null} onOpen={() => {}} />);
    expect(container.querySelectorAll(".site")).toHaveLength(0);
  });
});

describe("LocationView", () => {
  const location = makeRoster().locations[0];

  it("shows the location and its stations", () => {
    render(<LocationView location={location} view={null} onBack={() => {}} onStation={() => {}} onSession={() => {}} />);
    expect(screen.getByRole("heading", { name: "Enterprise" })).toBeInTheDocument();
    expect(screen.getByText("Data")).toBeInTheDocument();
    expect(screen.getByText("Operations")).toBeInTheDocument();
    expect(screen.getByText("claude")).toBeInTheDocument();
  });

  it("shows a station's model when set", () => {
    const loc = { ...location, stations: [makeStation({ model: "opus" })] };
    render(<LocationView location={loc} view={null} onBack={() => {}} onStation={() => {}} onSession={() => {}} />);
    expect(screen.getByText("claude · opus")).toBeInTheDocument();
  });

  it("goes back and opens a station", () => {
    const onBack = vi.fn();
    const onStation = vi.fn();
    render(<LocationView location={location} view={null} onBack={onBack} onStation={onStation} onSession={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /Sector map/ }));
    expect(onBack).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /Data/ }));
    expect(onStation).toHaveBeenCalledWith(location.stations[0]);
  });

  it("lists live sessions at a station and opens one", () => {
    const live = makeSession({ link_id: "lx", status: "blocked", station_id: "enterprise.data", prompt: "please help" });
    const dead = makeSession({ link_id: "ld", station_id: "enterprise.data", live: false });
    const elsewhere = makeSession({ link_id: "le", station_id: "enterprise.other" });
    const view = makeView({ cards: [makeCard({ sessions: [live, dead, elsewhere] })] });
    const onSession = vi.fn();
    render(<LocationView location={location} view={view} onBack={() => {}} onStation={() => {}} onSession={onSession} />);
    expect(screen.getByText("please help")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /Data|please help|Needs you/ }).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByText("please help"));
    expect(onSession).toHaveBeenCalledWith("lx");
  });

  it("falls back to a session title when there is no prompt", () => {
    const s = makeSession({ link_id: "lt", station_id: "enterprise.data", prompt: null, title: "A title" });
    const view = makeView({ cards: [makeCard({ sessions: [s] })] });
    render(<LocationView location={location} view={view} onBack={() => {}} onStation={() => {}} onSession={() => {}} />);
    expect(screen.getByText("A title")).toBeInTheDocument();
  });
});
