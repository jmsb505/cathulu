import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe("read-only project dashboard", () => {
  beforeEach(() => invokeMock.mockReset());

  it("renders the snapshot returned by the Tauri command", async () => {
    invokeMock.mockResolvedValueOnce({
      projects: [{
        id: 1,
        name: "Atlas",
        path: "C:\\work\\atlas",
        branch: "main",
        head: "0123456789abcdef",
        dirty: true,
        changedPaths: ["work.txt"],
        registeredAt: 1_800_000_000,
        reconciledAt: 1_800_000_100,
      }],
      objectives: [
        { id: 2, projectId: 1, projectName: "Atlas", title: "Ship memory foundations", status: "active", updatedAt: 1_800_000_120 },
        { id: 3, projectId: 1, projectName: "Atlas", title: "Resolve release blocker", status: "blocked", updatedAt: 1_800_000_130 },
      ],
      blockers: [{ id: 3, objectiveId: 2, projectName: "Atlas", title: "Resolve CI gate", updatedAt: 1_800_000_130 }],
      recentRuns: [
        { id: 4, projectName: "Atlas", objectiveTitle: "Ship memory foundations", taskTitle: "Verify", workflowKey: "windows_release", workflowVersion: 1, status: "verified_success", summary: "All checks passed.", startedAt: 1_800_000_110, finishedAt: 1_800_000_140 },
        { id: 7, projectName: "Atlas", objectiveTitle: null, taskTitle: "Complete required review", workflowKey: "windows_release", workflowVersion: 1, status: "blocked", summary: "A required gate is missing.", startedAt: 1_800_000_109, finishedAt: 1_800_000_141 },
        { id: 8, projectName: "Atlas", objectiveTitle: null, taskTitle: "Run migration", workflowKey: "migration", workflowVersion: 1, status: "failed", summary: "A required check failed.", startedAt: 1_800_000_108, finishedAt: 1_800_000_142 },
        { id: 9, projectName: "Atlas", objectiveTitle: null, taskTitle: null, workflowKey: null, workflowVersion: null, status: "inconclusive", summary: "Codex turn ended; Cathulu verification was not recorded.", startedAt: 1_800_000_107, finishedAt: 1_800_000_143 },
      ],
      failedChecks: [
        { id: 5, runId: 6, projectName: "Atlas", checkKey: "integration", mandatory: true, status: "failed", details: {}, updatedAt: 1_800_000_115 },
        { id: 9, runId: 8, projectName: "Atlas", checkKey: "packaging", mandatory: false, status: "inconclusive", details: {}, updatedAt: 1_800_000_116 },
      ],
      memoryHealth: { hotBytes: 512, warmBytes: 2048, coldBytes: 4096, canonicalCount: 3, episodicCount: 4, evidenceCount: 5, candidateCount: 1, invalidatedCount: 2, protectedArtifactCount: 1 },
    } as never);

    render(<App />);

    expect(await screen.findByRole("heading", { name: "Atlas" })).toBeTruthy();
    expect(screen.getByText("C:\\work\\atlas")).toBeTruthy();
    expect(screen.getByText("main")).toBeTruthy();
    expect(screen.getByText("work.txt")).toBeTruthy();
    expect(screen.getByText("Ship memory foundations")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Recent runs" })).toBeTruthy();
    expect(screen.getAllByText("windows release").length).toBe(2);
    expect(screen.getAllByText("blocked").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("failed").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText(/Codex turn ended; Cathulu verification was not recorded\./)).toBeTruthy();
    expect(screen.getAllByText("inconclusive").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByRole("heading", { name: "Quality watch" })).toBeTruthy();
    expect(screen.getByText("integration")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Memory health" })).toBeTruthy();
    expect(screen.getByText("2.0 KiB")).toBeTruthy();
    expect(screen.getByText("Invalidated")).toBeTruthy();
    expect(screen.getByText("Protected artifacts")).toBeTruthy();
    expect(screen.getByText("Resolve CI gate")).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("dashboard_snapshot");
    expect(screen.getByRole("link", { name: "Cathulu home" })).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("shows the empty state when no projects are registered", async () => {
    invokeMock.mockResolvedValueOnce({
      projects: [], objectives: [], blockers: [], recentRuns: [], failedChecks: [],
      memoryHealth: { hotBytes: 0, warmBytes: 0, coldBytes: 0, canonicalCount: 0, episodicCount: 0, evidenceCount: 0, candidateCount: 0, invalidatedCount: 0, protectedArtifactCount: 0 },
    } as never);

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
