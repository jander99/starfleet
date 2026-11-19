import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { Board } from "../components/Board";
import { backend } from "../api";
import { makeCard, makeContact, makeSession, makeView, NOW } from "./helpers";

vi.mock("../api", async (importOriginal) => {
  const actual = (await importOriginal()) as Record<string, unknown>;
  const { fakeBackend } = await import("./helpers");
  return { ...actual, backend: fakeBackend };
});

const mocked = vi.mocked(backend);

function props(over: Partial<Parameters<typeof Board>[0]> = {}): Parameters<typeof Board>[0] {
  return {
    view: makeView(),
    onSession: vi.fn(),
    onContact: vi.fn(),
    onDispatch: vi.fn(),
    onNewCard: vi.fn(),
    onEdit: vi.fn(),
    onArtifact: vi.fn(),
    onConsole: vi.fn(),
    ...over,
  };
}

beforeEach(() => {
  vi.mocked(backend).updateCard.mockClear();
  vi.mocked(backend).attach.mockClear();
});

describe("Board", () => {
  it("shows the empty state and the tray when nothing is connected", () => {
    render(<Board {...props({ view: makeView({ connected: false }) })} />);
    expect(screen.getByText(/No open cards/)).toBeInTheDocument();
    expect(screen.getByText("Herdr is not connected.")).toBeInTheDocument();
  });

  it("says when every running agent is on a card", () => {
    render(<Board {...props()} />);
    expect(screen.getByText("None. Every running agent is on a card.")).toBeInTheDocument();
  });

  it("asks for a new card and dispatches from a card", () => {
    const p = props({ view: makeView({ cards: [makeCard()] }) });
    render(<Board {...p} />);
    fireEvent.click(screen.getByRole("button", { name: "New card" }));
    expect(p.onNewCard).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Dispatch an agent" }));
    expect(p.onDispatch).toHaveBeenCalledWith("c1");
  });

  it("opens the console, edits, and toggles done on a card", async () => {
    const card = makeCard();
    const p = props({ view: makeView({ cards: [card] }) });
    render(<Board {...p} />);
    fireEvent.click(screen.getByRole("button", { name: "Card one" }));
    expect(p.onConsole).toHaveBeenCalledWith(card);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect(p.onEdit).toHaveBeenCalledWith(card);
    fireEvent.click(screen.getByRole("button", { name: "Mark done" }));
    await waitFor(() => expect(mocked.updateCard).toHaveBeenCalledWith("c1", { done: true }));
  });

  it("surfaces an update error inline", async () => {
    mocked.updateCard.mockRejectedValueOnce("card is locked");
    render(<Board {...props({ view: makeView({ cards: [makeCard()] }) })} />);
    fireEvent.click(screen.getByRole("button", { name: "Mark done" }));
    expect(await screen.findByText("card is locked")).toBeInTheDocument();
  });

  it("hides and reveals done cards", () => {
    const done = makeCard({ id: "c2", title: "Finished thing", done: true, state: "done" });
    render(<Board {...props({ view: makeView({ cards: [done] }) })} />);
    expect(screen.queryByText("Finished thing")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Show 1 done card/ }));
    expect(screen.getByText("Finished thing")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Hide 1 done card/ }));
    expect(screen.queryByText("Finished thing")).toBeNull();
  });

  it("pluralizes the done toggle", () => {
    const cards = [
      makeCard({ id: "a", done: true, state: "done" }),
      makeCard({ id: "b", done: true, state: "done" }),
    ];
    render(<Board {...props({ view: makeView({ cards }) })} />);
    expect(screen.getByRole("button", { name: /Show 2 done cards/ })).toBeInTheDocument();
  });

  it("renders a session row with its details and opens it", () => {
    const session = makeSession({ prompt: "do the thing", pending_prompt: "later", live: false, output_at: NOW });
    const p = props({ view: makeView({ cards: [makeCard({ sessions: [session] })] }) });
    const { container } = render(<Board {...p} />);
    expect(screen.getByText("do the thing")).toBeInTheDocument();
    expect(screen.getByText("Prompt held back, waiting on the agent")).toBeInTheDocument();
    expect(screen.getByText("Output saved")).toBeInTheDocument();
    fireEvent.click(container.querySelector(".session-row")!);
    expect(p.onSession).toHaveBeenCalledWith("l1");
  });

  it("falls back through station name, title, agent then Session", () => {
    const a = makeSession({ link_id: "a", station_name: null, title: "T", agent: "claude" });
    const b = makeSession({ link_id: "b", station_name: null, title: null, agent: null });
    const { container } = render(<Board {...props({ view: makeView({ cards: [makeCard({ sessions: [a, b] })] }) })} />);
    const titles = [...container.querySelectorAll(".session-title")].map((n) => n.textContent);
    expect(titles[0]).toMatch(/^T/);
    expect(titles[1]).toMatch(/^Session/);
  });

  it("renders artifacts with their status and opens one", () => {
    const card = makeCard({
      artifacts: [
        { id: "a1", kind: "pr", label: "PR #7", target: "https://x", last_status: "ci_passed", checked_at: NOW },
        { id: "a2", kind: "url", label: "Docs", target: "https://d", last_status: null, checked_at: null },
      ],
    });
    const p = props({ view: makeView({ cards: [card] }) });
    render(<Board {...p} />);
    expect(screen.getByText("PR #7")).toBeInTheDocument();
    expect(screen.getByText(/CI passed/)).toBeInTheDocument();
    expect(screen.getByText("Docs")).toBeInTheDocument();
    expect(screen.getByText("checking…")).toBeInTheDocument();
    fireEvent.click(screen.getByText("PR #7"));
    expect(p.onArtifact).toHaveBeenCalledWith("c1", "a1");
  });

  it("attaches an unassigned contact to a card", async () => {
    const view = makeView({ cards: [makeCard()], unassigned: [makeContact()] });
    const p = props({ view });
    render(<Board {...p} />);
    const tray = screen.getByRole("heading", { name: "Unassigned contacts" }).parentElement!;
    const select = within(tray).getByLabelText("Attach this session to a card");
    fireEvent.change(select, { target: { value: "c1" } });
    await waitFor(() => expect(mocked.attach).toHaveBeenCalledWith("c1", "p9"));
  });

  it("disables attaching when there are no cards", () => {
    render(<Board {...props({ view: makeView({ unassigned: [makeContact()] }) })} />);
    const select = screen.getByLabelText("Attach this session to a card");
    expect(select).toBeDisabled();
    expect(screen.getByText("Create a card to attach")).toBeInTheDocument();
  });

  it("opens an unassigned contact", () => {
    const p = props({ view: makeView({ unassigned: [makeContact()] }) });
    render(<Board {...p} />);
    fireEvent.click(screen.getByText(/Loose session/));
    expect(p.onContact).toHaveBeenCalledWith("p9");
  });

  it("orders open cards by attention", () => {
    const blocked = makeCard({ id: "b", title: "Blocked one", state: "blocked" });
    const idle = makeCard({ id: "i", title: "Idle one", state: "idle" });
    const { container } = render(<Board {...props({ view: makeView({ cards: [idle, blocked] }) })} />);
    const titles = [...container.querySelectorAll(".card-name")].map((n) => n.textContent);
    expect(titles).toEqual(["Blocked one", "Idle one"]);
  });
});

  it("shows a card's notes and falls back for an unnamed contact", () => {
    const card = makeCard({ notes: "remember this" });
    const contact = makeContact({ title: null, workspace: null, pane_id: "w1:p1", agent: null });
    render(<Board {...props({ view: makeView({ cards: [card], unassigned: [contact] }) })} />);
    expect(screen.getByText("remember this")).toBeInTheDocument();
    // No title or workspace: the pane id stands in. No agent: "agent".
    expect(screen.getByText("w1:p1")).toBeInTheDocument();
    expect(screen.getByText(/· agent/)).toBeInTheDocument();
  });
