import { describe, expect, it, vi, afterEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { CardStatePill, ErrorNote, Modal, StatusPill, timeAgo } from "../ui";
import type { CardState, Status } from "../api";

afterEach(() => vi.useRealTimers());

describe("StatusPill", () => {
  const cases: [Status, string][] = [
    ["blocked", "Needs you"],
    ["done", "Finished"],
    ["working", "Working"],
    ["idle", "Idle"],
    ["unknown", "Idle"],
    ["offline", "Ended"],
  ];

  it.each(cases)("labels %s as %s", (status, label) => {
    const { container } = render(<StatusPill status={status} />);
    expect(screen.getByText(label)).toBeInTheDocument();
    expect(container.querySelector(`.status-${status}`)).not.toBeNull();
  });
});

describe("CardStatePill", () => {
  const cases: [CardState, string][] = [
    ["blocked", "Needs you"],
    ["interrupted", "Interrupted"],
    ["done", "Finished"],
    ["paused", "Paused"],
    ["working", "Working"],
    ["idle", "Idle"],
    ["offline", "Offline"],
    ["empty", "No sessions"],
  ];

  it.each(cases)("labels %s as %s", (state, label) => {
    const { container } = render(<CardStatePill state={state} />);
    expect(screen.getByText(label)).toBeInTheDocument();
    expect(container.querySelector(`.card-state-${state}`)).not.toBeNull();
  });
});

describe("timeAgo", () => {
  it("is empty for a missing timestamp", () => {
    expect(timeAgo(null)).toBe("");
    expect(timeAgo(0)).toBe("");
  });

  it("rounds to the largest sensible unit", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_700_000_000_000);
    const now = Date.now();
    expect(timeAgo(now - 30_000)).toBe("just now");
    expect(timeAgo(now - 5 * 60_000)).toBe("5m ago");
    expect(timeAgo(now - 2 * 3600_000)).toBe("2h ago");
    expect(timeAgo(now - 3 * 86400_000)).toBe("3d ago");
  });

  it("clamps a future timestamp to just now", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_700_000_000_000);
    expect(timeAgo(Date.now() + 60_000)).toBe("just now");
  });
});

describe("Modal", () => {
  it("renders its title, children and a close button", () => {
    const onClose = vi.fn();
    render(
      <Modal title="Edit card" onClose={onClose}>
        <p>body</p>
      </Modal>,
    );
    expect(screen.getByRole("dialog", { name: "Edit card" })).toBeInTheDocument();
    expect(screen.getByText("body")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes on Escape and stops listening once unmounted", () => {
    const onClose = vi.fn();
    const { unmount } = render(
      <Modal title="T" onClose={onClose}>
        <p>x</p>
      </Modal>,
    );
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(window, { key: "Enter" });
    expect(onClose).toHaveBeenCalledTimes(1);
    unmount();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes on a scrim press but not on a press inside the dialog", () => {
    const onClose = vi.fn();
    const { container } = render(
      <Modal title="T" onClose={onClose}>
        <p>inner</p>
      </Modal>,
    );
    fireEvent.mouseDown(screen.getByText("inner"));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.mouseDown(container.querySelector(".scrim")!);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("can be wide", () => {
    const { container } = render(
      <Modal title="T" onClose={() => {}} wide>
        <p>x</p>
      </Modal>,
    );
    expect(container.querySelector(".modal.wide")).not.toBeNull();
  });
});

describe("ErrorNote", () => {
  it("renders the text, or nothing when there is none", () => {
    const { container, rerender } = render(<ErrorNote text="something broke" />);
    expect(screen.getByText("something broke")).toBeInTheDocument();
    rerender(<ErrorNote text={null} />);
    expect(container).toBeEmptyDOMElement();
  });
});
