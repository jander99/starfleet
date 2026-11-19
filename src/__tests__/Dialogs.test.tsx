import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CardDialog, DispatchDialog } from "../components/Dialogs";
import { backend } from "../api";
import { makeCard, makeRoster } from "./helpers";

vi.mock("../api", async (importOriginal) => {
  const actual = (await importOriginal()) as Record<string, unknown>;
  const { fakeBackend } = await import("./helpers");
  return { ...actual, backend: fakeBackend };
});

const mocked = vi.mocked(backend);

beforeEach(() => {
  vi.clearAllMocks();
});

describe("CardDialog", () => {
  it("creates a card from the form", async () => {
    const onClose = vi.fn();
    render(<CardDialog card={null} onClose={onClose} />);
    expect(screen.getByRole("heading", { name: "New card" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Title"), { target: { value: "New work" } });
    fireEvent.change(screen.getByLabelText(/Working folder/), { target: { value: "/w" } });
    fireEvent.change(screen.getByLabelText("Notes"), { target: { value: "some notes" } });
    fireEvent.click(screen.getByRole("button", { name: "Create card" }));
    await waitFor(() => expect(mocked.createCard).toHaveBeenCalledWith("New work", "/w", "some notes"));
    expect(onClose).toHaveBeenCalled();
  });

  it("edits an existing card", async () => {
    const card = makeCard({ title: "Old", cwd: "/old", notes: "n" });
    render(<CardDialog card={card} onClose={() => {}} />);
    expect(screen.getByRole("heading", { name: "Edit card" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Title"), { target: { value: "New" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(mocked.updateCard).toHaveBeenCalledWith("c1", { title: "New", cwd: "/old", notes: "n" }),
    );
  });

  it("shows a save error and stays open", async () => {
    mocked.createCard.mockRejectedValueOnce("folder not found");
    const onClose = vi.fn();
    render(<CardDialog card={null} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "Create card" }));
    expect(await screen.findByText("folder not found")).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("asks before deleting, then deletes", async () => {
    const onClose = vi.fn();
    render(<CardDialog card={makeCard()} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "Delete…" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete card, keep sessions running" }));
    await waitFor(() => expect(mocked.deleteCard).toHaveBeenCalledWith("c1"));
    expect(onClose).toHaveBeenCalled();
  });

  it("reports a delete error", async () => {
    mocked.deleteCard.mockRejectedValueOnce(new Error("nope"));
    render(<CardDialog card={makeCard()} onClose={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Delete…" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete card, keep sessions running" }));
    expect(await screen.findByText("nope")).toBeInTheDocument();
  });

  it("cancels", () => {
    const onClose = vi.fn();
    render(<CardDialog card={null} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalled();
  });
});

describe("DispatchDialog", () => {
  it("dispatches to the chosen station and card", async () => {
    const onDispatched = vi.fn();
    const card = makeCard();
    render(
      <DispatchDialog
        roster={makeRoster()}
        cards={[card]}
        stationId={null}
        cardId={null}
        onClose={() => {}}
        onDispatched={onDispatched}
      />,
    );
    fireEvent.change(screen.getByLabelText("Prompt"), { target: { value: "do it" } });
    fireEvent.click(screen.getByRole("button", { name: "Dispatch" }));
    await waitFor(() =>
      expect(mocked.dispatch).toHaveBeenCalledWith({
        card_id: "c1",
        station_id: "enterprise.data",
        prompt: "do it",
        agent: null,
        model: null,
      }),
    );
    expect(onDispatched).toHaveBeenCalledWith({ link_id: "l-new", pane_id: "p-new", warning: null });
  });

  it("requires a prompt", async () => {
    render(
      <DispatchDialog roster={makeRoster()} cards={[]} stationId={null} cardId={null} onClose={() => {}} onDispatched={() => {}} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Dispatch" }));
    expect(await screen.findByText("write a prompt for the agent first")).toBeInTheDocument();
    expect(mocked.dispatch).not.toHaveBeenCalled();
  });

  it("creates a card first when dispatching to a new one, then reuses it on retry", async () => {
    mocked.createCard.mockResolvedValueOnce("c-fresh");
    mocked.dispatch.mockRejectedValueOnce("herdr down");
    render(
      <DispatchDialog roster={makeRoster()} cards={[]} stationId={null} cardId={null} onClose={() => {}} onDispatched={() => {}} />,
    );
    // With no cards, the card select defaults to the new-card option.
    fireEvent.change(screen.getByLabelText("Card title"), { target: { value: "Fresh" } });
    fireEvent.change(screen.getByLabelText("Working folder"), { target: { value: "/f" } });
    fireEvent.change(screen.getByLabelText("Prompt"), { target: { value: "go" } });
    fireEvent.click(screen.getByRole("button", { name: "Dispatch" }));
    expect(await screen.findByText("herdr down")).toBeInTheDocument();
    expect(mocked.createCard).toHaveBeenCalledWith("Fresh", "/f", "");

    // Retry: the card now exists, so it is not created again.
    fireEvent.click(screen.getByRole("button", { name: "Dispatch" }));
    await waitFor(() => expect(mocked.dispatch).toHaveBeenCalledTimes(2));
    expect(mocked.createCard).toHaveBeenCalledTimes(1);
  });

  it("shows the station brief and default options", () => {
    render(
      <DispatchDialog roster={makeRoster()} cards={[]} stationId={null} cardId={null} onClose={() => {}} onDispatched={() => {}} />,
    );
    expect(screen.getByText(/Standing brief for Data/)).toBeInTheDocument();
    expect(screen.getByText("No standing brief; only your prompt is sent.")).toBeInTheDocument();
    expect(screen.getByText("Station default (claude)")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Agent's own default")).toBeInTheDocument();
  });

  it("passes a chosen agent and model through", async () => {
    render(
      <DispatchDialog roster={makeRoster()} cards={[]} stationId={null} cardId={null} onClose={() => {}} onDispatched={() => {}} />,
    );
    fireEvent.change(screen.getByLabelText("Agent"), { target: { value: "codex" } });
    fireEvent.change(screen.getByLabelText("Model"), { target: { value: "o3" } });
    fireEvent.change(screen.getByLabelText("Prompt"), { target: { value: "p" } });
    fireEvent.click(screen.getByRole("button", { name: "Dispatch" }));
    await waitFor(() =>
      expect(mocked.dispatch).toHaveBeenCalledWith(expect.objectContaining({ agent: "codex", model: "o3" })),
    );
  });
});

  it("lists stations grouped by location and tracks the selects", () => {
    render(
      <DispatchDialog roster={makeRoster()} cards={[makeCard({ title: "Card A" })]} stationId={null} cardId={null} onClose={() => {}} onDispatched={() => {}} />,
    );
    const station = screen.getByLabelText("Station") as HTMLSelectElement;
    // Options are grouped under each location.
    expect(station.querySelectorAll("optgroup").length).toBeGreaterThan(0);
    // Changing a select updates what is shown.
    fireEvent.change(station, { target: { value: station.options[0].value } });
    expect(station.value).toBe(station.options[0].value);
    const card = screen.getByLabelText("Card") as HTMLSelectElement;
    fireEvent.change(card, { target: { value: "c1" } });
    expect(card.value).toBe("c1");
  });
