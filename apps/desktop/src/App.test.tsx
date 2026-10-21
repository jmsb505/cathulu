import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: () => true }));

const invokeMock = vi.mocked(invoke);

function snapshot(overrides: Record<string, unknown> = {}) {
  return {
    projects: [
      { id: 1, name: "PULSO", path: "C:\\work\\pulso", branch: "main", head: "a".repeat(40), dirty: false, changedPaths: [], registeredAt: 1, reconciledAt: 1 },
      { id: 2, name: "Mecanografica proposals", path: "C:\\work\\mecanografica", branch: "main", head: "b".repeat(40), dirty: false, changedPaths: [], registeredAt: 1, reconciledAt: 1 },
    ],
    objectives: [],
    blockers: [],
    recentRuns: [
      { id: 10, projectId: 1, projectName: "PULSO", objectiveTitle: "M18", taskTitle: "Run isolated QA", workflowKey: null, workflowVersion: null, status: "running", summary: "", startedAt: 100, finishedAt: null },
      { id: 20, projectId: 2, projectName: "Mecanografica proposals", objectiveTitle: null, taskTitle: "Review proposal", workflowKey: null, workflowVersion: null, status: "verified_success", summary: "", startedAt: 90, finishedAt: 110 },
    ],
    agentActivity: [],
    delegations: [
      {
        id: 100, projectId: 1, projectName: "PULSO", taskTitle: "Coordinate PULSO", runId: 10, role: "sub_overseer", status: "dispatched", parentDelegationId: null, projectManagerAssignmentStatus: "active", activationStatus: "ready", parentRunStatus: "running", createdAt: 100, updatedAt: 100, report: null, worktree: null,
        workerTelemetry: { hookEventId: 1, agentType: "sub_overseer", effectiveModel: null, effectiveEffort: null, linkedAt: 101, events: [{ eventId: 1, eventKind: "started", agentType: "sub_overseer", model: null, createdAt: 101 }] },
      },
      {
        id: 101, projectId: 1, projectName: "PULSO", taskTitle: "Run QA", runId: 10, role: "worker", status: "dispatched", parentDelegationId: 100, projectManagerAssignmentStatus: null, activationStatus: "ready", parentRunStatus: "running", createdAt: 102, updatedAt: 102, report: null, worktree: null, workerTelemetry: null,
      },
      {
        id: 200, projectId: 2, projectName: "Mecanografica proposals", taskTitle: "Coordinate proposals", runId: 20, role: "sub_overseer", status: "dispatched", parentDelegationId: null, projectManagerAssignmentStatus: "closed", activationStatus: "ready", parentRunStatus: "verified_success", createdAt: 90, updatedAt: 110, report: null, worktree: null, workerTelemetry: null,
      },
    ],
    failedChecks: [],
    memoryHealth: { hotBytes: 0, warmBytes: 0, coldBytes: 0, canonicalCount: 0, episodicCount: 0, evidenceCount: 0, sqliteBytes: 0 },
    ...overrides,
  };
}

describe("project activity graph", () => {
  beforeEach(() => { cleanup(); invokeMock.mockReset(); });

  it("defaults to one portfolio-wide Main root and labels project selection as a focus filter", async () => {
    invokeMock.mockResolvedValueOnce(snapshot() as never);
    render(<App />);

    const allProjects = await screen.findByRole("button", { name: "All projects" });
    expect(allProjects.getAttribute("aria-pressed")).toBe("true");
    expect(screen.getAllByRole("group", { name: "Main Overseer, global coordinator" })).toHaveLength(1);
    expect(screen.getByRole("group", { name: /PULSO: Working/ })).toBeTruthy();
    expect(screen.getByRole("group", { name: /Mecanografica proposals: Stopped/ })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Filter activity by project" })).toBeTruthy();
  });

  it("shows only recorded Main → Project Manager → parent-linked Worker relationships", async () => {
    invokeMock.mockResolvedValueOnce(snapshot() as never);
    render(<App />);

    expect(await screen.findByRole("button", { name: /Project Manager, Observed active: Coordinate PULSO/ })).toBeTruthy();
    const main = screen.getByRole("group", { name: "Main Overseer, global coordinator" });
    expect(main.className).toContain("state-coordinator");
    expect(main.className).not.toContain("state-active");
    expect(screen.queryByText("Run 10")).toBeNull();
    expect(screen.getByRole("button", { name: /Worker, Unobserved: Run QA/ })).toBeTruthy();
    expect(screen.getByLabelText("1 agents observed active")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Mecanografica proposals/ })).toBeTruthy();
  });

  it("keeps a dispatched agent unobserved until a linked start hook is recorded", async () => {
    const initial = snapshot();
    const delegations = initial.delegations.map((item) => item.id === 100 ? { ...item, workerTelemetry: null } : item);
    invokeMock.mockResolvedValueOnce(snapshot({ delegations }) as never);
    render(<App />);

    expect(await screen.findByRole("button", { name: /Project Manager, Unobserved: Coordinate PULSO/ })).toBeTruthy();
    expect(screen.getByLabelText("0 agents observed active")).toBeTruthy();
  });

  it("does not attach an unlinked hook observation to the hierarchy or count it as active", async () => {
    const initial = snapshot();
    const recentRuns = initial.recentRuns.filter((run) => run.projectId === 1);
    invokeMock.mockResolvedValueOnce(snapshot({
      recentRuns,
      delegations: [],
      agentActivity: [{ id: 50, runId: 10, agentType: "worker", model: "local-model", startedAt: 101, stoppedAt: null, lastEventKind: "started", lastEventAt: 101, eventCount: 1 }],
    }) as never);
    render(<App />);

    expect(await screen.findByLabelText("worker, Unobserved")).toBeTruthy();
    expect(screen.getByLabelText("0 agents observed active")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Project Manager,/ })).toBeNull();
  });

  it("shows a linked stop as finished and keeps project histories isolated", async () => {
    const initial = snapshot();
    const delegations = initial.delegations.map((item) => item.id === 101 ? {
      ...item,
      workerTelemetry: { hookEventId: 7, agentType: "worker", effectiveModel: null, effectiveEffort: null, linkedAt: 103, events: [
        { eventId: 7, eventKind: "started", agentType: "worker", model: null, createdAt: 103 },
        { eventId: 8, eventKind: "stopped", agentType: "worker", model: null, createdAt: 104 },
      ] },
    } : item);
    invokeMock.mockResolvedValueOnce(snapshot({ delegations }) as never);
    render(<App />);

    expect(await screen.findByRole("button", { name: /Worker, Stopped: Run QA/ })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Mecanografica proposals/ }));
    await waitFor(() => expect(screen.getByRole("button", { name: /Project Manager, Unobserved: Coordinate proposals/ })).toBeTruthy());
    expect(screen.getByRole("group", { name: "Main Overseer, global coordinator" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Run QA/ })).toBeNull();
  });
});
