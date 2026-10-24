import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: () => true }));

const invokeMock = vi.mocked(invoke);
const mainRole = {
  roleKey: "main-overseer",
  roleKind: "main_overseer",
  projectId: null,
  projectName: null,
  memoryScopeKey: "global",
  receiverScopeKey: "scope:main-overseer",
  toolProfileKey: "main-overseer-v1",
  createdAt: 1_800_000_000,
};
const emptyDashboardSnapshot = () => ({
  logicalRoles: [mainRole],
  projects: [],
  objectives: [],
  blockers: [],
  recentRuns: [],
  agentActivity: [],
  agentUpdates: [],
  pendingAgentActions: [] as unknown[],
  pendingOwnerApprovalRequests: [] as unknown[],
  recentAgentUpdateCount: 0,
  delegations: [],
  roleDispatches: [],
  roleInbox: [],
  failedChecks: [],
  memoryHealth: {
    hotBytes: 0, warmBytes: 0, coldBytes: 0, hotScopeCount: 0, largestScopeHotBytes: 0,
    canonicalCount: 0, episodicCount: 0, evidenceCount: 0, candidateCount: 0,
    invalidatedCount: 0, protectedArtifactCount: 0, duplicateMemoryGroups: 0,
    staleMemoryCount: 0, reusedMemoryCount: 0, contextTransferBytes: 0,
    contextReintroducedBytes: 0, contextReintroductionBasisPoints: 0,
    contextAmplificationBasisPoints: 0, retrievalUsefulnessBasisPoints: 0,
    pendingMaintenanceProposals: 0, sqliteBytes: 0, sqliteGrowthBytes: 0,
    orphanedEvidenceCount: 0, duplicateArtifactGroups: 0, bytesCollected: 0,
  },
});

describe("dashboard and local Codex settings", () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockReset();
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: vi.fn().mockReturnValue({ matches: false }),
    });
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
  });

  it("renders the snapshot returned by the Tauri command", async () => {
    invokeMock.mockResolvedValueOnce({
      logicalRoles: [mainRole, {
        roleKey: "project-manager:1",
        roleKind: "project_manager",
        projectId: 1,
        projectName: "Atlas",
        memoryScopeKey: "project:1",
        receiverScopeKey: "scope:project-manager:1",
        toolProfileKey: "project-manager-v1",
        createdAt: 1_800_000_000,
      }],
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
      agentActivity: [],
      agentUpdates: [],
      pendingAgentActions: [],
      pendingOwnerApprovalRequests: [],
      recentAgentUpdateCount: 0,
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
      roleDispatches: [],
      roleInbox: [],
      failedChecks: [
        { id: 5, runId: 6, projectName: "Atlas", checkKey: "integration", mandatory: true, status: "failed", details: {}, updatedAt: 1_800_000_115 },
        { id: 9, runId: 8, projectName: "Atlas", checkKey: "packaging", mandatory: false, status: "inconclusive", details: {}, updatedAt: 1_800_000_116 },
      ],
      memoryHealth: { hotBytes: 512, warmBytes: 2048, coldBytes: 4096, hotScopeCount: 2, largestScopeHotBytes: 256, canonicalCount: 3, episodicCount: 4, evidenceCount: 5, candidateCount: 1, invalidatedCount: 2, protectedArtifactCount: 1, duplicateMemoryGroups: 0, staleMemoryCount: 2, reusedMemoryCount: 8, contextTransferBytes: 1000, contextReintroducedBytes: 128, contextReintroductionBasisPoints: 1280, contextAmplificationBasisPoints: 15000, retrievalUsefulnessBasisPoints: 7500, pendingMaintenanceProposals: 2, sqliteBytes: 8192, sqliteGrowthBytes: 2048, orphanedEvidenceCount: 1, duplicateArtifactGroups: 0, bytesCollected: 0 },
    } as never);

    render(<App />);

    expect(await screen.findByRole("region", { name: "Project activity graph" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Main Overseer/ })).toBeTruthy();
    const projectManager = screen.getByRole("button", { name: /Atlas Project Manager/ });
    expect(projectManager.getAttribute("aria-label")).toContain("latest task Verify");
    expect(screen.queryByRole("navigation", { name: "Dashboard views" })).toBeNull();
    expect(screen.queryByText("Recent runs")).toBeNull();
    expect(invokeMock).toHaveBeenCalledWith("dashboard_snapshot");
    expect(screen.getByRole("link", { name: "Cathulu activity" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open Codex settings" })).toBeTruthy();
  });

  it("shows the empty state when no projects are registered", async () => {
    invokeMock.mockResolvedValueOnce(emptyDashboardSnapshot() as never);

    render(<App />);

    expect(await screen.findByRole("button", { name: /Main Overseer/ })).toBeTruthy();
    expect(screen.getByText("No registered project roles")).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Dashboard views" })).toBeNull();
  });

  it("keeps Main visibly idle while project agents continue working", async () => {
    invokeMock.mockResolvedValueOnce({
      ...emptyDashboardSnapshot(),
      logicalRoles: [mainRole, {
        roleKey: "project-manager:1", roleKind: "project_manager", projectId: 1,
        projectName: "Atlas", memoryScopeKey: "project:1", receiverScopeKey: "scope:project-manager:1",
        toolProfileKey: "project-manager-v1", createdAt: 1_800_000_000,
      }],
      projects: [{ id: 1, name: "Atlas", path: "C:\\work\\atlas", branch: "main", head: null, dirty: false, changedPaths: [], registeredAt: 1_800_000_000, reconciledAt: 1_800_000_100 }],
      recentRuns: [{ id: 1, projectName: "Atlas", objectiveTitle: "Implement feature", taskTitle: null, workflowKey: null, workflowVersion: null, status: "running", summary: "", startedAt: 1_800_000_100, finishedAt: null }],
    } as never);

    render(<App />);

    const mainNode = await screen.findByRole("button", { name: /Main Overseer/ });
    expect(mainNode.className).not.toContain("is-active");
    expect(mainNode.textContent).toContain("IDLE · DELEGATES RUN");
  });

  it("shows Main as working only while a routed Main inbox turn is active", async () => {
    invokeMock.mockResolvedValueOnce({
      ...emptyDashboardSnapshot(),
      roleInbox: [{ id: 3, sourceDelegationId: 10, targetBindingKey: "main-overseer", projectId: 1, projectName: "Atlas", eventType: "delegation_report_submitted", status: "running", attemptCount: 1, lastError: null, createdAt: 1_800_000_100, updatedAt: 1_800_000_101 }],
    } as never);

    render(<App />);

    const mainNode = await screen.findByRole("button", { name: /Main Overseer/ });
    expect(mainNode.className).toContain("is-active");
    expect(mainNode.textContent).toContain("REVIEWING UPDATE");
    fireEvent.click(mainNode);
    const dossier = await screen.findByRole("region", { name: "Main Overseer work details" });
    expect(within(dossier).getByText("Working")).toBeTruthy();
    expect(within(dossier).getByText("Main event trail")).toBeTruthy();
  });

  it("distinguishes Main escalations from requests that actually need the owner", async () => {
    const snapshot = emptyDashboardSnapshot();
    snapshot.pendingAgentActions = [{
      eventId: 4,
      projectId: 1,
      projectName: "Atlas",
      delegationId: 10,
      runId: 3,
      role: "sub_overseer",
      routedToRole: "main_overseer",
      actionKey: "contract-choice",
      summary: "The PM needs a bounded contract decision.",
      ownerAction: "Choose the supported contract version",
      createdAt: 1_800_000_120,
    }];
    snapshot.pendingOwnerApprovalRequests = [{
      id: 7,
      projectId: 1,
      projectName: "Atlas",
      runId: 3,
      managerDelegationId: 10,
      actionKey: "destructive-change",
      requestSummary: "The PM escalated an irreversible operation.",
      ownerAction: "Approve the irreversible operation",
      reason: "It is outside autonomous role authority.",
      requestHash: "a".repeat(64),
      createdAt: 1_800_000_121,
    }];
    invokeMock.mockResolvedValueOnce(snapshot as never);

    render(<App />);

    const pulse = await screen.findByLabelText("Current workflow process");
    expect(pulse.textContent).toContain("1Needs you");
    expect(pulse.textContent).toContain("1Needs Main");
    expect(screen.getByText("1 OWNER ACTION")).toBeTruthy();
  });

  it("opens the separate local Codex settings surface and labels catalog availability as unverified", async () => {
    invokeMock
      .mockResolvedValueOnce(emptyDashboardSnapshot() as never)
      .mockResolvedValueOnce({
        catalog: {
          source: "bundled_local_codex_cli",
          cliVersion: "codex-cli test",
          accountAvailabilityVerified: false,
          models: [{ modelSlug: "model-a", displayName: "Model A", reasoningEfforts: ["low", "high"], defaultReasoningEffort: "low" }],
        },
        preferences: [],
        preferenceState: "stored_in_cathulu",
        runtimeEffectivenessVerified: false,
      } as never);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Open Codex settings" }));

    expect(await screen.findByRole("heading", { name: "Codex settings" })).toBeTruthy();
    expect(await screen.findByText(/does not verify account or workspace availability/)).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Main Overseer" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Default sub-agents" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Named agent" })).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("codex_selector_snapshot");
  });

  it("requires an explicit reviewed apply after saving preferences locally", async () => {
    const preference = {
      roleId: "main_overseer",
      roleKind: "main_overseer",
      agentName: null,
      modelSlug: "model-a",
      reasoningEffort: "low",
      catalogVersion: "codex-cli test",
      lastAppliedTarget: null,
      lastAppliedAt: null,
      lastAppliedConfigHash: null,
      updatedAt: 1_800_000_000,
    };
    const selectorSnapshot = (written = false) => ({
      catalog: {
        source: "bundled_local_codex_cli",
        cliVersion: "codex-cli test",
        accountAvailabilityVerified: false,
        models: [{ modelSlug: "model-a", displayName: "Model A", reasoningEfforts: ["low", "high"], defaultReasoningEffort: "low" }],
      },
      preferences: [written ? { ...preference, lastAppliedTarget: "user_base", lastAppliedAt: 1_800_000_001, lastAppliedConfigHash: "a".repeat(64) } : preference],
      preferenceState: written ? "written_to_config" : "stored_in_cathulu",
      runtimeEffectivenessVerified: false,
    });
    invokeMock
      .mockResolvedValueOnce(emptyDashboardSnapshot() as never)
      .mockResolvedValueOnce(selectorSnapshot() as never)
      .mockResolvedValueOnce(preference as never)
      .mockResolvedValueOnce(selectorSnapshot() as never)
      .mockResolvedValueOnce({
        previewId: "preview-1",
        target: "User · config.toml",
        changes: [
          { file: "config.toml", key: "model", currentValue: null, selectedValue: "model-a" },
          { file: "config.toml", key: "model_reasoning_effort", currentValue: null, selectedValue: "low" },
        ],
        skippedRoles: [],
        exportText: "model = \"model-a\"",
        canApply: true,
        exportOnly: false,
      } as never)
      .mockResolvedValueOnce({
        target: "User · config.toml",
        preferenceState: "written_to_config",
        backupCount: 1,
        configHash: "a".repeat(64),
        runtimeEffectivenessVerified: false,
      } as never)
      .mockResolvedValueOnce(selectorSnapshot(true) as never);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Open Codex settings" }));
    await screen.findByRole("heading", { name: "Main Overseer" });
    fireEvent.click(screen.getAllByRole("button", { name: "Save preference" })[0]);
    expect(await screen.findByText(/saved in Cathulu/)).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("save_codex_selector_preference", {
      input: { roleKind: "main_overseer", agentName: null, modelSlug: "model-a", reasoningEffort: "low" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Review configuration changes" }));
    expect(await screen.findByRole("heading", { name: "Review before applying" })).toBeTruthy();
    expect(screen.getAllByText("not set")).toHaveLength(2);
    expect(screen.queryByRole("button", { name: "Apply reviewed config" })).toBeTruthy();
    expect(invokeMock).not.toHaveBeenCalledWith("apply_codex_config_preview", expect.anything());

    fireEvent.click(screen.getByRole("button", { name: "Apply reviewed config" }));
    expect(await screen.findByText(/Cathulu created 1 backup/)).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("apply_codex_config_preview", { previewId: "preview-1" });
    expect(screen.getByText("Runtime effectiveness unverified")).toBeTruthy();
  });

  it("shows command failures as a visible local-state error", async () => {
    invokeMock.mockRejectedValueOnce(new Error("database unavailable"));

    render(<App />);

    expect((await screen.findByRole("alert")).textContent).toContain("database unavailable");
  });
});
