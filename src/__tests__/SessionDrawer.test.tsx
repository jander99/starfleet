import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SessionDrawer, type DrawerTarget } from "../components/SessionDrawer";
import { backend } from "../api";
import { NOW } from "./helpers";

vi.mock("../api", async (importOriginal) => {
  const actual = (await importOriginal()) as Record<string, unknown>;
  const { fakeBackend } = await import("./helpers");
  return { ...actual, backend: fakeBackend };
});

const mocked = vi.mocked(backend);

function target(over: Partial<DrawerTarget> = {}): DrawerTarget {
  return {
    heading: "Data",
    status: "working",
    paneId: "p1",
    linkId: "l1",
    agent: "claude",
    cardTitle: "Card one",
    prompt: null,
    pendingPrompt: null,
    cwd: "/w",
    lastSeen: null,
    outputAt: null,
    ...over,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocked.tail.mockResolvedValue("recent output");
  mocked.savedOutput.mockResolvedValue("saved output");
});

describe("SessionDrawer", () => {
  it("shows the live output and header details", async () => {
    render(<SessionDrawer target={target()} onClose={() => {}} />);
    expect(await screen.findByText("recent output")).toBeInTheDocument();
    expect(screen.getByText("Card one · claude · p1")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Data" })).toBeInTheDocument();
    expect(mocked.tail).toHaveBeenCalledWith("p1", 200);
  });

  it("polls while the agent is working and stops when idle", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { rerender } = render(<SessionDrawer target={target({ status: "working" })} onClose={() => {}} />);
      await waitFor(() => expect(mocked.tail).toHaveBeenCalledTimes(1));
      await vi.advanceTimersByTimeAsync(2000);
      expect(mocked.tail).toHaveBeenCalledTimes(2);

      rerender(<SessionDrawer target={target({ status: "idle" })} onClose={() => {}} />);
      await vi.advanceTimersByTimeAsync(4000);
      // One more read on the status change, then no further polling.
      expect(mocked.tail).toHaveBeenCalledTimes(3);
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows a tail error", async () => {
    mocked.tail.mockRejectedValueOnce("pane gone");
    render(<SessionDrawer target={target()} onClose={() => {}} />);
    expect(await screen.findByText("pane gone")).toBeInTheDocument();
  });

  it("shows saved output once the pane is gone", async () => {
    render(<SessionDrawer target={target({ paneId: null, outputAt: NOW })} onClose={() => {}} />);
    expect(await screen.findByText("saved output")).toBeInTheDocument();
    expect(screen.getByText(/Saved when the agent last stopped/)).toBeInTheDocument();
    expect(mocked.savedOutput).toHaveBeenCalledWith("l1");
  });

  it("says so when the session is gone with nothing saved", () => {
    render(<SessionDrawer target={target({ paneId: null, outputAt: null })} onClose={() => {}} />);
    expect(screen.getByText(/no output was saved/)).toBeInTheDocument();
  });

  it("closes", () => {
    const onClose = vi.fn();
    render(<SessionDrawer target={target()} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("shows the original prompt", () => {
    render(<SessionDrawer target={target({ prompt: "the original ask" })} onClose={() => {}} />);
    expect(screen.getByText("Original prompt")).toBeInTheDocument();
    expect(screen.getByText("the original ask")).toBeInTheDocument();
  });

  it("focuses the pane and reports it", async () => {
    render(<SessionDrawer target={target()} onClose={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Jump to pane" }));
    await waitFor(() => expect(mocked.focus).toHaveBeenCalledWith("p1"));
    expect(await screen.findByText("Brought to the front in Herdr.")).toBeInTheDocument();
  });

  it("detaches from a card and closes", async () => {
    const onClose = vi.fn();
    render(<SessionDrawer target={target()} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "Detach from card" }));
    await waitFor(() => expect(mocked.detach).toHaveBeenCalledWith("l1"));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("labels detach differently for a dead pane", () => {
    render(<SessionDrawer target={target({ paneId: null, outputAt: NOW })} onClose={() => {}} />);
    expect(screen.getByRole("button", { name: "Remove from card" })).toBeInTheDocument();
  });

  it("sends a held-back prompt", async () => {
    render(<SessionDrawer target={target({ pendingPrompt: "held", status: "idle" })} onClose={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Send the prompt now" }));
    await waitFor(() => expect(mocked.sendPending).toHaveBeenCalledWith("l1"));
    expect(await screen.findByText("Prompt sent.")).toBeInTheDocument();
  });

  it("disables sending a held prompt while blocked", () => {
    render(<SessionDrawer target={target({ pendingPrompt: "held", status: "blocked" })} onClose={() => {}} />);
    expect(screen.getByRole("button", { name: "Send the prompt now" })).toBeDisabled();
  });

  it("offers reply keys only while blocked", async () => {
    const { rerender } = render(<SessionDrawer target={target({ status: "blocked" })} onClose={() => {}} />);
    expect(screen.getByText("Answer the agent:")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Enter" }));
    await waitFor(() => expect(mocked.replyKeys).toHaveBeenCalledWith("p1", null, ["enter"]));

    rerender(<SessionDrawer target={target({ status: "idle" })} onClose={() => {}} />);
    expect(screen.queryByText("Answer the agent:")).toBeNull();
  });

  it("sends a follow-up message", async () => {
    render(<SessionDrawer target={target({ status: "idle" })} onClose={() => {}} />);
    const box = screen.getByPlaceholderText("Send a follow-up message");
    fireEvent.change(box, { target: { value: "keep going" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(mocked.followUp).toHaveBeenCalledWith("p1", "keep going"));
    expect(box).toHaveValue("");
  });

  it("blocks the follow-up form while the agent is blocked", () => {
    render(<SessionDrawer target={target({ status: "blocked" })} onClose={() => {}} />);
    expect(screen.getByPlaceholderText("Answer the agent's question first")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
  });
});

  it("reports a saved-output error once the pane is gone", async () => {
    mocked.savedOutput.mockRejectedValueOnce(new Error("log unreadable"));
    render(<SessionDrawer target={target({ paneId: null, outputAt: NOW })} onClose={() => {}} />);
    expect(await screen.findByText("log unreadable")).toBeInTheDocument();
  });

  it("reports a failed action", async () => {
    mocked.focus.mockRejectedValueOnce(new Error("no such pane"));
    render(<SessionDrawer target={target()} onClose={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Jump to pane" }));
    expect(await screen.findByText("no such pane")).toBeInTheDocument();
  });
