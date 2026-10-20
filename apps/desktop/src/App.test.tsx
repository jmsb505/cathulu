import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe("read-only project dashboard", () => {
  beforeEach(() => invokeMock.mockReset());

  it("renders the snapshot returned by the Tauri command", async () => {
    invokeMock.mockResolvedValueOnce([
      {
        id: 1,
        name: "Atlas",
        path: "C:\\work\\atlas",
        branch: "main",
        head: "0123456789abcdef",
        dirty: true,
        changedPaths: ["work.txt"],
        registeredAt: 1_800_000_000,
        reconciledAt: 1_800_000_100,
      },
    ] as never);

    render(<App />);

    expect(await screen.findByRole("heading", { name: "Atlas" })).toBeTruthy();
    expect(screen.getByText("C:\\work\\atlas")).toBeTruthy();
    expect(screen.getByText("main")).toBeTruthy();
    expect(screen.getByText("work.txt")).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("project_snapshot");
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("shows the empty state when no projects are registered", async () => {
    invokeMock.mockResolvedValueOnce([] as never);

    render(<App />);

    expect(await screen.findByText("A quiet starting point.")).toBeTruthy();
    expect(screen.getByText("Register a repository through Codex to see its Git snapshot here.")).toBeTruthy();
  });

  it("shows command failures as a visible local-state error", async () => {
    invokeMock.mockRejectedValueOnce(new Error("database unavailable"));

    render(<App />);

    expect((await screen.findByRole("alert")).textContent).toContain("database unavailable");
  });
});
