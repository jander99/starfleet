import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => cleanup());

// jsdom has no canvas; stub it so components that draw do not log
// "not implemented" on every render. Tests that care spy on it themselves.
HTMLCanvasElement.prototype.getContext = vi.fn(() => null) as never;
