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
      delegations: [{
        id: 12,
        projectName: "Atlas",
        taskTitle: "Implement worker reports",
        runId: 7,
        role: "worker",
        status: "dispatched",
        parentRunStatus: "blocked",
        createdAt: 1_800_000_120,
        updatedAt: 1_800_000_150,
        report: {
          reportId: 13,
          delegationId: 12,
          reviewStatus: "reported",
          report: {
            schemaVersion: 1,
            summary: "The worker completed the scoped changes and reports passing tests.",
            outcome: "completed",
            changedFiles: ["src/delegation.rs"],
            commits: ["0123456789abcdef0123456789abcdef01234567"],
            checkResults: [{ name: "workspace tests", status: "passed", summary: "All reported tests passed." }],
            blockers: [],
            artifactIds: [11],
            qualityCheckIds: [5],
          },
          reportHash: "a".repeat(64),
          reportedAt: 1_800_000_150,
          qualityCheckRefs: [{ qualityCheckId: 5, checkKey: "workspace-tests", status: "passed" }],
          reviewHistory: [],
        },
        worktree: {
          worktreePathHash: "f".repeat(64),
          branch: "codex/worker-fixture",
          baseCommit: "abcdef0123456789abcdef0123456789abcdef01",
          observedHead: "0123456789abcdef0123456789abcdef01234567",
          isClean: true,
          changedPaths: ["src/delegation.rs"],
          reportVerification: {
            status: "verified",
            verifiedCommits: ["0123456789abcdef0123456789abcdef01234567"],
            unverifiedCommits: [],
            verifiedPaths: ["src/delegation.rs"],
            unverifiedPaths: [],
          },
          lastReconciledAt: 1_800_000_151,
        },
        workerTelemetry: {
          hookEventId: 17,
          agentType: "worker",
          effectiveModel: "gpt-5-codex",
          effectiveEffort: null,
          linkedAt: 1_800_000_149,
          events: [
            { eventId: 17, eventKind: "started", agentType: "worker", model: "gpt-5-codex", createdAt: 1_800_000_149 },
            { eventId: 18, eventKind: "stopped", agentType: "worker", model: "gpt-5-codex", createdAt: 1_800_000_150 },
          ],
        },
      }, {
        id: 14,
        projectName: "Atlas",
        taskTitle: "Resolve schema mismatch",
        runId: 8,
        role: "worker",
        status: "dispatched",
        parentRunStatus: "blocked",
        createdAt: 1_800_000_121,
        updatedAt: 1_800_000_151,
        report: {
          reportId: 15,
          delegationId: 14,
          reviewStatus: "needs_follow_up",
          report: {
            schemaVersion: 1,
            summary: "The migration stopped after detecting an incompatible schema.",
            outcome: "blocked",
            changedFiles: [],
            commits: [],
            checkResults: [{ name: "migration check", status: "failed", summary: "The existing schema does not match the expected version." }],
            blockers: ["Resolve the schema mismatch before retrying."],
            artifactIds: [],
            qualityCheckIds: [],
          },
          reportHash: "b".repeat(64),
          reportedAt: 1_800_000_151,
          qualityCheckRefs: [],
          reviewHistory: [{ id: 16, decision: "needs_follow_up", note: "Please add the migration diagnosis.", createdAt: 1_800_000_152 }],
        },
      }],
      failedChecks: [
        { id: 5, runId: 6, projectName: "Atlas", checkKey: "integration", mandatory: true, status: "failed", details: {}, updatedAt: 1_800_000_115 },
        { id: 9, runId: 8, projectName: "Atlas", checkKey: "packaging", mandatory: false, status: "inconclusive", details: {}, updatedAt: 1_800_000_116 },
      ],
      memoryHealth: { hotBytes: 512, warmBytes: 2048, coldBytes: 4096, hotScopeCount: 2, largestScopeHotBytes: 256, canonicalCount: 3, episodicCount: 4, evidenceCount: 5, candidateCount: 1, invalidatedCount: 2, protectedArtifactCount: 1, duplicateMemoryGroups: 0, staleMemoryCount: 2, reusedMemoryCount: 8, contextTransferBytes: 1000, contextReintroducedBytes: 128, contextReintroductionBasisPoints: 1280, contextAmplificationBasisPoints: 15000, retrievalUsefulnessBasisPoints: 7500, pendingMaintenanceProposals: 2, sqliteBytes: 8192, sqliteGrowthBytes: 2048, orphanedEvidenceCount: 1, duplicateArtifactGroups: 0, bytesCollected: 0 },
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
    expect(screen.getByRole("heading", { name: "Delegations & worker reports" })).toBeTruthy();
    expect(screen.getByText("Implement worker reports")).toBeTruthy();
    expect(screen.getByText("Reported · awaiting review")).toBeTruthy();
    expect(screen.getByText("Worker-reported outcome: completed")).toBeTruthy();
    expect(screen.getByText("Report Git verification")).toBeTruthy();
    expect(screen.getByText("verified")).toBeTruthy();
    expect(screen.getByText(/Observed worker/)).toBeTruthy();
    expect(screen.getByText(/gpt-5-codex · effort unavailable/)).toBeTruthy();
    expect(screen.getAllByText("Parent run blocked").length).toBe(2);
    expect(screen.getByText("needs follow up")).toBeTruthy();
    expect(screen.getByText("Resolve the schema mismatch before retrying.")).toBeTruthy();
    expect(screen.getByText(/Parent-run verification still depends on its own mandatory checks\./)).toBeTruthy();
    expect(screen.getByText("HOT scopes")).toBeTruthy();
    expect(screen.getByText("Largest scope HOT")).toBeTruthy();
    expect(screen.getByText("256 B")).toBeTruthy();
    expect(screen.getByText("2.0 KiB")).toBeTruthy();
    expect(screen.getByText("Invalidated")).toBeTruthy();
    expect(screen.getByText("Protected artifacts")).toBeTruthy();
    expect(screen.getByText(/12\.8%/)).toBeTruthy();
    expect(screen.getByText("150.0%")).toBeTruthy();
    expect(screen.getByText("75.0%")).toBeTruthy();
    expect(screen.getByText("2.0 KiB")).toBeTruthy();
    expect(screen.getByText("Resolve CI gate")).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("dashboard_snapshot");
    expect(screen.getByRole("link", { name: "Cathulu home" })).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("shows the empty state when no projects are registered", async () => {
    invokeMock.mockResolvedValueOnce({
      projects: [], objectives: [], blockers: [], recentRuns: [], delegations: [], failedChecks: [],
      memoryHealth: { hotBytes: 0, warmBytes: 0, coldBytes: 0, hotScopeCount: 0, largestScopeHotBytes: 0, canonicalCount: 0, episodicCount: 0, evidenceCount: 0, candidateCount: 0, invalidatedCount: 0, protectedArtifactCount: 0, duplicateMemoryGroups: 0, staleMemoryCount: 0, reusedMemoryCount: 0, contextTransferBytes: 0, contextReintroducedBytes: 0, contextReintroductionBasisPoints: 0, contextAmplificationBasisPoints: 0, retrievalUsefulnessBasisPoints: 0, pendingMaintenanceProposals: 0, sqliteBytes: 0, sqliteGrowthBytes: 0, orphanedEvidenceCount: 0, duplicateArtifactGroups: 0, bytesCollected: 0 },
    } as never);

    render(<App />);

    expect(await screen.findByText("A quiet starting point.")).toBeTruthy();
    expect(screen.getByText("Register a repository through Codex to see its Git snapshot here.")).toBeTruthy();
    expect(screen.getByText(/No worker handoffs yet\./)).toBeTruthy();
  });

  it("shows command failures as a visible local-state error", async () => {
    invokeMock.mockRejectedValueOnce(new Error("database unavailable"));

    render(<App />);

    expect((await screen.findByRole("alert")).textContent).toContain("database unavailable");
  });
});
