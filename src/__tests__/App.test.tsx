import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import App from "../App";
import { backend } from "../api";
import { makeCard, makeContact, makeSession, makeStation, makeView } from "./helpers";

vi.mock("../api", async (importOriginal) => {
  const actual = (await importOriginal()) as Record<string, unknown>;
  const { fakeBackend } = await import("./helpers");
  return { ...actual, backend: fakeBackend, isDemo: true };
});

const mocked = vi.mocked(backend);

beforeEach(() => {
  vi.clearAllMocks();
  mocked.view.mockResolvedValue(makeView());
  mocked.preflight.mockResolvedValue([]);
  mocked.readLog.mockResolvedValue([]);
  mocked.tail.mockResolvedValue("tail text");
});

describe("App", () => {
  it("starts on the sector map", async () => {
    render(<App />);
    expect(await screen.findByLabelText("Sector map")).toBeInTheDocument();
    expect(screen.getByText(/Demo data/)).toBeInTheDocument();
    expect(screen.getByText("Herdr 1.2.3")).toBeInTheDocument();
  });

  it("shows a fatal error when the view cannot load", async () => {
    mocked.view.mockRejectedValueOnce("core failed");
    render(<App />);
    expect(await screen.findByText("Starfleet could not start: core failed")).toBeInTheDocument();
  });

  it("shows a fatal error when the roster cannot load", async () => {
    mocked.roster.mockRejectedValueOnce(new Error("no roster"));
    render(<App />);
    expect(await screen.findByText("Starfleet could not start: no roster")).toBeInTheDocument();
  });

  it("switches between tabs", async () => {
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    expect(screen.getByRole("heading", { name: "Mission board" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "System check" }));
    expect(screen.getByRole("heading", { name: "System check" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Sector map" }));
    expect(screen.getByLabelText("Sector map")).toBeInTheDocument();
  });

  it("lands on the system check when a required check fails", async () => {
    mocked.preflight.mockResolvedValueOnce([
      { id: "a", label: "Token", status: "fail", detail: "missing", fix: "set it" },
    ]);
    render(<App />);
    expect(await screen.findByRole("heading", { name: "System check" })).toBeInTheDocument();
    expect(screen.getByText(/1 required item needs attention/)).toBeInTheDocument();
    expect(screen.getByText("set it")).toBeInTheDocument();
  });

  it("summarizes warnings when nothing is required", async () => {
    mocked.preflight.mockResolvedValueOnce([
      { id: "a", label: "Optional", status: "warn", detail: "d", fix: null },
      { id: "b", label: "Ok", status: "ok", detail: "d", fix: null },
    ]);
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "System check" }));
    expect(screen.getByText("Ready. 1 optional item is not set up.")).toBeInTheDocument();
  });

  it("says all systems ready when everything passes", async () => {
    mocked.preflight.mockResolvedValue([
      { id: "b", label: "Ok", status: "ok", detail: "d", fix: null },
    ]);
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "System check" }));
    expect(screen.getByText("All systems ready.")).toBeInTheDocument();
  });

  it("flags the check tab with the number of failures", async () => {
    mocked.preflight.mockResolvedValue([
      { id: "a", label: "A", status: "fail", detail: "", fix: null },
      { id: "b", label: "B", status: "fail", detail: "", fix: null },
    ]);
    render(<App />);
    const tab = await screen.findByRole("button", { name: /System check/ });
    await waitFor(() => expect(within(tab).getByText("2")).toBeInTheDocument());
  });

  it("reruns the checks from the system check page", async () => {
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "System check" }));
    expect(mocked.preflight).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Run again" }));
    await waitFor(() => expect(mocked.preflight).toHaveBeenCalledTimes(2));
  });

  it("warns when Herdr is down and links to the check", async () => {
    mocked.view.mockResolvedValue(makeView({ connected: false, error: "socket closed", herdr_version: null }));
    render(<App />);
    expect(await screen.findByText(/Not connected to Herdr: socket closed/)).toBeInTheDocument();
    expect(screen.getByText("Herdr offline")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open system check" }));
    expect(screen.getByRole("heading", { name: "System check" })).toBeInTheDocument();
  });

  it("opens a location and comes back", async () => {
    render(<App />);
    fireEvent.click(await screen.findByLabelText(/Enterprise, 1 stations/));
    expect(screen.getByRole("heading", { name: "Enterprise" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /← Sector map/ }));
    expect(screen.getByLabelText("Sector map")).toBeInTheDocument();
  });

  it("counters jump to the board", async () => {
    mocked.view.mockResolvedValue(makeView({ counts: { blocked: 2, done: 0, working: 1, idle: 0, offline: 0 } }));
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: /Needs you 2/ }));
    expect(screen.getByRole("heading", { name: "Mission board" })).toBeInTheDocument();
  });

  it("opens a session drawer from the board and follows it when it moves", async () => {
    const session = makeSession({ link_id: "l1", station_name: "Data", title: null });
    mocked.view.mockResolvedValue(makeView({ cards: [makeCard({ sessions: [session] })] }));
    const { container } = render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(container.querySelector(".session-row")!);
    expect(await screen.findByRole("complementary", { name: "Session" })).toBeInTheDocument();
    expect(await screen.findByText("tail text")).toBeInTheDocument();
  });

  it("resolves an unassigned drawer target and then a moved one", async () => {
    const contact = makeContact({ pane_id: "p9", title: "Loose" });
    mocked.view.mockResolvedValueOnce(makeView({ unassigned: [contact] }));
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(screen.getByText(/Loose/));
    const drawer = await screen.findByRole("complementary", { name: "Session" });
    expect(within(drawer).getByText(/Unassigned/)).toBeInTheDocument();
  });

  it("opens the new-card dialog from the board", async () => {
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(screen.getByRole("button", { name: "New card" }));
    expect(screen.getByRole("heading", { name: "New card" })).toBeInTheDocument();
  });

  it("opens the console for a card", async () => {
    mocked.view.mockResolvedValue(makeView({ cards: [makeCard()] }));
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(screen.getByRole("button", { name: "Card one" }));
    expect(await screen.findByPlaceholderText("Add a note or decision to the log…")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Add a note or decision to the log…")).toBeInTheDocument();
  });

  it("closes the console by clicking the scrim", async () => {
    mocked.view.mockResolvedValue(makeView({ cards: [makeCard()] }));
    const { container } = render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(screen.getByRole("button", { name: "Card one" }));
    await screen.findByPlaceholderText("Add a note or decision to the log…");
    fireEvent.mouseDown(container.querySelector(".console-scrim")!);
    expect(screen.queryByPlaceholderText("Add a note or decision to the log…")).toBeNull();
  });

  it("shows a notice when an artifact cannot be opened, and dismisses it", async () => {
    mocked.openArtifact.mockRejectedValueOnce("no browser");
    const card = makeCard({
      artifacts: [{ id: "a1", kind: "url", label: "Link", target: "https://x", last_status: null, checked_at: null }],
    });
    mocked.view.mockResolvedValue(makeView({ cards: [card] }));
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(screen.getByText("Link"));
    expect(await screen.findByText("no browser")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText("no browser")).toBeNull();
  });

  it("dispatches from a station and opens the new session", async () => {
    mocked.dispatch.mockResolvedValueOnce({ link_id: "l-new", pane_id: "p-new", warning: "heads up" });
    render(<App />);
    fireEvent.click(await screen.findByLabelText(/Enterprise, 1 stations/));
    fireEvent.click(screen.getByRole("button", { name: /Data/ }));
    expect(await screen.findByRole("heading", { name: "Dispatch an agent" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Prompt"), { target: { value: "go" } });
    fireEvent.click(screen.getByRole("button", { name: "Dispatch" }));
    await waitFor(() => expect(mocked.dispatch).toHaveBeenCalled());
    expect(await screen.findByText("heads up")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Mission board" })).toBeInTheDocument();
  });

  it("dispatches from a card", async () => {
    mocked.view.mockResolvedValue(makeView({ cards: [makeCard()] }));
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(screen.getByRole("button", { name: "Dispatch an agent" }));
    expect(await screen.findByRole("heading", { name: "Dispatch an agent" })).toBeInTheDocument();
  });

  it("reloads when the backend reports a change", async () => {
    let notify: (() => void) | null = null;
    mocked.onChange.mockImplementation((cb: () => void) => {
      notify = cb;
      return () => {};
    });
    render(<App />);
    await screen.findByLabelText("Sector map");
    expect(mocked.view).toHaveBeenCalledTimes(1);
    mocked.view.mockResolvedValue(makeView({ herdr_version: "2.0.0" }));
    await act(async () => notify!());
    expect(await screen.findByText("Herdr 2.0.0")).toBeInTheDocument();
  });

  it("opens a station's live session from the location view", async () => {
    const s = makeSession({ link_id: "lz", station_id: "enterprise.data", prompt: "station prompt" });
    mocked.view.mockResolvedValue(makeView({ cards: [makeCard({ sessions: [s] })] }));
    render(<App />);
    fireEvent.click(await screen.findByLabelText(/Enterprise, 1 stations/));
    fireEvent.click(screen.getByText("station prompt"));
    expect(await screen.findByRole("complementary", { name: "Session" })).toBeInTheDocument();
  });
});

  it("closes the dialogs", async () => {
    mocked.view.mockResolvedValue(makeView({ cards: [makeCard()] }));
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(screen.getByRole("button", { name: "New card" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("heading", { name: "New card" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Dispatch an agent" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("heading", { name: "Dispatch an agent" })).toBeNull();
  });

  it("closes the session drawer", async () => {
    const session = makeSession({ link_id: "l1" });
    mocked.view.mockResolvedValue(makeView({ cards: [makeCard({ sessions: [session] })] }));
    const { container } = render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(container.querySelector(".session-row")!);
    const drawer = await screen.findByRole("complementary", { name: "Session" });
    fireEvent.click(within(drawer).getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("complementary", { name: "Session" })).toBeNull();
  });

  it("follows a drawer pane that was attached to a card", async () => {
    let notify: (() => void) | null = null;
    mocked.onChange.mockImplementation((cb: () => void) => {
      notify = cb;
      return () => {};
    });
    const contact = makeContact({ pane_id: "p9", title: "Loose" });
    mocked.view.mockResolvedValueOnce(makeView({ unassigned: [contact] }));
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(screen.getByText(/Loose/));
    await screen.findByRole("complementary", { name: "Session" });
    // The pane is now a session on a card, so it is no longer in unassigned.
    const moved = makeSession({ link_id: "l-moved", pane_id: "p9", station_name: "Data" });
    mocked.view.mockResolvedValue(makeView({ unassigned: [], cards: [makeCard({ sessions: [moved] })] }));
    await act(async () => notify!());
    const drawer = await screen.findByRole("complementary", { name: "Session" });
    expect(within(drawer).getByRole("heading", { name: "Data" })).toBeInTheDocument();
  });

  it("opens the card dialog for editing from a card", async () => {
    mocked.view.mockResolvedValue(makeView({ cards: [makeCard()] }));
    render(<App />);
    await screen.findByLabelText("Sector map");
    fireEvent.click(screen.getByRole("button", { name: "Mission board" }));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.getByRole("heading", { name: "Edit card" })).toBeInTheDocument();
  });
