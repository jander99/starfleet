import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CardConsole } from "../components/CardConsole";
import { backend } from "../api";
import { makeCard, makeLog } from "./helpers";

vi.mock("../api", async (importOriginal) => {
  const actual = (await importOriginal()) as Record<string, unknown>;
  const { fakeBackend } = await import("./helpers");
  return { ...actual, backend: fakeBackend };
});

const mocked = vi.mocked(backend);

beforeEach(() => {
  vi.clearAllMocks();
  mocked.readLog.mockResolvedValue([]);
});

describe("CardConsole", () => {
  it("shows the header and an empty log", async () => {
    render(<CardConsole card={makeCard({ state: "idle" })} onClose={() => {}} />);
    expect(screen.getByRole("heading", { name: "Card one" })).toBeInTheDocument();
    expect(screen.getByText(/Captain's Log · idle · \/work\/one/)).toBeInTheDocument();
    expect(screen.getByText("Loading the log…")).toBeInTheDocument();
    expect(await screen.findByText(/Nothing logged yet/)).toBeInTheDocument();
  });

  it("closes", () => {
    const onClose = vi.fn();
    render(<CardConsole card={makeCard()} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("shows a read error", async () => {
    mocked.readLog.mockRejectedValueOnce("cannot read");
    render(<CardConsole card={makeCard()} onClose={() => {}} />);
    expect(await screen.findByText("cannot read")).toBeInTheDocument();
  });

  it("renders entries with friendly kind labels", async () => {
    mocked.readLog.mockResolvedValueOnce([
      makeLog({ id: "a", kind: "prompt", owner: "captain", text: "the ask" }),
      makeLog({ id: "b", kind: "result", owner: "claude", text: "the answer" }),
      makeLog({ id: "c", kind: "weird", owner: "captain", text: "custom" }),
    ]);
    render(<CardConsole card={makeCard()} onClose={() => {}} />);
    expect(await screen.findByText("the ask")).toBeInTheDocument();
    expect(screen.getByText(/^order ·/)).toBeInTheDocument();
    expect(screen.getByText(/^result ·/)).toBeInTheDocument();
    expect(screen.getByText(/^weird ·/)).toBeInTheDocument();
    expect(screen.getByText("claude")).toBeInTheDocument();
  });

  it("collapses older rounds until asked to show them", async () => {
    mocked.readLog.mockResolvedValueOnce([
      makeLog({ id: "p1", kind: "prompt", text: "round one" }),
      makeLog({ id: "p2", kind: "prompt", text: "round two" }),
    ]);
    render(<CardConsole card={makeCard()} onClose={() => {}} />);
    expect(await screen.findByText("round two")).toBeInTheDocument();
    expect(screen.queryByText("round one")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show 1 earlier round" }));
    expect(screen.getByText("round one")).toBeInTheDocument();
  });

  it("pluralizes the hidden-rounds button", async () => {
    mocked.readLog.mockResolvedValueOnce([
      makeLog({ id: "p1", kind: "prompt" }),
      makeLog({ id: "p2", kind: "prompt" }),
      makeLog({ id: "p3", kind: "prompt" }),
    ]);
    render(<CardConsole card={makeCard()} onClose={() => {}} />);
    expect(await screen.findByRole("button", { name: "Show 2 earlier rounds" })).toBeInTheDocument();
  });

  it("appends a note and reloads", async () => {
    render(<CardConsole card={makeCard()} onClose={() => {}} />);
    await screen.findByText(/Nothing logged yet/);
    const box = screen.getByPlaceholderText("Add a note or decision to the log…");
    fireEvent.change(box, { target: { value: "a decision" } });
    fireEvent.click(screen.getByRole("button", { name: "Log it" }));
    await waitFor(() => expect(mocked.appendLog).toHaveBeenCalledWith("c1", "a decision"));
    expect(box).toHaveValue("");
    expect(mocked.readLog).toHaveBeenCalledTimes(2);
  });

  it("submits with Cmd+Enter", async () => {
    render(<CardConsole card={makeCard()} onClose={() => {}} />);
    await screen.findByText(/Nothing logged yet/);
    const box = screen.getByPlaceholderText("Add a note or decision to the log…");
    fireEvent.change(box, { target: { value: "quick note" } });
    fireEvent.keyDown(box, { key: "Enter", metaKey: true });
    await waitFor(() => expect(mocked.appendLog).toHaveBeenCalledWith("c1", "quick note"));
  });

  it("does not submit empty text or a plain Enter", async () => {
    render(<CardConsole card={makeCard()} onClose={() => {}} />);
    await screen.findByText(/Nothing logged yet/);
    const box = screen.getByPlaceholderText("Add a note or decision to the log…");
    fireEvent.keyDown(box, { key: "Enter", metaKey: true });
    fireEvent.change(box, { target: { value: "x" } });
    fireEvent.keyDown(box, { key: "Enter" });
    expect(mocked.appendLog).not.toHaveBeenCalled();
  });

  it("disables the button for blank text", async () => {
    render(<CardConsole card={makeCard()} onClose={() => {}} />);
    await screen.findByText(/Nothing logged yet/);
    expect(screen.getByRole("button", { name: "Log it" })).toBeDisabled();
    fireEvent.change(screen.getByPlaceholderText("Add a note or decision to the log…"), { target: { value: "  " } });
    expect(screen.getByRole("button", { name: "Log it" })).toBeDisabled();
  });

  it("shows an append error", async () => {
    mocked.appendLog.mockRejectedValueOnce("disk full");
    render(<CardConsole card={makeCard()} onClose={() => {}} />);
    await screen.findByText(/Nothing logged yet/);
    fireEvent.change(screen.getByPlaceholderText("Add a note or decision to the log…"), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: "Log it" }));
    expect(await screen.findByText("disk full")).toBeInTheDocument();
  });
});
