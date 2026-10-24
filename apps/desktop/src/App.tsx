import { invoke, isTauri } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { currentMonitor, getCurrentWindow, LogicalPosition, LogicalSize } from "@tauri-apps/api/window";
import { type CSSProperties, type ReactNode, useEffect, useState } from "react";
import CodexSettings from "./CodexSettings";

type ProjectSnapshot = {
  id: number;
  name: string;
  path: string;
  branch: string | null;
  head: string | null;
  dirty: boolean;
  changedPaths: string[];
  registeredAt: number;
  reconciledAt: number;
};

type WorkerCheckResult = { name: string; status: string; summary: string };
type WorkerReportPayload = {
  schemaVersion: number;
  summary: string;
  outcome: string;
  changedFiles: string[];
  commits: string[];
  checkResults: WorkerCheckResult[];
  blockers: string[];
  artifactIds: number[];
  qualityCheckIds: number[];
};
type WorkerReportRecord = {
  reportId: number;
  delegationId: number;
  reviewStatus: "reported" | "accepted" | "rejected" | "needs_follow_up";
  report: WorkerReportPayload;
  reportHash: string;
  reportedAt: number;
  qualityCheckRefs: { qualityCheckId: number; checkKey: string; mandatory: boolean; status: string }[];
  reviewHistory: { id: number; decision: string; note: string; createdAt: number }[];
};
type DelegationWorktreeState = {
  worktreePathHash: string | null;
  branch: string | null;
  baseCommit: string | null;
  observedHead: string | null;
  isClean: boolean | null;
  changedPaths: string[];
  reportVerification: {
    status: string;
    verifiedCommits: string[];
    unverifiedCommits: string[];
    verifiedPaths: string[];
    unverifiedPaths: string[];
  } | null;
  lastReconciledAt: number | null;
};
type DelegationWorkerTelemetry = {
  hookEventId: number;
  agentType: string;
  effectiveModel: string | null;
  effectiveEffort: string | null;
  linkedAt: number;
  events: { eventId: number; eventKind: string; agentType: string; model: string | null; createdAt: number }[];
};
type DelegationSnapshot = {
  id: number;
  projectId: number;
  projectName: string;
  taskTitle: string;
  runId: number;
  role: string;
  status: string;
  parentDelegationId: number | null;
  logicalRoleKey: string | null;
  projectManagerAssignmentStatus: string | null;
  activationStatus: string | null;
  parentRunStatus: string;
  createdAt: number;
  updatedAt: number;
  report: WorkerReportRecord | null;
  worktree: DelegationWorktreeState | null;
  workerTelemetry: DelegationWorkerTelemetry | null;
};
type AgentUpdateSnapshot = {
  eventId: number;
  projectId: number;
  projectName: string;
  delegationId: number;
  runId: number;
  parentDelegationId: number | null;
  role: string;
  updateType: "working" | "waiting" | "action_required" | "action_resolved";
  phase: "planning" | "investigating" | "implementing" | "verifying" | "reviewing" | "coordinating" | null;
  summary: string;
  actionKey: string | null;
  ownerAction: string | null;
  createdAt: number;
};
type PendingAgentAction = {
  eventId: number;
  projectId: number;
  projectName: string;
  delegationId: number;
  runId: number;
  role: string;
  routedToRole: "main_overseer" | "project_manager";
  actionKey: string;
  summary: string;
  ownerAction: string;
  createdAt: number;
};
type PendingOwnerApproval = {
  id: number;
  projectId: number;
  projectName: string;
  runId: number;
  managerDelegationId: number;
  actionKey: string;
  requestSummary: string;
  ownerAction: string;
  reason: string;
  requestHash: string;
  createdAt: number;
};
type LogicalAgentRole = {
  roleKey: string;
  roleKind: "main_overseer" | "project_manager";
  projectId: number | null;
  projectName: string | null;
  memoryScopeKey: string;
  receiverScopeKey: string;
  toolProfileKey: string;
  createdAt: number;
};
type DashboardSnapshot = {
  logicalRoles: LogicalAgentRole[];
  projects: ProjectSnapshot[];
  objectives: { id: number; projectId: number; projectName: string; title: string; status: string; updatedAt: number }[];
  blockers: { id: number; objectiveId: number; projectName: string; title: string; updatedAt: number }[];
  recentRuns: {
    id: number;
    projectName: string;
    objectiveTitle: string | null;
    taskTitle: string | null;
    workflowKey: string | null;
    workflowVersion: number | null;
    status: string;
    summary: string;
    startedAt: number;
    finishedAt: number | null;
  }[];
  agentActivity: {
    id: number;
    runId: number;
    agentType: string;
    model: string | null;
    startedAt: number | null;
    stoppedAt: number | null;
    lastEventAt: number;
    eventCount: number;
  }[];
  agentUpdates: AgentUpdateSnapshot[];
  pendingAgentActions: PendingAgentAction[];
  pendingOwnerApprovalRequests: PendingOwnerApproval[];
  recentAgentUpdateCount: number;
  delegations: DelegationSnapshot[];
  roleDispatches: {
    id: number; delegationId: number; projectId: number; projectName: string;
    taskTitle: string; role: string; status: string; attemptCount: number;
    lastError: string | null; createdAt: number; updatedAt: number;
  }[];
  roleInbox: {
    id: number; sourceDelegationId: number; targetBindingKey: string;
    projectId: number; projectName: string; eventType: string; status: string;
    attemptCount: number; lastError: string | null; createdAt: number; updatedAt: number;
  }[];
  failedChecks: { id: number; runId: number; projectName: string; checkKey: string; mandatory: boolean; status: string; details: unknown; updatedAt: number }[];
  memoryHealth: {
    hotBytes: number; hotScopeCount: number; largestScopeHotBytes: number;
    warmBytes: number; coldBytes: number; canonicalCount: number;
    episodicCount: number; evidenceCount: number; workingCount: number; aggregateCount: number;
    candidateCount: number;
    invalidatedCount: number; protectedArtifactCount: number;
    duplicateMemoryGroups: number; staleMemoryCount: number; reusedMemoryCount: number;
    contextTransferBytes: number; contextReintroducedBytes: number; contextReintroductionBasisPoints: number;
    contextAmplificationBasisPoints: number;
    retrievalUsefulnessBasisPoints: number; pendingMaintenanceProposals: number;
    sqliteBytes: number; sqliteGrowthBytes: number; orphanedEvidenceCount: number;
    duplicateArtifactGroups: number; bytesCollected: number;
  };
};

type RunRecord = DashboardSnapshot["recentRuns"][number];
type HookAgentRecord = DashboardSnapshot["agentActivity"][number];
type ProjectMonitorState = "working" | "queued" | "stale" | "blocked" | "failed" | "inconclusive" | "complete" | "stopped" | "idle";
type ProjectMonitorItem = {
  project: ProjectSnapshot;
  state: ProjectMonitorState;
  detail: string;
  activeAgents: number;
  issueCount: number;
  attentionCount: number;
  staleAgentCount: number;
  latestRun: RunRecord | null;
  latestUpdate: AgentUpdateSnapshot | null;
  latestDispatch: DashboardSnapshot["roleDispatches"][number] | null;
  pendingAction: PendingAgentAction | null;
  pendingOwnerApproval: PendingOwnerApproval | null;
  failedCheckKeys: string[];
};

const AGENT_SIGNAL_FRESHNESS_SECONDS = 15 * 60;

function hasRecentAgentSignal(timestamp: number, now = Date.now()) {
  return timestamp > 0 && now / 1000 - timestamp <= AGENT_SIGNAL_FRESHNESS_SECONDS;
}

function nativeRuntimeAvailable() {
  // Test harnesses may mock only `invoke`; the real browser import always exposes isTauri().
  return typeof isTauri === "function" ? isTauri() : true;
}

function runTitle(run: RunRecord) {
  return (run.taskTitle ?? run.objectiveTitle ?? run.workflowKey ?? `Run ${run.id}`).replace(/_/g, " ");
}

function projectMonitorItems(snapshot: DashboardSnapshot): ProjectMonitorItem[] {
  const priority: Record<ProjectMonitorState, number> = { working: 0, queued: 1, stale: 2, blocked: 3, failed: 4, inconclusive: 5, complete: 6, stopped: 7, idle: 8 };
  return snapshot.projects.map((project) => {
    const latestRun = snapshot.recentRuns
      .filter((run) => run.projectName === project.name)
      .sort((left, right) => right.startedAt - left.startedAt || right.id - left.id)[0] ?? null;
    const blockers = snapshot.blockers.filter((blocker) => blocker.projectName === project.name);
    const mandatoryFailures = latestRun
      ? snapshot.failedChecks.filter((check) => check.runId === latestRun.id && check.mandatory && check.status.toLowerCase() === "failed")
      : [];
    const pendingOwnerApproval = snapshot.pendingOwnerApprovalRequests
      .filter((request) => request.projectId === project.id)
      .sort((left, right) => right.createdAt - left.createdAt || right.id - left.id)[0] ?? null;
    const ownerRequestKeys = new Set(snapshot.pendingOwnerApprovalRequests
      .filter((request) => request.projectId === project.id)
      .map((request) => `${request.managerDelegationId}:${request.actionKey}`));
    const pendingActions = snapshot.pendingAgentActions
      .filter((action) => action.projectId === project.id && !ownerRequestKeys.has(`${action.delegationId}:${action.actionKey}`))
      .sort((left, right) => right.createdAt - left.createdAt || right.eventId - left.eventId);
    const latestUpdate = latestRun
      ? snapshot.agentUpdates
        .filter((update) => update.projectId === project.id && update.runId === latestRun.id)
        .sort((left, right) => right.createdAt - left.createdAt || right.eventId - left.eventId)[0] ?? null
      : null;
    const delegations = latestRun ? snapshot.delegations.filter((item) => item.runId === latestRun.id) : [];
    const latestDispatch = snapshot.roleDispatches
      .filter((item) => item.projectId === project.id)
      .sort((left, right) => right.updatedAt - left.updatedAt || right.id - left.id)[0] ?? null;
    const dispatchNeedsReview = latestDispatch !== null && ["blocked", "failed", "needs_reconcile"].includes(latestDispatch.status);
    const dispatchQueued = latestDispatch?.status === "queued";
    const dispatchActive = latestDispatch !== null && ["preflighting", "thread_ready", "turn_starting", "dispatched", "running"].includes(latestDispatch.status);
    const managerIds = new Set(delegations
      .filter((delegation) => delegation.role === "sub_overseer" && delegation.projectManagerAssignmentStatus === "active")
      .map((delegation) => delegation.id));
    const trackedAgents = delegations.filter((delegation) =>
      managerIds.has(delegation.id) || (delegation.role === "worker" && delegation.parentDelegationId !== null && managerIds.has(delegation.parentDelegationId)),
    );
    const boundHookEventIds = new Set(delegations.flatMap((delegation) =>
      delegation.workerTelemetry?.events.map((event) => event.eventId) ?? [],
    ));
    const hookAgents = latestRun
      ? snapshot.agentActivity.filter((item) => item.runId === latestRun.id && !boundHookEventIds.has(item.id))
      : [];
    const activeAgents = trackedAgents.filter((item) => agentState(item, snapshot.agentUpdates) === "active").length
      + hookAgents.filter((item) => hookAgentState(item, latestRun?.status ?? "unknown") === "active").length;
    const staleAgentCount = trackedAgents.filter((item) => agentState(item, snapshot.agentUpdates) === "stale").length
      + hookAgents.filter((item) => hookAgentState(item, latestRun?.status ?? "unknown") === "stale").length;
    const state: ProjectMonitorState = pendingActions.length > 0 || pendingOwnerApproval !== null || dispatchNeedsReview
      ? "blocked"
      : dispatchActive
        ? "working"
        : dispatchQueued
          ? "queued"
      : latestRun?.status === "running" && staleAgentCount > 0 && activeAgents === 0
        ? "stale"
      : latestRun?.status === "running"
      ? "working"
      : latestRun?.status === "blocked" || (!latestRun && blockers.length > 0)
        ? "blocked"
        : latestRun?.status === "failed" || mandatoryFailures.length > 0
          ? "failed"
          : latestRun?.status === "inconclusive"
            ? "inconclusive"
            : latestRun?.status === "verified_success"
              ? "complete"
              : latestRun
                ? "stopped"
                : "idle";
    const detail = pendingOwnerApproval?.ownerAction ?? pendingActions[0]?.ownerAction
      ?? (dispatchNeedsReview ? latestDispatch?.lastError ?? "Role dispatch needs review" : null)
      ?? (dispatchQueued ? `Queued · ${latestDispatch?.taskTitle ?? "role assignment"}` : null)
      ?? (dispatchActive ? `${latestDispatch?.role === "worker" ? "Worker" : "Project Manager"} · ${latestDispatch?.taskTitle ?? "starting"}` : null)
      ?? (blockers.length > 0 && !latestRun
      ? blockers[0].title
      : staleAgentCount > 0
        ? `${staleAgentCount} agent signal${staleAgentCount === 1 ? "" : "s"} older than 15m; activity unconfirmed`
      : latestRun
        ? runTitle(latestRun)
        : "No recorded work");
    return {
      project,
      state,
      detail,
      activeAgents,
      issueCount: blockers.length + mandatoryFailures.length + pendingActions.length + (pendingOwnerApproval ? 1 : 0) + staleAgentCount + (dispatchNeedsReview ? 1 : 0),
      attentionCount: pendingActions.length + (pendingOwnerApproval ? 1 : 0) + (dispatchNeedsReview ? 1 : 0),
      staleAgentCount,
      latestRun,
      latestUpdate,
      latestDispatch,
      pendingAction: pendingActions[0] ?? null,
      pendingOwnerApproval,
      failedCheckKeys: mandatoryFailures.map((check) => check.checkKey),
    };
  }).sort((left, right) => {
    return priority[left.state] - priority[right.state] || left.project.name.localeCompare(right.project.name);
  });
}

function monitorStateLabel(state: ProjectMonitorState) {
  return state === "working" ? "Working" : state === "queued" ? "Queued" : state === "stale" ? "No fresh signal" : state === "blocked" ? "Needs review" : state === "failed" ? "Failed" : state === "inconclusive" ? "Unverified" : state === "complete" ? "Verified" : state === "stopped" ? "Stopped" : "Idle";
}

function projectMonitorLabel(item: ProjectMonitorItem) {
  if (item.pendingOwnerApproval) return "Needs you";
  if (item.pendingAction) return item.pendingAction.routedToRole === "project_manager" ? "Needs PM" : "Needs Main";
  if (item.latestDispatch && ["blocked", "failed", "needs_reconcile"].includes(item.latestDispatch.status)) return "Needs review";
  return monitorStateLabel(item.state);
}

function actionRecipientLabel(role: string) {
  return role === "project_manager" ? "Needs PM" : "Needs Main";
}

function roleLabel(role: string) {
  return role === "sub_overseer" ? "Project Manager" : role === "worker" ? "Worker" : role.replace(/_/g, " ");
}

function updateAge(timestamp: number, now = Date.now()) {
  const minutes = Math.max(0, Math.floor((now / 1000 - timestamp) / 60));
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

function latestAgentUpdate(snapshot: DashboardSnapshot, delegationId: number) {
  return snapshot.agentUpdates.find((update) => update.delegationId === delegationId) ?? null;
}

function attentionItems(
  snapshot: DashboardSnapshot,
  staleAgents: { key: string; source: string; title: string; detail: string; createdAt: number; priority: number }[] = [],
) {
  const ownerRequests = snapshot.pendingOwnerApprovalRequests.map((request) => ({
    key: `owner-${request.id}`,
    source: `${request.projectName} · Owner approval`,
    title: request.ownerAction,
    detail: `${request.requestSummary} · ${request.reason}`,
    createdAt: request.createdAt,
    priority: 0,
    label: "Needs you",
  }));
  const ownerRequestKeys = new Set(snapshot.pendingOwnerApprovalRequests.map((request) => `${request.managerDelegationId}:${request.actionKey}`));
  const actions = snapshot.pendingAgentActions.filter((action) => !ownerRequestKeys.has(`${action.delegationId}:${action.actionKey}`)).map((action) => ({
    key: `agent-${action.eventId}`,
    source: `${action.projectName} · ${roleLabel(action.role)}`,
    title: action.ownerAction,
    detail: action.summary,
    createdAt: action.createdAt,
    priority: action.routedToRole === "main_overseer" ? 1 : 3,
    label: actionRecipientLabel(action.routedToRole),
  }));
  const blockedTasks = snapshot.blockers.map((blocker) => ({
    key: `task-${blocker.id}`,
    source: blocker.projectName,
    title: blocker.title,
    detail: "Blocked task recorded in Cathulu",
    createdAt: blocker.updatedAt,
    priority: 1,
    label: "Review",
  }));
  const failedChecks = snapshot.failedChecks
    .filter((check) => check.mandatory && check.status.toLowerCase() === "failed")
    .map((check) => ({
      key: `check-${check.id}`,
      source: `${check.projectName} · required check`,
      title: check.checkKey.replace(/_/g, " "),
      detail: "Required QA check failed",
      createdAt: check.updatedAt,
      priority: 2,
      label: "QA",
    }));
  const dispatchIssues = snapshot.roleDispatches
    .filter((item) => ["blocked", "failed", "needs_reconcile"].includes(item.status))
    .map((item) => ({
      key: `dispatch-${item.id}`,
      source: `${item.projectName} · ${roleLabel(item.role)}`,
      title: item.taskTitle,
      detail: item.lastError ?? (item.status === "needs_reconcile" ? "Dispatch state must be reconciled before retry" : "Role dispatch needs review"),
      createdAt: item.updatedAt,
      priority: 1,
      label: "Dispatch",
    }));
  const normalizedStale = staleAgents.map((item) => ({ ...item, label: "Check" }));
  return [...ownerRequests, ...actions, ...blockedTasks, ...failedChecks, ...dispatchIssues, ...normalizedStale]
    .sort((left, right) => left.priority - right.priority || right.createdAt - left.createdAt)
    .slice(0, 4);
}

function BrandMark() {
  return <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>;
}

function Icon({ name }: { name: "monitor" | "settings" | "pin" | "expand" | "collapse" | "open" | "close" }) {
  const paths: Record<typeof name, ReactNode> = {
    monitor: <><rect x="3" y="4" width="18" height="13" rx="2" /><path d="M8 21h8M12 17v4" /></>,
    settings: <><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-1.7 2.9-.2-.1a1.8 1.8 0 0 0-1.9.1 1.7 1.7 0 0 0-.9 1.5v.2h-3.4v-.2a1.7 1.7 0 0 0-1-1.5 1.8 1.8 0 0 0-1.8-.1l-.2.1L7 17l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H5.7v-3.4h.2a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.9L7 7.6l1.7-2.9.2.1a1.8 1.8 0 0 0 1.9-.1 1.7 1.7 0 0 0 .9-1.5V3h3.4v.2a1.7 1.7 0 0 0 1 1.5 1.8 1.8 0 0 0 1.8.1l.2-.1 1.7 2.9-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.5 1h.2V14h-.2a1.7 1.7 0 0 0-1.3 1Z" /></>,
    pin: <><path d="m16 3 5 5-4 1-4 4 3 3-1 1-4-2-5 5-1-1 5-5-2-4 1-1 3 3 4-4 1-4Z" /></>,
    expand: <><path d="M8 4H4v4M16 4h4v4M4 16v4h4M20 16v4h-4" /><path d="m4 4 6 6m10-6-6 6M4 20l6-6m10 6-6-6" /></>,
    collapse: <><path d="M8 4v4H4M16 4v4h4M4 16h4v4M20 16h-4v4" /></>,
    open: <><path d="M14 4h6v6M20 4l-9 9" /><path d="M18 13v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h6" /></>,
    close: <><path d="m6 6 12 12M18 6 6 18" /></>,
  };
  return <svg className="ui-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>;
}

export default function App() {
  const [nativeRuntime] = useState(nativeRuntimeAvailable);
  const isMonitorWindow = new URLSearchParams(window.location.search).get("window") === "monitor";
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [monitorError, setMonitorError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const projects = snapshot?.projects ?? [];

  useEffect(() => {
    if (!nativeRuntime) {
      setLoading(false);
      return;
    }
    let cancelled = false;
    const loadSnapshot = () => {
      invoke<DashboardSnapshot>("dashboard_snapshot")
        .then((nextSnapshot) => {
          if (!cancelled) {
            setSnapshot(nextSnapshot);
            setError(null);
          }
        })
        .catch((reason: unknown) => {
          if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    };
    loadSnapshot();
    const poll = window.setInterval(loadSnapshot, 5_000);
    const refreshSnapshot = () => loadSnapshot();
    window.addEventListener("cathulu:refresh-dashboard", refreshSnapshot);
    return () => {
      cancelled = true;
      window.clearInterval(poll);
      window.removeEventListener("cathulu:refresh-dashboard", refreshSnapshot);
    };
  }, [nativeRuntime]);

  const openMonitor = async () => {
    try {
      const existing = await WebviewWindow.getByLabel("monitor");
      if (existing) {
        await existing.unminimize();
        await existing.show();
        await existing.setFocus();
        setMonitorError(null);
        return;
      }
      let placement: { center: true } | { x: number; y: number } = { center: true };
      const display = await currentMonitor();
      if (display) {
        const origin = display.position.toLogical(display.scaleFactor);
        const size = display.size.toLogical(display.scaleFactor);
        placement = {
          x: Math.max(origin.x + 8, origin.x + size.width - 174 - 8),
          y: Math.max(origin.y + 8, origin.y + size.height - 58 - 48),
        };
      }
      const monitor = new WebviewWindow("monitor", {
        url: "/?window=monitor",
        title: "Cathulu Monitor",
        width: 174,
        height: 58,
        minWidth: 174,
        minHeight: 58,
        ...placement,
        resizable: false,
        decorations: false,
        transparent: true,
        alwaysOnTop: true,
        skipTaskbar: true,
        shadow: false,
        noRedirectionBitmap: true,
      });
      monitor.once("tauri://error", () => setMonitorError("The floating monitor could not open. Try restarting Cathulu."));
      setMonitorError(null);
    } catch (reason) {
      setMonitorError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  if (!nativeRuntime) return <BrowserLaunchNotice />;
  if (isMonitorWindow) return <MonitorWindow snapshot={snapshot} loading={loading} error={error} />;

  return (
    <div className="support-shell">
      <header className="support-header">
        <a className="support-brand" href="#dashboard" aria-label="Cathulu activity" onClick={() => setSettingsOpen(false)}>
          <BrandMark /><span>CATHULU</span>
        </a>
        <div className="support-header-actions">
          <span className={`local-status ${error ? "has-error" : ""}`} title={error ? "Snapshot refresh failed" : "Connected to local state"}><i aria-hidden="true" /></span>
          <button className="toolbar-button" type="button" onClick={openMonitor} aria-label="Open floating monitor" title="Open floating monitor">
            <Icon name="monitor" />
          </button>
          <button className="toolbar-button" type="button" onClick={() => setSettingsOpen(true)} aria-label="Open Codex settings" title="Codex settings">
            <Icon name="settings" />
          </button>
        </div>
      </header>

      {settingsOpen ? (
        <div className="settings-page">
          <div className="settings-topline"><span>Codex configuration</span></div>
          <CodexSettings projects={projects} onBack={() => setSettingsOpen(false)} />
        </div>
      ) : (
        <main id="dashboard" className="support-main">
          <div className="support-content">
            {error && snapshot && <span className="refresh-indicator" role="status" title="Refresh failed. Showing the last saved snapshot.">!</span>}
            {monitorError && <p className="inline-action-error" role="status">{monitorError}</p>}
            {!snapshot ? (
              <SnapshotState loading={loading} error={error} />
            ) : (
              <AgentActivityView snapshot={snapshot} />
            )}
          </div>
        </main>
      )}
    </div>
  );
}

function BrowserLaunchNotice() {
  return (
    <main className="browser-launch-screen">
      <section className="browser-launch-card" aria-labelledby="browser-launch-title">
        <div className="browser-brand"><BrandMark />CATHULU <span>DESKTOP</span></div>
        <p className="browser-launch-kicker">NATIVE CONNECTION REQUIRED</p>
        <h1 id="browser-launch-title">Open the desktop app to read local state.</h1>
        <p className="browser-launch-copy">This page is running in a regular browser, which has no connection to Cathulu's local database. No repository data was requested.</p>
        <div className="launch-command"><span>FROM APPS/DESKTOP</span><code>cargo tauri dev</code></div>
        <p className="browser-launch-foot">In PowerShell, run that command from the <code>apps/desktop</code> folder.</p>
      </section>
    </main>
  );
}

function SnapshotState({ loading, error }: { loading: boolean; error: string | null }) {
  if (loading) return <div className="snapshot-state" role="status"><span className="skeleton-line long" /><span className="skeleton-line" /><span className="skeleton-line short" /><p>Reading the local snapshot…</p></div>;
  if (error) return <div className="snapshot-state state-error" role="alert"><span className="state-mark">!</span><div><h2>Local state could not be read</h2><p>{error}</p><p>Keep Cathulu running and confirm its local data folder is available.</p></div></div>;
  return <div className="snapshot-state"><h2>No snapshot yet</h2><p>Register a repository through Cathulu's local MCP server to see its current Git state here.</p></div>;
}

type AgentVisualState = "active" | "waiting" | "stale" | "finished" | "blocked" | "unknown";

function agentState(delegation: DelegationSnapshot, updates: AgentUpdateSnapshot[] = []): AgentVisualState {
  const lastEvent = [...(delegation.workerTelemetry?.events ?? [])].sort((a, b) => a.createdAt - b.createdAt).at(-1);
  const lastUpdate = updates.find((update) => update.delegationId === delegation.id);
  const status = delegation.status.toLowerCase();
  const outcome = delegation.report?.report.outcome.toLowerCase();
  if (["blocked", "failed", "cancelled"].includes(status) || outcome === "blocked" || delegation.report?.report.blockers.length) return "blocked";
  if (["failed", "blocked"].includes(delegation.parentRunStatus.toLowerCase())) return "blocked";
  if (delegation.report || ["verified_success", "inconclusive", "cancelled"].includes(delegation.parentRunStatus.toLowerCase())) return "finished";
  if (lastUpdate && (!lastEvent || lastUpdate.createdAt >= lastEvent.createdAt)) {
    if (lastUpdate.updateType === "action_required") return "blocked";
    if (!hasRecentAgentSignal(lastUpdate.createdAt)) return "stale";
    if (lastUpdate.updateType === "waiting") return "waiting";
    if (lastUpdate.updateType === "working" || lastUpdate.updateType === "action_resolved") return "active";
  }
  if (lastEvent?.eventKind === "started" || ["active", "running"].includes(status)) {
    return hasRecentAgentSignal(Math.max(lastEvent?.createdAt ?? 0, delegation.updatedAt)) ? "active" : "stale";
  }
  if (lastEvent?.eventKind === "stopped" || ["completed", "accepted", "reported"].includes(status) || delegation.report) return "finished";
  if (["prepared", "dispatched", "pending", "waiting"].includes(status)) {
    return hasRecentAgentSignal(delegation.updatedAt) ? "waiting" : "stale";
  }
  return "unknown";
}

function hookAgentState(agent: HookAgentRecord, runStatus: string): AgentVisualState {
  if (agent.stoppedAt !== null) return "finished";
  if (agent.startedAt !== null && runStatus === "running") {
    return hasRecentAgentSignal(agent.lastEventAt) ? "active" : "stale";
  }
  return "unknown";
}

function staleAgentAttentionItems(snapshot: DashboardSnapshot) {
  const managerIds = new Set(snapshot.delegations
    .filter((delegation) => delegation.role === "sub_overseer" && delegation.projectManagerAssignmentStatus === "active")
    .map((delegation) => delegation.id));
  const activeDelegations = snapshot.delegations.filter((delegation) =>
    managerIds.has(delegation.id) || (delegation.role === "worker" && delegation.parentDelegationId !== null && managerIds.has(delegation.parentDelegationId)),
  );
  const delegationItems = activeDelegations
    .filter((delegation) => agentState(delegation, snapshot.agentUpdates) === "stale")
    .map((delegation) => {
      const update = latestAgentUpdate(snapshot, delegation.id);
      const lastSignalAt = Math.max(update?.createdAt ?? 0, delegation.updatedAt);
      return {
        key: `stale-delegation-${delegation.id}`,
        source: `${delegation.projectName} · ${roleLabel(delegation.role)}`,
        title: "No recent agent signal",
        detail: `Last recorded signal ${updateAge(lastSignalAt)}; activity is unconfirmed, not verified stopped.`,
        createdAt: lastSignalAt,
        priority: 3,
      };
    });
  const boundHookEventIds = new Set(snapshot.delegations.flatMap((delegation) =>
    delegation.workerTelemetry?.events.map((event) => event.eventId) ?? [],
  ));
  const currentRuns = new Map(snapshot.recentRuns.filter((run) => run.status === "running").map((run) => [run.id, run]));
  const hookItems = snapshot.agentActivity
    .filter((agent) => currentRuns.has(agent.runId) && !boundHookEventIds.has(agent.id) && hookAgentState(agent, "running") === "stale")
    .map((agent) => {
      const run = currentRuns.get(agent.runId)!;
      return {
        key: `stale-hook-${agent.id}`,
        source: `${run.projectName} · unlinked observed agent`,
        title: "No recent hook activity",
        detail: `Last observed ${updateAge(agent.lastEventAt)}; role association is unknown.`,
        createdAt: agent.lastEventAt,
        priority: 3,
      };
    });
  return [...delegationItems, ...hookItems];
}

function agentRole(delegation: DelegationSnapshot) {
  return roleLabel(delegation.role);
}

function agentStateLabel(state: AgentVisualState) {
  return state === "active" ? "Working"
    : state === "waiting" ? "Waiting"
      : state === "stale" ? "No recent update"
        : state === "finished" ? "Finished"
          : state === "blocked" ? "Blocked"
            : "Status unknown";
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; value >= 1024 && index < units.length; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value >= 10 ? Math.round(value) : value.toFixed(1)} ${unit}`;
}

function memoryClassMetrics(health: DashboardSnapshot["memoryHealth"]) {
  return [
    { key: "working", label: "Working", count: health.workingCount ?? 0 },
    { key: "canonical", label: "Canonical", count: health.canonicalCount },
    { key: "episodic", label: "Episodic", count: health.episodicCount },
    { key: "evidence", label: "Evidence", count: health.evidenceCount },
    { key: "aggregate", label: "Aggregate", count: health.aggregateCount ?? 0 },
  ];
}

function MemoryUsageOverview({ health }: { health: DashboardSnapshot["memoryHealth"] }) {
  const classes = memoryClassMetrics(health);
  const temperatures = [
    { key: "hot", label: "Hot", bytes: health.hotBytes },
    { key: "warm", label: "Warm", bytes: health.warmBytes },
    { key: "cold", label: "Cold", bytes: health.coldBytes },
  ];
  const classTotal = classes.reduce((total, item) => total + item.count, 0);
  const temperatureTotal = temperatures.reduce((total, item) => total + item.bytes, 0);

  return (
    <section className="overview-panel memory-overview" aria-label="Stored memory usage">
      <div className="overview-panel-heading"><span>Memory mix</span><small>{classTotal} stored records</small></div>
      <div className="memory-metric-block">
        <div className="memory-metric-label"><span>Memory class</span><small>Record count</small></div>
        {classTotal > 0 ? <>
          <div className="memory-stack" role="img" aria-label={classes.map((item) => `${item.label}: ${item.count}`).join(", ")}>
            {classes.filter((item) => item.count > 0).map((item) => <i key={item.key} className={`memory-segment memory-${item.key}`} style={{ width: `${(item.count / classTotal) * 100}%` }} />)}
          </div>
          <ul className="memory-class-legend">
            {classes.map((item) => <li key={item.key} title={`${item.count} ${item.label.toLowerCase()} memory records`}><i className={`memory-key memory-${item.key}`} /><span>{item.label}</span><b>{item.count}</b></li>)}
          </ul>
        </> : <p className="memory-empty">No memory records stored.</p>}
      </div>
      <div className="memory-metric-block temperature-block">
        <div className="memory-metric-label"><span>Retrieval temperature</span><small>{formatBytes(temperatureTotal)} body bytes</small></div>
        {temperatureTotal > 0 ? <>
          <div className="memory-stack" role="img" aria-label={temperatures.map((item) => `${item.label}: ${formatBytes(item.bytes)}`).join(", ")}>
            {temperatures.filter((item) => item.bytes > 0).map((item) => <i key={item.key} className={`memory-segment temperature-${item.key}`} style={{ width: `${(item.bytes / temperatureTotal) * 100}%` }} />)}
          </div>
          <ul className="temperature-legend">
            {temperatures.map((item) => <li key={item.key} title={`${item.label} memory body bytes`}><i className={`memory-key temperature-${item.key}`} /><span>{item.label}</span><b>{formatBytes(item.bytes)}</b></li>)}
          </ul>
        </> : <p className="memory-empty">No memory body bytes recorded.</p>}
      </div>
      <small className="memory-footnote">Class counts include retained history; temperature measures stored body text only.</small>
    </section>
  );
}

function AgentActivityView({ snapshot }: { snapshot: DashboardSnapshot }) {
  const projectItems = projectMonitorItems(snapshot);
  const projectRoleRows = snapshot.logicalRoles
    .filter((role) => role.roleKind === "project_manager" && role.projectId !== null)
    .map((role) => {
      const project = snapshot.projects.find((candidate) => candidate.id === role.projectId);
      if (!project) return null;
      const item = projectItems.find((candidate) => candidate.project.id === project.id)!;
      const managers = snapshot.delegations
        .filter((delegation) => delegation.logicalRoleKey === role.roleKey && delegation.role === "sub_overseer")
        .sort((left, right) => {
          const activeDelta = Number(right.projectManagerAssignmentStatus === "active") - Number(left.projectManagerAssignmentStatus === "active");
          return activeDelta || right.updatedAt - left.updatedAt || right.id - left.id;
        });
      const manager = managers[0] ?? null;
      const assigned = manager?.projectManagerAssignmentStatus === "active" ? manager : null;
      const workers = manager
        ? snapshot.delegations.filter((delegation) => delegation.parentDelegationId === manager.id && delegation.role === "worker")
        : [];
      const latestUpdate = assigned ? latestAgentUpdate(snapshot, assigned.id) : null;
      const state: AgentVisualState = item.state === "blocked" || item.state === "failed"
        ? "blocked"
        : item.state === "stale" && !assigned
          ? "stale"
        : assigned
          ? agentState(assigned, snapshot.agentUpdates)
          : item.state === "working"
            ? "waiting"
            : "finished";
      const statusLabel = item.state === "blocked" || item.state === "failed" || item.state === "stale"
        ? monitorStateLabel(item.state)
        : assigned
          ? latestUpdate?.updateType === "action_required" ? "Action needed"
            : latestUpdate?.phase && state === "active" ? latestUpdate.phase.replace(/_/g, " ")
              : state === "active" ? "Working"
                : state === "blocked" ? "Blocked"
                  : state === "waiting" ? "Waiting"
                    : assigned.status === "prepared" ? "Preparing" : "Assigned"
          : item.state === "working" ? "No PM" : monitorStateLabel(item.state);
      const run = snapshot.recentRuns
        .filter((candidate) => candidate.projectName === project.name)
        .sort((left, right) => right.startedAt - left.startedAt || right.id - left.id)[0] ?? null;
      const boundHookEventIds = new Set(snapshot.delegations
        .filter((delegation) => delegation.runId === run?.id)
        .flatMap((delegation) => delegation.workerTelemetry?.events.map((event) => event.eventId) ?? []));
      const unlinkedHooks = run
        ? snapshot.agentActivity.filter((activity) => activity.runId === run.id && !boundHookEventIds.has(activity.id))
        : [];
      return { role, project, item, manager, assigned, workers, state, statusLabel, latestUpdate, run, unlinkedHooks };
    })
    .filter((row): row is NonNullable<typeof row> => row !== null)
    .sort((left, right) => left.project.name.localeCompare(right.project.name));
  const allDelegations = projectRoleRows.flatMap((row) => [
    row.assigned,
    ...row.workers.filter((worker) => worker.parentDelegationId === row.assigned?.id),
  ].filter((item): item is DelegationSnapshot => item !== null));
  const activeCount = allDelegations.filter((delegation) => agentState(delegation, snapshot.agentUpdates) === "active").length
    + projectRoleRows.reduce((total, row) => total + row.unlinkedHooks.filter((agent) => hookAgentState(agent, row.run?.status ?? "unknown") === "active").length, 0);
  const waitingCount = projectRoleRows.filter((row) => row.state === "waiting" && !row.assigned).length
    + allDelegations.filter((delegation) => agentState(delegation, snapshot.agentUpdates) === "waiting").length;
  const staleCount = allDelegations.filter((delegation) => agentState(delegation, snapshot.agentUpdates) === "stale").length
    + projectRoleRows.reduce((total, row) => total + row.unlinkedHooks.filter((agent) => hookAgentState(agent, row.run?.status ?? "unknown") === "stale").length, 0);
  const actionDelegationIds = new Set(snapshot.pendingAgentActions.map((action) => action.delegationId));
  const blockedAgentCount = allDelegations.filter((delegation) =>
    agentState(delegation, snapshot.agentUpdates) === "blocked" && !actionDelegationIds.has(delegation.id),
  ).length;
  const issueCount = projectRoleRows.reduce((total, row) => total + row.item.issueCount, 0) + blockedAgentCount;
  const ownerApprovalCount = snapshot.pendingOwnerApprovalRequests.length;
  const ownerRequestKeys = new Set(snapshot.pendingOwnerApprovalRequests.map((request) => `${request.managerDelegationId}:${request.actionKey}`));
  const mainEscalationCount = snapshot.pendingAgentActions.filter((action) => action.routedToRole === "main_overseer" && !ownerRequestKeys.has(`${action.delegationId}:${action.actionKey}`)).length;
  const queuedDispatchCount = snapshot.roleDispatches.filter((item) => item.status === "queued").length;
  const activeDispatchCount = snapshot.roleDispatches.filter((item) => ["preflighting", "thread_ready", "turn_starting", "dispatched", "running"].includes(item.status)).length;
  const dispatchAttentionCount = snapshot.roleDispatches.filter((item) => ["blocked", "failed", "needs_reconcile"].includes(item.status)).length;
  const hasRunningProject = projectRoleRows.some((row) => row.run?.status === "running") || allDelegations.some((delegation) => agentState(delegation, snapshot.agentUpdates) === "active");
  const mainInbox = snapshot.roleInbox.filter((message) => message.targetBindingKey === "main-overseer");
  const mainTurnActive = mainInbox.some((message) => ["starting", "dispatched", "running"].includes(message.status));
  const mainTurnQueued = mainInbox.some((message) => message.status === "queued");
  const mainInboxAttention = mainInbox.filter((message) => ["blocked", "unroutable", "needs_reconcile"].includes(message.status));
  const mainInboxIssues = mainInboxAttention.length;
  const actions = attentionItems(snapshot, staleAgentAttentionItems(snapshot));
  const recentUpdateCount = snapshot.recentAgentUpdateCount;
  const liveProjectCount = projectRoleRows.filter((row) => row.run?.status === "running").length;
  const [selectedNodeKey, setSelectedNodeKey] = useState<string | null>(null);
  const selectedIsMain = selectedNodeKey === "main";
  const selectedAgentId = selectedNodeKey?.startsWith("agent:") ? Number(selectedNodeKey.slice("agent:".length)) : null;
  const selectedAgent = selectedAgentId === null ? null : snapshot.delegations.find((item) => item.id === selectedAgentId) ?? null;
  const selectedProjectId = selectedNodeKey?.startsWith("project:") ? Number(selectedNodeKey.slice("project:".length)) : null;
  const selectedProjectRow = selectedProjectId === null ? null : projectRoleRows.find((row) => row.project.id === selectedProjectId) ?? null;
  const selected = selectedAgent ?? selectedProjectRow?.assigned ?? null;
  const selectedProject = selectedAgent
    ? snapshot.projects.find((project) => project.id === selectedAgent.projectId) ?? null
    : selectedProjectRow?.project ?? null;
  const selectedRun = selectedAgent
    ? snapshot.recentRuns.find((run) => run.id === selectedAgent.runId) ?? null
    : selectedProjectRow?.run ?? null;
  const selectedUpdate = selected
    ? latestAgentUpdate(snapshot, selected.id)
    : selectedProjectRow?.item.latestUpdate ?? null;
  const selectedUpdates = selected
    ? snapshot.agentUpdates.filter((update) => update.delegationId === selected.id).sort((left, right) => right.createdAt - left.createdAt).slice(0, 5)
    : selectedProjectRow
      ? snapshot.agentUpdates.filter((update) => update.projectId === selectedProjectRow.project.id && (!selectedRun || update.runId === selectedRun.id)).sort((left, right) => right.createdAt - left.createdAt).slice(0, 5)
      : [];
  const selectedAction = selectedAgent
    ? snapshot.pendingAgentActions.find((action) => action.delegationId === selectedAgent.id) ?? null
    : selectedProjectRow
      ? selectedProjectRow.item.pendingAction
      : selectedIsMain
        ? [...snapshot.pendingAgentActions]
          .filter((action) => action.routedToRole === "main_overseer")
          .sort((left, right) => right.createdAt - left.createdAt)[0] ?? null
        : null;
  const selectedOwnerApproval = selectedIsMain
    ? [...snapshot.pendingOwnerApprovalRequests].sort((left, right) => right.createdAt - left.createdAt)[0] ?? null
    : selectedAgent?.role === "sub_overseer"
      ? snapshot.pendingOwnerApprovalRequests.find((request) => request.managerDelegationId === selectedAgent.id) ?? null
      : selectedProjectRow?.assigned
        ? snapshot.pendingOwnerApprovalRequests.find((request) => request.managerDelegationId === selectedProjectRow.assigned?.id) ?? null
        : null;
  const selectedAttention = selectedIsMain
    ? selectedOwnerApproval
      ? { source: `${selectedOwnerApproval.projectName} · Owner approval`, title: selectedOwnerApproval.ownerAction, detail: `${selectedOwnerApproval.requestSummary} · ${selectedOwnerApproval.reason}`, label: "Needs you" }
      : selectedAction
        ? { source: `${selectedAction.projectName} · ${roleLabel(selectedAction.role)}`, title: selectedAction.ownerAction, detail: selectedAction.summary, label: actionRecipientLabel(selectedAction.routedToRole) }
        : actions[0] ?? null
    : selectedOwnerApproval
      ? { source: `${selectedOwnerApproval.projectName} · Owner approval`, title: selectedOwnerApproval.ownerAction, detail: `${selectedOwnerApproval.requestSummary} · ${selectedOwnerApproval.reason}`, label: "Needs you" }
      : selectedAction
      ? {
        source: `${selectedAction.projectName} · ${roleLabel(selectedAction.role)}`,
        title: selectedAction.ownerAction,
        detail: selectedAction.summary,
        label: actionRecipientLabel(selectedAction.routedToRole),
      }
      : null;
  const selectedStatus = selectedIsMain
    ? selectedOwnerApproval ? "Needs you" : selectedAction ? "Needs Main" : mainInboxIssues ? "Review inbox" : mainTurnActive ? "Reviewing update" : mainTurnQueued ? "Update queued" : "Idle"
    : selected
      ? selectedOwnerApproval ? "Needs you" : selectedAction ? actionRecipientLabel(selectedAction.routedToRole) : agentStateLabel(agentState(selected, snapshot.agentUpdates))
      : selectedProjectRow
        ? selectedOwnerApproval ? "Needs you" : selectedAction ? actionRecipientLabel(selectedAction.routedToRole) : selectedProjectRow.assigned ? agentStateLabel(agentState(selectedProjectRow.assigned, snapshot.agentUpdates)) : monitorStateLabel(selectedProjectRow.item.state)
        : "Status unavailable";
  const selectedTone = selectedIsMain
    ? selectedOwnerApproval || selectedAction || mainInboxIssues ? "blocked" : mainTurnActive ? "active" : mainTurnQueued ? "queued" : "finished"
    : selected
      ? agentState(selected, snapshot.agentUpdates)
      : selectedProjectRow
        ? selectedProjectRow.item.state === "working" ? "active"
          : selectedProjectRow.item.state === "blocked" || selectedProjectRow.item.state === "failed" ? "blocked"
              : selectedProjectRow.item.state === "stale" || selectedProjectRow.item.state === "inconclusive" ? "stale"
              : selectedProjectRow.item.state === "complete" || selectedProjectRow.item.state === "stopped" ? "finished"
                : "unknown"
        : "unknown";
  const selectedTitle = selected?.taskTitle
    ?? selectedRun?.taskTitle
    ?? selectedRun?.objectiveTitle
    ?? (selectedIsMain ? "Coordination across projects" : "No active task recorded");
  const selectedSummarySource = selectedIsMain ? "Across all projects"
    : selectedUpdate ? "Latest update"
      : selected?.report ? "Latest report"
        : selectedRun ? "Latest project run"
          : "Current status";
  const selectedSummary = selectedIsMain
    ? snapshot.pendingOwnerApprovalRequests.length > 0
      ? `${snapshot.pendingOwnerApprovalRequests.length} owner action${snapshot.pendingOwnerApprovalRequests.length === 1 ? " needs" : "s need"} your response.`
      : snapshot.pendingAgentActions.some((action) => action.routedToRole === "main_overseer")
        ? `${snapshot.pendingAgentActions.filter((action) => action.routedToRole === "main_overseer").length} Project Manager escalation${snapshot.pendingAgentActions.filter((action) => action.routedToRole === "main_overseer").length === 1 ? " needs" : "s need"} Main review.`
      : mainInboxIssues > 0
        ? `${mainInboxIssues} Main message${mainInboxIssues === 1 ? " needs" : "s need"} delivery review.`
      : mainTurnActive
        ? `Main is reviewing a routed update. Project Managers continue independently.`
      : mainTurnQueued
        ? `A project update is queued for Main. Other projects continue independently.`
      : activeCount > 0
        ? `Main is idle while ${activeCount} project agent${activeCount === 1 ? " continues" : "s continue"} work.`
      : staleCount > 0 && activeCount === 0
        ? `${staleCount} agent signal${staleCount === 1 ? " is" : "s are"} out of date; activity is unconfirmed.`
      : issueCount > 0
        ? `${issueCount} recorded attention signal${issueCount === 1 ? " needs" : "s need"} review.`
        : activeCount > 0
          ? `${activeCount} agent${activeCount === 1 ? " is" : "s are"} working across ${liveProjectCount} live project${liveProjectCount === 1 ? "" : "s"}.`
          : "No active agent work is recorded."
    : selectedUpdate?.summary
      ?? selected?.report?.report.summary
      ?? selectedAction?.summary
      ?? selectedRun?.summary
      ?? (selectedProjectRow?.item.state === "idle" ? "No project activity has been recorded yet." : "No checkpoint or report has been recorded for this work.");
  const selectedTriggerId = selectedIsMain ? "graph-main-node"
    : selectedAgent ? `graph-agent-node-${selectedAgent.id}`
      : selectedProjectRow ? `graph-project-node-${selectedProjectRow.project.id}`
        : null;
  useEffect(() => {
    if (!selectedNodeKey) return;
    const dossier = document.getElementById("graph-agent-dossier");
    dossier?.focus({ preventScroll: true });
    dossier?.scrollIntoView({
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
      block: "nearest",
    });
  }, [selectedNodeKey]);

  const closeDossier = () => {
    if (!selectedNodeKey || !selectedTriggerId) return;
    const triggerId = selectedTriggerId;
    setSelectedNodeKey(null);
    window.requestAnimationFrame(() => document.getElementById(triggerId)?.focus());
  };
  const logicalState = hasRunningProject
    ? staleCount > 0 && activeCount === 0 ? "review" : "live"
    : issueCount > 0 ? "danger" : projectRoleRows.length > 0 ? "good" : "quiet";
  const projectsWithUnlinkedHooks = projectRoleRows.filter((row) => row.unlinkedHooks.length > 0).length;
  const graphStageStyle = {
    "--graph-stage-min-height": `${Math.max(360, 121 + projectRoleRows.length * 66)}px`,
    "--graph-stage-min-height-narrow": `${Math.max(340, 97 + projectRoleRows.length * 63 + projectsWithUnlinkedHooks * 20)}px`,
  } as CSSProperties;

  return (
    <section className="agent-activity-view" aria-label="Project activity graph">
      <div className="graph-toolbar">
        <div className="graph-scope-copy">
          <i className={`run-light ${logicalState === "live" ? "is-active" : logicalState}`} aria-hidden="true" />
          <span>{projectRoleRows.length} persistent project role{projectRoleRows.length === 1 ? "" : "s"}</span>
        </div>
        <div className="graph-counts" aria-label="Current project activity counts">
          <span className="count-active" title={`${activeCount} agents with a current recorded working signal`} aria-label={`${activeCount} agents with a current recorded working signal`}><i />{activeCount}</span>
          <span className="count-waiting" title={`${waitingCount} agents waiting for a recorded next step`} aria-label={`${waitingCount} agents waiting for a recorded next step`}><i />{waitingCount}</span>
          <span className={`count-stale ${staleCount ? "has-stale" : ""}`} title={`${staleCount} agent signals older than 15 minutes; activity is unconfirmed, not verified stopped`} aria-label={`${staleCount} stale agent signals; activity is unconfirmed`}><i />{staleCount}</span>
          <span className="count-updates" title={`${recentUpdateCount} agent checkpoints recorded in the last hour`} aria-label={`${recentUpdateCount} agent checkpoints in the last hour`}><i />{recentUpdateCount}</span>
          <span className={issueCount ? "count-blocked has-issues" : "count-blocked"} title={`${issueCount} recorded action, blocker, or failed required-check signals`} aria-label={`${issueCount} recorded attention signals`}><i />{issueCount}</span>
        </div>
      </div>

      <div className="home-observability" aria-label="Workflow and memory overview">
        <section className="overview-panel workflow-overview" aria-label="Current workflow process">
          <div className="overview-panel-heading"><span>Workflow pulse</span><small>Latest recorded state</small></div>
          <div className="workflow-metrics">
            <div title="Repositories currently registered in Cathulu"><i className="metric-signal signal-neutral" /><b>{snapshot.projects.length}</b><span>Projects</span></div>
            <div title="Projects whose latest recorded run is still open"><i className="metric-signal signal-live" /><b>{liveProjectCount}</b><span>Live</span></div>
            <div title="Agents with a current recorded working signal"><i className="metric-signal signal-live" /><b>{activeCount}</b><span>Working</span></div>
            <div title="Agent checkpoints recorded in the last hour"><i className="metric-signal signal-update" /><b>{recentUpdateCount}</b><span>Updates / h</span></div>
            <div className={ownerApprovalCount ? "needs-owner" : ""} title="Explicit owner approvals or decisions required"><i className="metric-signal signal-attention" /><b>{ownerApprovalCount}</b><span>Needs you</span></div>
            <div className={mainEscalationCount ? "needs-owner" : ""} title="Project Manager escalations routed to Main Overseer"><i className="metric-signal signal-attention" /><b>{mainEscalationCount}</b><span>Needs Main</span></div>
            <div className={snapshot.failedChecks.length ? "has-failed-checks" : ""} title="Failed quality checks currently included in the dashboard snapshot"><i className="metric-signal signal-failure" /><b>{snapshot.failedChecks.length}</b><span>Failed checks</span></div>
          </div>
          <div className={`dispatch-pulse ${dispatchAttentionCount ? "has-attention" : ""}`} aria-label="Role dispatch queue state">
            <span><i className="dispatch-pulse-dot" />Role dispatch</span>
            <b>{queuedDispatchCount} queued</b>
            <b>{activeDispatchCount} active</b>
            {dispatchAttentionCount > 0 && <b className="dispatch-attention-count">{dispatchAttentionCount} needs review</b>}
          </div>
          <div className={`dispatch-pulse ${mainInboxIssues ? "has-attention" : ""}`} aria-label="Main Overseer event state">
            <span><i className="dispatch-pulse-dot" />Main</span>
            <b>{mainTurnActive ? "reviewing" : "idle"}</b>
            {mainTurnQueued && <b>{mainInbox.filter((message) => message.status === "queued").length} queued</b>}
            {mainInboxIssues > 0 && <b className="dispatch-attention-count">{mainInboxIssues} needs review</b>}
          </div>
        </section>
        <MemoryUsageOverview health={snapshot.memoryHealth} />
      </div>

      <div className={`agent-workspace ${actions.length || snapshot.agentUpdates.length || hasRunningProject ? "has-side-activity" : ""}`}>
        <aside className="graph-side-rail" aria-label="Project attention and recent activity">
          {actions.length > 0 && <section className={`graph-attention-panel ${ownerApprovalCount ? "has-owner-action" : ""}`} aria-label="Items needing attention">
            <div className="graph-attention-heading" role="status" aria-live="polite" aria-atomic="true">
              <span className="attention-beacon"><i /></span>
              <strong>{ownerApprovalCount ? `${ownerApprovalCount} OWNER ACTION${ownerApprovalCount === 1 ? "" : "S"}` : mainEscalationCount ? `${mainEscalationCount} MAIN ESCALATION${mainEscalationCount === 1 ? "" : "S"}` : "NEEDS ATTENTION"}</strong>
            </div>
            <ol>
              {actions.slice(0, 3).map((action) => <li key={action.key}>
                <span className="attention-copy"><b>{action.source} · {action.label}</b><strong>{action.title}</strong><small>{action.detail} · {updateAge(action.createdAt)}</small></span>
                <i aria-hidden="true">›</i>
              </li>)}
            </ol>
            {actions.length > 3 && <small className="attention-overflow">+{actions.length - 3} more recorded items</small>}
          </section>}

          {(snapshot.agentUpdates.length > 0 || hasRunningProject) && <section className="graph-checkpoints" aria-label="Latest agent checkpoints">
            <div className="graph-checkpoints-heading"><span>Agent activity</span><small>{recentUpdateCount} in the last hour</small></div>
            {snapshot.agentUpdates.length ? <ol>
              {snapshot.agentUpdates.slice(0, 3).map((update) => <li className={`checkpoint checkpoint-${update.updateType}`} key={update.eventId}>
                <i className="checkpoint-mark" aria-hidden="true"><b /></i>
                <span className="checkpoint-copy"><strong>{update.projectName} · {roleLabel(update.role)}{update.phase ? ` · ${update.phase}` : ""}</strong><small>{update.summary}</small></span>
                <time dateTime={new Date(update.createdAt * 1000).toISOString()}>{updateAge(update.createdAt)}</time>
              </li>)}
            </ol> : <p className="checkpoint-empty">Waiting for the first recorded agent checkpoint.</p>}
          </section>}
        </aside>

        <div className={`agent-graph-stage ${projectRoleRows.length ? "has-agents" : ""}`} style={graphStageStyle}>
        <div className="graph-role-tree">
          <button
            type="button"
            id="graph-main-node"
            className={`graph-overseer-node ${mainTurnActive ? "is-active" : ""} ${selectedIsMain ? "is-selected" : ""}`}
            onClick={() => setSelectedNodeKey(selectedIsMain ? null : "main")}
            aria-expanded={selectedIsMain}
            aria-controls={selectedIsMain ? "graph-agent-dossier" : undefined}
            title="Open Main Overseer coordination"
            aria-label={`Main Overseer. ${ownerApprovalCount ? `${ownerApprovalCount} owner actions need attention` : mainEscalationCount ? `${mainEscalationCount} Project Manager escalations need review` : mainInboxIssues ? `${mainInboxIssues} Main messages need delivery review` : mainTurnActive ? "reviewing a routed update" : mainTurnQueued ? "a routed update is queued" : "idle; project agents run independently"}. Open coordination details`}
          >
            <span className={`graph-overseer-orbit ${mainTurnActive ? "is-active" : ""}`}><BrandMark /></span>
            <span className="graph-overseer-label"><small>{ownerApprovalCount ? "OWNER ACTION" : mainEscalationCount ? "REVIEW ESCALATION" : mainInboxIssues ? "INBOX NEEDS REVIEW" : mainTurnActive ? "REVIEWING UPDATE" : mainTurnQueued ? "UPDATE QUEUED" : "IDLE · DELEGATES RUN"}</small><strong>Main Overseer</strong></span>
            <i className={`graph-status-dot state-${mainInboxIssues || ownerApprovalCount ? "danger" : mainTurnActive ? "live" : mainTurnQueued ? "review" : "quiet"}`} title={mainTurnActive ? "Main is processing a routed event" : mainTurnQueued ? "Main event is queued" : "Main is idle; project agents run independently"} />
          </button>

          {projectRoleRows.length ? <ol className="graph-project-list" aria-label="Persistent Project Manager roles">
            {projectRoleRows.map(({ role, project, item, assigned, workers, state, statusLabel, unlinkedHooks, run }) => {
              const selectedProjectNode = selectedNodeKey === `project:${project.id}`;
              return (
                <li className="graph-project-row" key={role.roleKey}>
                  <button
                    type="button"
                    id={`graph-project-node-${project.id}`}
                    className={`graph-project-node state-${state} ${selectedProjectNode ? "is-selected" : ""}`}
                    onClick={() => setSelectedNodeKey(selectedProjectNode ? null : `project:${project.id}`)}
                    aria-expanded={selectedProjectNode}
                    aria-controls={selectedProjectNode ? "graph-agent-dossier" : undefined}
                    title={`${project.name} · ${statusLabel}${assigned ? ` · ${assigned.taskTitle}` : run?.taskTitle ? ` · ${run.taskTitle}` : ""}. Open work details.`}
                    aria-label={`${project.name} Project Manager. ${statusLabel}${assigned ? `; current task ${assigned.taskTitle}` : run?.taskTitle ? `; latest task ${run.taskTitle}` : "; no task recorded"}. Open work details`}
                  >
                    <i className={`project-state-indicator state-${state}`} aria-hidden="true"><i /></i>
                    <span className="graph-project-copy"><strong>{project.name}</strong><small>{statusLabel}</small></span>
                    <i className={`graph-status-dot state-${state}`} aria-hidden="true" />
                  </button>
                  <div className="graph-worker-cluster" role="group" aria-label={`${project.name} recorded child workers`}>
                    {workers.map((worker, index) => {
                      const workerSelected = selectedNodeKey === `agent:${worker.id}`;
                      const workerState = agentState(worker, snapshot.agentUpdates);
                      return (
                        <button
                          key={worker.id}
                          type="button"
                          id={`graph-agent-node-${worker.id}`}
                          className={`graph-worker-node state-${workerState} ${workerSelected ? "is-selected" : ""}`}
                          onClick={() => setSelectedNodeKey(workerSelected ? null : `agent:${worker.id}`)}
                          aria-expanded={workerSelected}
                          aria-controls={workerSelected ? "graph-agent-dossier" : undefined}
                          title={`${project.name} · Worker ${index + 1} · ${worker.taskTitle}. Open work details.`}
                          aria-label={`${project.name} Worker ${index + 1}: ${agentStateLabel(workerState)}; ${worker.taskTitle}. Open work details`}
                        ><span>Worker {index + 1}</span><i aria-hidden="true" /></button>
                      );
                    })}
                    {!workers.length && <span className="graph-worker-empty" title="No explicit child Worker delegation is recorded" aria-label="No child Worker delegation recorded">·</span>}
                    {item.issueCount > 0 && <span className="graph-issue-count" title={`${item.issueCount} recorded action, blocker, or failed-required-check signals`} aria-label={`${item.issueCount} project attention signals`}>{item.issueCount}</span>}
                  </div>
                  {unlinkedHooks.length > 0 && <div className="graph-unlinked-cluster" role="group" aria-label={`${unlinkedHooks.length} observed agents with no recorded role link`}>
                    {unlinkedHooks.slice(0, 3).map((agent) => (
                      <span key={agent.id} className={`graph-unlinked-node state-${hookAgentState(agent, run?.status ?? "unknown")}`} title={`Observed ${agent.agentType.replace(/_/g, " ")}${agent.model ? ` · ${agent.model}` : ""} · ${agent.eventCount} hook events · no Project Manager or Worker parent is recorded`} aria-label={`Observed ${agent.agentType.replace(/_/g, " ")}; parent role unknown`}><b>?</b></span>
                    ))}
                    {unlinkedHooks.length > 3 && <span className="graph-unlinked-overflow" title={`${unlinkedHooks.length - 3} more observed agents without a recorded role link`}>+{unlinkedHooks.length - 3}</span>}
                  </div>}
                </li>
              );
            })}
          </ol> : <div className="graph-empty-state"><span className="graph-empty-orbit"><BrandMark /></span><small>No registered project roles</small></div>}
        </div>
        </div>
      </div>
      {selectedNodeKey && <section id="graph-agent-dossier" className="worker-dossier" tabIndex={-1} aria-label={`${selectedIsMain ? "Main Overseer" : selectedAgent ? agentRole(selectedAgent) : "Project Manager"} work details`}>
        <header className="worker-dossier-heading">
          <div className="dossier-identity">
            <i className={`detail-state state-${selectedTone}`} aria-hidden="true" />
            <span>
              <small>{selectedProject?.name ?? "All projects"} · {selectedIsMain ? "Main Overseer" : selectedAgent ? agentRole(selectedAgent) : "Project Manager"}{selectedRun ? ` · Run ${selectedRun.id}` : ""}</small>
              <h2>{selectedTitle}</h2>
            </span>
          </div>
          <div className="dossier-heading-actions">
            <span className={`dossier-status state-${selectedTone}`}>{selectedStatus}</span>
            <button type="button" className="dossier-close" onClick={closeDossier} aria-label="Close work details" title="Close work details"><Icon name="close" /></button>
          </div>
        </header>
        <div className="dossier-summary">
          <div className="dossier-summary-heading">
            <span>{selectedSummarySource}</span>
            {selectedUpdate && <time dateTime={new Date(selectedUpdate.createdAt * 1000).toISOString()}>Updated {updateAge(selectedUpdate.createdAt)}</time>}
          </div>
          <p className="dossier-summary-copy">{selectedSummary}</p>
          {selectedAttention && <aside className="dossier-owner-action" role="status">
            <strong>{selectedAttention.label}</strong>
            <p>{selectedAttention.title}</p>
            <small>{selectedAttention.source}{selectedAttention.detail ? ` · ${selectedAttention.detail}` : ""}</small>
            {selectedOwnerApproval && <OwnerApprovalControls request={selectedOwnerApproval} />}
          </aside>}
          {(selectedIsMain || selectedRun || selected?.report || selectedUpdate || selectedProjectRow) && <div className="dossier-glance" aria-label="Work summary metrics">
            {selectedIsMain ? <>
              <div><strong>{mainTurnActive ? "Working" : mainTurnQueued ? "Queued" : "Idle"}</strong><span>Main turn</span></div>
              <div><strong>{liveProjectCount}</strong><span>Live projects</span></div>
              <div><strong>{activeCount}</strong><span>Agents working</span></div>
              <div className={issueCount ? "needs-action" : ""}><strong>{issueCount}</strong><span>Attention</span></div>
            </> : <>
              {selectedRun && <div><strong>{selectedRun.status === "running" ? "In progress" : selectedRun.status === "verified_success" ? "Verified" : selectedRun.status.replace(/_/g, " ")}</strong><span>Project run</span></div>}
              {selected?.report && <>
                <div><strong>{selected.report.report.checkResults.length}</strong><span>Checks reported</span></div>
                <div><strong>{selected.report.report.changedFiles.length}</strong><span>Files reported</span></div>
              </>}
              {!selected?.report && selectedUpdate && <div><strong>{updateAge(selectedUpdate.createdAt)}</strong><span>Last update</span></div>}
              {!selected?.report && selectedProjectRow && <div><strong>{selectedProjectRow.workers.length}</strong><span>Recorded workers</span></div>}
            </>}
          </div>}
        </div>
        {(selectedIsMain ? actions.length > 0 || mainInbox.length > 0 : selectedRun || selectedUpdates.length > 0 || selected?.report || selected?.worktree || selected?.workerTelemetry || Boolean(selectedAction)) && <details className="dossier-evidence">
          <summary><span>Full evidence</span><small>{selectedIsMain ? "Recent attention items" : "Reports, updates, Git and QA details"}</small></summary>
          <div className="worker-dossier-grid">
            {selectedIsMain && actions.length > 0 && <section className="dossier-section">
              <h3>Latest attention</h3>
              <ul className="dossier-check-list">{actions.map((action) => <li key={action.key}>
                <i className={`check-signal ${action.priority === 0 ? "status-failed" : "status-unknown"}`} />
                <span><b>{action.source} · {action.title}</b><small>{action.detail}</small></span>
                <strong>{action.label}</strong>
              </li>)}</ul>
            </section>}
            {selectedIsMain && mainInbox.length > 0 && <section className="dossier-section dossier-checkpoints">
              <h3>Main event trail <span>{mainInbox.length}</span></h3>
              <ol>{[...mainInbox].sort((left, right) => right.updatedAt - left.updatedAt || right.id - left.id).slice(0, 6).map((message) => <li key={message.id}>
                <i className={`checkpoint-mark checkpoint-${message.status}`} aria-hidden="true"><b /></i>
                <span><strong>{message.projectName} · {message.eventType.replace(/_/g, " ")}</strong><small>{message.status.replace(/_/g, " ")}{message.lastError ? ` · ${message.lastError}` : ""}</small></span>
                <time dateTime={new Date(message.updatedAt * 1000).toISOString()}>{updateAge(message.updatedAt)}</time>
              </li>)}</ol>
            </section>}
            {selectedRun && <section className="dossier-section">
              <h3>Project run</h3>
              <div className="dossier-fact-row"><span>Result</span><b>{selectedRun.status.replace(/_/g, " ")}</b></div>
              {selectedRun.workflowKey && <div className="dossier-fact-row"><span>Workflow</span><b>{selectedRun.workflowKey.replace(/_/g, " ")}{selectedRun.workflowVersion ? ` · v${selectedRun.workflowVersion}` : ""}</b></div>}
              {selectedRun.summary && <p>{selectedRun.summary}</p>}
            </section>}
            {selectedUpdates.length > 0 && <section className="dossier-section dossier-checkpoints">
              <h3>Recent updates <span>{selectedUpdates.length}</span></h3>
              <ol>{selectedUpdates.map((update) => <li key={update.eventId}>
                <i className={`checkpoint-mark checkpoint-${update.updateType}`} aria-hidden="true"><b /></i>
                <span><strong>{update.phase?.replace(/_/g, " ") ?? update.updateType.replace(/_/g, " ")}</strong><small>{update.summary}</small></span>
                <time dateTime={new Date(update.createdAt * 1000).toISOString()}>{updateAge(update.createdAt)}</time>
              </li>)}</ol>
            </section>}
            {selected?.workerTelemetry && !selected.report && <section className="dossier-section">
              <h3>Recorded agent activity</h3>
              <div className="dossier-fact-row"><span>Hook events</span><b>{selected.workerTelemetry.events.length}</b></div>
              {selected.workerTelemetry.effectiveModel && <div className="dossier-fact-row"><span>Observed model</span><b>{selected.workerTelemetry.effectiveModel}</b></div>}
              {selected.workerTelemetry.events.length > 0 && <ol className="dossier-telemetry-list">{selected.workerTelemetry.events.map((event) => <li key={event.eventId}><span>{event.eventKind.replace(/_/g, " ")}</span><time dateTime={new Date(event.createdAt * 1000).toISOString()}>{updateAge(event.createdAt)}</time></li>)}</ol>}
            </section>}
            {selected?.report && <section className="dossier-section dossier-report">
              <h3>{agentRole(selected)} report <span>reported by agent</span></h3>
              <p>{selected.report.report.summary}</p>
              <div className="dossier-outcome"><span>Reported outcome</span><strong>{selected.report.report.outcome.replace(/_/g, " ")}</strong></div>
              <div className="dossier-subsection"><h4>Reported changed files · {selected.report.report.changedFiles.length}</h4>
                {selected.report.report.changedFiles.length ? <ul className="dossier-path-list">{selected.report.report.changedFiles.map((path) => <li key={path} title={path}>{path}</li>)}</ul> : <p className="dossier-empty">No files were reported as changed.</p>}
              </div>
              <div className="dossier-subsection"><h4>Reported checks · {selected.report.report.checkResults.length}</h4>
                {selected.report.report.checkResults.length ? <ul className="dossier-check-list">{selected.report.report.checkResults.map((check, index) => <li key={`${check.name}-${index}`}><i className={`check-signal status-${check.status.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`} /><span><b>{check.name}</b><small>{check.summary}</small></span><strong>{check.status}</strong></li>)}</ul> : <p className="dossier-empty">No checks were included in this report.</p>}
              </div>
              {selected.report.report.blockers.length > 0 && <div className="dossier-subsection dossier-blockers"><h4>Reported blockers</h4><ul>{selected.report.report.blockers.map((blocker, index) => <li key={`${blocker}-${index}`}>{blocker}</li>)}</ul></div>}
              {selected.report.report.commits.length > 0 && <div className="dossier-commits"><span>Reported commits</span>{selected.report.report.commits.map((commit) => <code key={commit}>{commit}</code>)}</div>}
              {selected.workerTelemetry && <small className="dossier-source-note">Hook observed {selected.workerTelemetry.events.length} event{selected.workerTelemetry.events.length === 1 ? "" : "s"}{selected.workerTelemetry.effectiveModel ? ` · ${selected.workerTelemetry.effectiveModel}` : ""}</small>}
            </section>}
            {selected?.worktree && <section className="dossier-section dossier-worktree">
              <h3>Git and worktree</h3>
              <div className="dossier-fact-row"><span>Branch</span><b>{selected.worktree.branch ?? "Unknown"}</b></div>
              <div className="dossier-fact-row"><span>Observed HEAD</span><b title={selected.worktree.observedHead ?? "No Git HEAD recorded"}>{selected.worktree.observedHead?.slice(0, 12) ?? "Not recorded"}</b></div>
              <div className="dossier-fact-row"><span>Observed state</span><b>{selected.worktree.isClean === null ? "Not available" : selected.worktree.isClean ? "Clean" : "Changes present"}</b></div>
              <div className="dossier-fact-row"><span>Reconciled</span><b>{selected.worktree.lastReconciledAt ? updateAge(selected.worktree.lastReconciledAt) : "Not reconciled"}</b></div>
              {selected.worktree.reportVerification && <div className="dossier-verification"><b>{selected.worktree.reportVerification.status.replace(/_/g, " ")}</b><span>{selected.worktree.reportVerification.verifiedPaths.length} verified paths · {selected.worktree.reportVerification.unverifiedPaths.length} unverified</span></div>}
              {selected.worktree.changedPaths.length > 0 && <div className="dossier-subsection"><h4>Observed changed paths · {selected.worktree.changedPaths.length}</h4><ul className="dossier-path-list">{selected.worktree.changedPaths.map((path) => <li key={path} title={path}>{path}</li>)}</ul></div>}
            </section>}
            {selected?.report?.qualityCheckRefs.length ? <section className="dossier-section">
              <h3>Cathulu QA references</h3>
              <ul className="dossier-check-list">{selected.report.qualityCheckRefs.map((check) => <li key={check.qualityCheckId}><i className={`check-signal status-${check.status.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`} /><span><b>{check.checkKey}</b><small>{check.mandatory ? "Required" : "Optional"} · recorded by Cathulu</small></span><strong>{check.status}</strong></li>)}</ul>
            </section> : null}
          </div>
        </details>}
      </section>}
    </section>
  );
}

function OwnerApprovalControls({ request }: { request: PendingOwnerApproval }) {
  const [armedDecision, setArmedDecision] = useState<"approved" | "declined" | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setArmedDecision(null);
    setSubmitting(false);
    setError(null);
  }, [request.id, request.requestHash]);

  const decide = async (decision: "approved" | "declined") => {
    if (armedDecision !== decision) {
      setArmedDecision(decision);
      setError(null);
      return;
    }
    if (submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      await invoke("decide_owner_approval_request", {
        requestId: request.id,
        expectedRequestHash: request.requestHash,
        decision,
      });
      setArmedDecision(null);
      window.dispatchEvent(new Event("cathulu:refresh-dashboard"));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="owner-decision-controls" aria-label="Owner approval decision">
      {armedDecision ? <>
        <p className="owner-decision-confirmation" role="status">
          {armedDecision === "approved" ? "Approve" : "Decline"} only this exact request? Cathulu will record your decision; it will not perform the action.
        </p>
        <div className="owner-decision-buttons">
          <button className={`owner-decision-button is-${armedDecision}`} type="button" disabled={submitting} onClick={() => void decide(armedDecision)}>
            {submitting ? "Recording…" : `Confirm ${armedDecision === "approved" ? "approval" : "decline"}`}
          </button>
          <button className="owner-decision-cancel" type="button" disabled={submitting} onClick={() => setArmedDecision(null)}>Cancel</button>
        </div>
      </> : <div className="owner-decision-buttons">
        <button className="owner-decision-button is-approved" type="button" onClick={() => void decide("approved")}>Approve exact action</button>
        <button className="owner-decision-button is-declined" type="button" onClick={() => void decide("declined")}>Decline</button>
      </div>}
      {error && <p className="owner-decision-error" role="alert">Decision not recorded: {error}</p>}
      <small className="owner-decision-note">Approval applies only to the action shown above. Review and execute it separately.</small>
    </div>
  );
}

function CompactProjectSignals({
  items,
  loading,
  selectedProjectId,
  onSelect,
}: {
  items: ProjectMonitorItem[];
  loading: boolean;
  selectedProjectId: number | null;
  onSelect: (projectId: number) => void;
}) {
  const stableItems = [...items].sort((left, right) => left.project.id - right.project.id);
  return (
    <span className="monitor-project-signals" role="group" title={loading ? "Reading project activity" : `${stableItems.length} project statuses`} aria-label={loading ? "Reading project activity" : `${stableItems.length} project statuses`}>
      {stableItems.map((item) => <button
        key={item.project.id}
        className={`project-signal-bubble state-${item.state} ${selectedProjectId === item.project.id ? "is-selected" : ""}`}
        type="button"
        onMouseDown={(event) => event.stopPropagation()}
        onClick={() => onSelect(item.project.id)}
        aria-label={`${item.project.name}: ${projectMonitorLabel(item)}. Show project details`}
        aria-expanded={selectedProjectId === item.project.id}
        aria-controls={`monitor-project-details-${item.project.id}`}
        title={`${item.project.name} · ${monitorStateLabel(item.state)}`}
      />)}
    </span>
  );
}

function MonitorWindow({ snapshot, loading, error }: { snapshot: DashboardSnapshot | null; loading: boolean; error: string | null }) {
  const [expanded, setExpanded] = useState(false);
  const [pinned, setPinned] = useState(true);
  const [actionError, setActionError] = useState<string | null>(null);
  const [expandedProjectId, setExpandedProjectId] = useState<number | null>(null);
  const projectItems = snapshot ? projectMonitorItems(snapshot) : [];
  const liveProjectCount = projectItems.filter((item) => item.latestRun?.status === "running").length;
  const workingAgentCount = projectItems.reduce((total, item) => total + item.activeAgents, 0);
  const ownerActionCount = snapshot?.pendingOwnerApprovalRequests.length ?? 0;
  const queuedDispatchCount = snapshot?.roleDispatches.filter((item) => item.status === "queued").length ?? 0;
  const activeDispatchCount = snapshot?.roleDispatches.filter((item) => ["preflighting", "thread_ready", "turn_starting", "dispatched", "running"].includes(item.status)).length ?? 0;
  const memoryClasses = snapshot ? memoryClassMetrics(snapshot.memoryHealth) : [];
  const memoryClassTotal = memoryClasses.reduce((total, item) => total + item.count, 0);
  const compactWidth = 174;
  const compactHeight = 58;
  const expandedWidth = 376;
  const expandedHeight = 360;

  const togglePinned = async () => {
    try {
      await getCurrentWindow().setAlwaysOnTop(!pinned);
      setPinned(!pinned);
      setActionError(null);
    } catch (reason) {
      setActionError(reason instanceof Error ? reason.message : "Could not update the floating window pin.");
    }
  };

  const resizeMonitor = async (nextExpanded: boolean, projectId: number | null = null) => {
    if (nextExpanded === expanded) {
      if (projectId !== null) setExpandedProjectId(projectId);
      return;
    }
    try {
      const window = getCurrentWindow();
      const nextWidth = nextExpanded ? expandedWidth : compactWidth;
      const nextHeight = nextExpanded ? expandedHeight : compactHeight;
      const display = await currentMonitor();
      if (display) {
        const origin = display.position.toLogical(display.scaleFactor);
        const bounds = display.size.toLogical(display.scaleFactor);
        const current = (await window.outerPosition()).toLogical(display.scaleFactor);
        const currentWidth = expanded ? expandedWidth : compactWidth;
        const currentHeight = expanded ? expandedHeight : compactHeight;
        const maxX = Math.max(origin.x + 8, origin.x + bounds.width - nextWidth - 8);
        const maxY = Math.max(origin.y + 8, origin.y + bounds.height - nextHeight - 8);
        const x = Math.min(Math.max(current.x + currentWidth - nextWidth, origin.x + 8), maxX);
        const y = Math.min(Math.max(current.y + currentHeight - nextHeight, origin.y + 8), maxY);
        await window.setPosition(new LogicalPosition(x, y));
      }
      await window.setSize(new LogicalSize(nextWidth, nextHeight));
      setExpanded(nextExpanded);
      setExpandedProjectId(nextExpanded ? projectId : null);
    } catch (reason) {
      setActionError(reason instanceof Error ? reason.message : "Could not resize the floating monitor.");
    }
  };

  const toggleExpanded = () => { void resizeMonitor(!expanded); };
  const selectProject = (projectId: number) => { void resizeMonitor(true, projectId); };
  const toggleProjectDetails = (projectId: number) => {
    setExpandedProjectId((current) => current === projectId ? null : projectId);
  };

  const restoreDashboard = async () => {
    try {
      const dashboard = await WebviewWindow.getByLabel("main");
      if (!dashboard) {
        setActionError("The main dashboard is not available.");
        return;
      }
      await dashboard.unminimize();
      await dashboard.show();
      await dashboard.setFocus();
      setActionError(null);
    } catch (reason) {
      setActionError(reason instanceof Error ? reason.message : "Could not focus the dashboard window.");
    }
  };

  const dragWindow = () => { void getCurrentWindow().startDragging().catch(() => setActionError("Window dragging is unavailable.")); };
  const closeWindow = () => { void getCurrentWindow().close(); };

  return (
    <main className={`monitor-root ${expanded ? "is-expanded" : "is-compact"}`}>
      <section className="monitor-glass" aria-label="Cathulu floating monitor">
        <header className="monitor-header" onMouseDown={dragWindow}>
          <span className="monitor-brand" title="Cathulu"><BrandMark /></span>
          <CompactProjectSignals items={projectItems} loading={loading} selectedProjectId={expandedProjectId} onSelect={selectProject} />
          <div className="monitor-actions">
            <button className={pinned ? "monitor-icon-button is-pinned" : "monitor-icon-button"} type="button" onMouseDown={(event) => event.stopPropagation()} onClick={() => void togglePinned()} aria-label={pinned ? "Turn always on top off" : "Keep monitor always on top"} title={pinned ? "Always on top" : "Pin above other windows"}><Icon name="pin" /></button>
            <button className="monitor-icon-button" type="button" onMouseDown={(event) => event.stopPropagation()} onClick={() => void toggleExpanded()} aria-label={expanded ? "Collapse monitor" : "Expand monitor"} title={expanded ? "Collapse" : "Expand"}><Icon name={expanded ? "collapse" : "expand"} /></button>
            <button className="monitor-icon-button" type="button" onMouseDown={(event) => event.stopPropagation()} onClick={closeWindow} aria-label="Close floating monitor" title="Close"><Icon name="close" /></button>
          </div>
        </header>
        <div className="monitor-project-board" aria-label="Project activity" hidden={!expanded}>
            <div className="monitor-board-heading">
              <span>Projects</span>
              <button className="monitor-open" type="button" onMouseDown={(event) => event.stopPropagation()} onClick={() => void restoreDashboard()} aria-label="Open dashboard" title="Open dashboard"><Icon name="open" /></button>
            </div>
            {snapshot && <section className="monitor-pulse" aria-label="Workflow pulse">
              <div className="monitor-pulse-item" title="Projects with a latest run still recorded as running"><i className="signal-live" /><b>{liveProjectCount}</b><span>Live</span></div>
              <div className="monitor-pulse-item" title="Agents with a current recorded working signal"><i className="signal-live" /><b>{workingAgentCount}</b><span>Working</span></div>
              <div className="monitor-pulse-item" title="Prepared role assignments accepted into Cathulu's asynchronous queue"><i className="signal-update" /><b>{queuedDispatchCount}</b><span>Queued</span></div>
              <div className="monitor-pulse-item" title="Role turns acknowledged by Codex and not yet returned"><i className="signal-live" /><b>{activeDispatchCount}</b><span>Dispatching</span></div>
              <div className={`monitor-pulse-item ${ownerActionCount ? "needs-owner" : ""}`} title="Open, explicitly recorded requests for owner input"><i className="signal-attention" /><b>{ownerActionCount}</b><span>Needs you</span></div>
            </section>}
            {snapshot && <section className="monitor-memory" aria-label="Memory usage by class">
              <div className="monitor-memory-heading"><span>Memory mix</span><small>{memoryClassTotal} records</small></div>
              {memoryClassTotal > 0 ? <>
                <div className="memory-stack monitor-memory-stack" role="img" aria-label={memoryClasses.map((item) => `${item.label}: ${item.count}`).join(", ")}>
                  {memoryClasses.filter((item) => item.count > 0).map((item) => <i key={item.key} className={`memory-segment memory-${item.key}`} style={{ width: `${(item.count / memoryClassTotal) * 100}%` }} />)}
                </div>
                <ul className="monitor-memory-legend">
                  {memoryClasses.map((item) => <li key={item.key} title={`${item.count} ${item.label.toLowerCase()} memory records`}><i className={`memory-key memory-${item.key}`} /><span>{item.label}</span><b>{item.count}</b></li>)}
                </ul>
              </> : <p className="monitor-memory-empty">No memory records</p>}
            </section>}
            {!snapshot ? <p className="monitor-error">{error ?? (loading ? "Reading local state…" : "No snapshot available")}</p> : projectItems.length === 0 ? <p className="monitor-empty">No registered projects</p> : projectItems.map((item) => (
              <div className="monitor-project-entry" key={item.project.id}>
                <button
                  className={`monitor-project-row state-${item.state} ${item.attentionCount ? "has-attention" : ""} ${item.staleAgentCount ? "has-stale" : ""}`}
                  type="button"
                  onMouseDown={(event) => event.stopPropagation()}
                  onClick={() => toggleProjectDetails(item.project.id)}
                  aria-expanded={expandedProjectId === item.project.id}
                  aria-controls={`monitor-project-details-${item.project.id}`}
                  aria-label={`${item.project.name}: ${projectMonitorLabel(item)}. ${item.detail}. ${expandedProjectId === item.project.id ? "Hide" : "Show"} project details`}
                >
                  <span className={`project-state-indicator state-${item.state}`} aria-hidden="true"><i />{item.state === "working" && <span className="project-activity-wave"><i /><i /><i /></span>}</span>
                  <span className="monitor-project-copy"><strong>{item.project.name}</strong><small>{item.detail}</small></span>
                  <span className="monitor-project-meta">
                    {item.issueCount > 0 && <b className={`project-issue-count ${item.attentionCount ? "is-attention" : ""} ${item.staleAgentCount ? "is-stale" : ""}`} title={`${item.attentionCount} owner actions; ${item.staleAgentCount} stale agent signals; ${item.issueCount} total attention signals`}>{item.issueCount}</b>}
                    {item.state === "working" && item.activeAgents > 0 && <b className="project-agent-count" title={`${item.activeAgents} active agents`}>{item.activeAgents}</b>}
                    <span className={`project-state-label state-${item.state}`}>{projectMonitorLabel(item)}</span>
                    <i className={`monitor-disclosure-chevron ${expandedProjectId === item.project.id ? "is-open" : ""}`} aria-hidden="true" />
                  </span>
                </button>
                <div className="monitor-project-details" id={`monitor-project-details-${item.project.id}`} role="region" aria-label={`${item.project.name} latest activity`} hidden={expandedProjectId !== item.project.id}>
                  {item.pendingOwnerApproval && <section className="monitor-detail-attention">
                    <small>Needs you</small>
                    <strong>{item.pendingOwnerApproval.ownerAction}</strong>
                    <p>{item.pendingOwnerApproval.requestSummary} · {item.pendingOwnerApproval.reason}</p>
                  </section>}
                  {item.pendingAction && <section className="monitor-detail-attention">
                    <small>{actionRecipientLabel(item.pendingAction.routedToRole)}</small>
                    <strong>{item.pendingAction.ownerAction}</strong>
                    <p>{item.pendingAction.summary}</p>
                  </section>}
                  {item.latestUpdate ? <section className="monitor-detail-checkpoint">
                    <div><small>{item.latestUpdate.phase?.replace(/_/g, " ") ?? "Latest checkpoint"}</small><time dateTime={new Date(item.latestUpdate.createdAt * 1000).toISOString()}>{updateAge(item.latestUpdate.createdAt)}</time></div>
                    <p>{item.latestUpdate.summary}</p>
                  </section> : item.latestRun?.summary ? <section className="monitor-detail-checkpoint">
                    <div><small>Latest run summary</small><span className="monitor-detail-state">{projectMonitorLabel(item)}</span></div>
                    <p>{item.latestRun.summary}</p>
                  </section> : <p className="monitor-detail-empty">No checkpoint recorded for this project.</p>}
                  {item.failedCheckKeys.length > 0 && <p className="monitor-detail-failure"><strong>Required check failed</strong><span>{item.failedCheckKeys.slice(0, 2).join(" · ")}{item.failedCheckKeys.length > 2 ? ` · +${item.failedCheckKeys.length - 2}` : ""}</span></p>}
                  {item.latestRun && <small className="monitor-detail-source">Run {item.latestRun.id}{item.activeAgents > 0 ? ` · ${item.activeAgents} active agent${item.activeAgents === 1 ? "" : "s"}` : ""}</small>}
                </div>
              </div>
            ))}
        </div>
        {actionError && <p className="monitor-error" role="status">{actionError}</p>}
      </section>
    </main>
  );
}
