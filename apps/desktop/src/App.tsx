import { invoke, isTauri } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { currentMonitor, getCurrentWindow, LogicalPosition, LogicalSize } from "@tauri-apps/api/window";
import { type ReactNode, useEffect, useState } from "react";
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
  qualityCheckRefs: { qualityCheckId: number; checkKey: string; status: string }[];
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
  actionKey: string;
  summary: string;
  ownerAction: string;
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
  recentAgentUpdateCount: number;
  delegations: DelegationSnapshot[];
  failedChecks: { id: number; runId: number; projectName: string; checkKey: string; mandatory: boolean; status: string; details: unknown; updatedAt: number }[];
  memoryHealth: {
    hotBytes: number; hotScopeCount: number; largestScopeHotBytes: number;
    warmBytes: number; coldBytes: number; canonicalCount: number;
    episodicCount: number; evidenceCount: number; candidateCount: number;
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
type ProjectMonitorState = "working" | "stale" | "blocked" | "failed" | "stopped" | "idle";
type ProjectMonitorItem = {
  project: ProjectSnapshot;
  state: ProjectMonitorState;
  detail: string;
  activeAgents: number;
  issueCount: number;
  attentionCount: number;
  staleAgentCount: number;
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
  const priority: Record<ProjectMonitorState, number> = { working: 0, stale: 1, blocked: 2, failed: 3, stopped: 4, idle: 5 };
  return snapshot.projects.map((project) => {
    const latestRun = snapshot.recentRuns
      .filter((run) => run.projectName === project.name)
      .sort((left, right) => right.startedAt - left.startedAt || right.id - left.id)[0] ?? null;
    const blockers = snapshot.blockers.filter((blocker) => blocker.projectName === project.name);
    const mandatoryFailures = latestRun
      ? snapshot.failedChecks.filter((check) => check.runId === latestRun.id && check.mandatory && check.status.toLowerCase() === "failed")
      : [];
    const pendingActions = snapshot.pendingAgentActions.filter((action) => action.projectId === project.id);
    const delegations = latestRun ? snapshot.delegations.filter((item) => item.runId === latestRun.id) : [];
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
    const state: ProjectMonitorState = pendingActions.length > 0
      ? "blocked"
      : latestRun?.status === "running" && staleAgentCount > 0 && activeAgents === 0
        ? "stale"
      : latestRun?.status === "running"
      ? "working"
      : latestRun?.status === "blocked" || (!latestRun && blockers.length > 0)
        ? "blocked"
        : latestRun?.status === "failed" || mandatoryFailures.length > 0
          ? "failed"
          : latestRun
            ? "stopped"
            : "idle";
    const detail = pendingActions[0]?.ownerAction
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
      issueCount: blockers.length + mandatoryFailures.length + pendingActions.length + staleAgentCount,
      attentionCount: pendingActions.length,
      staleAgentCount,
    };
  }).sort((left, right) => {
    return priority[left.state] - priority[right.state] || left.project.name.localeCompare(right.project.name);
  });
}

function monitorStateLabel(state: ProjectMonitorState) {
  return state === "working" ? "Working" : state === "stale" ? "No fresh signal" : state === "blocked" ? "Blocked" : state === "failed" ? "Failed" : state === "stopped" ? "Stopped" : "Idle";
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
  const actions = snapshot.pendingAgentActions.map((action) => ({
    key: `agent-${action.eventId}`,
    source: `${action.projectName} · ${roleLabel(action.role)}`,
    title: action.ownerAction,
    detail: action.summary,
    createdAt: action.createdAt,
    priority: 0,
  }));
  const blockedTasks = snapshot.blockers.map((blocker) => ({
    key: `task-${blocker.id}`,
    source: blocker.projectName,
    title: blocker.title,
    detail: "Blocked task recorded in Cathulu",
    createdAt: blocker.updatedAt,
    priority: 1,
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
    }));
  return [...actions, ...blockedTasks, ...failedChecks, ...staleAgents]
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
    return () => {
      cancelled = true;
      window.clearInterval(poll);
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
      const workers = assigned
        ? snapshot.delegations.filter((delegation) => delegation.parentDelegationId === assigned.id && delegation.role === "worker")
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
  const mainRole = snapshot.logicalRoles.find((role) => role.roleKind === "main_overseer");
  const allDelegations = projectRoleRows.flatMap((row) => [row.assigned, ...row.workers].filter((item): item is DelegationSnapshot => item !== null));
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
  const hasRunningProject = projectRoleRows.some((row) => row.run?.status === "running") || allDelegations.some((delegation) => agentState(delegation, snapshot.agentUpdates) === "active");
  const actions = attentionItems(snapshot, staleAgentAttentionItems(snapshot));
  const recentUpdateCount = snapshot.recentAgentUpdateCount;
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const selected = allDelegations.find((item) => item.id === Number(selectedAgentId)) ?? null;
  const selectedUpdate = selected ? latestAgentUpdate(snapshot, selected.id) : null;
  const logicalState = hasRunningProject
    ? staleCount > 0 && activeCount === 0 ? "review" : "live"
    : issueCount > 0 ? "danger" : projectRoleRows.length > 0 ? "good" : "quiet";

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

      {actions.length > 0 && <aside className={`graph-attention-panel ${snapshot.pendingAgentActions.length ? "has-owner-action" : ""}`} aria-label="Items needing attention">
        <div className="graph-attention-heading" role="status" aria-live="polite" aria-atomic="true">
          <span className="attention-beacon"><i /></span>
          <strong>{snapshot.pendingAgentActions.length ? `${snapshot.pendingAgentActions.length} OWNER ACTION${snapshot.pendingAgentActions.length === 1 ? "" : "S"}` : "NEEDS ATTENTION"}</strong>
        </div>
        <ol>
          {actions.slice(0, 3).map((action) => <li key={action.key}>
            <span className="attention-copy"><b>{action.source}</b><strong>{action.title}</strong><small>{action.detail} · {updateAge(action.createdAt)}</small></span>
            <i aria-hidden="true">›</i>
          </li>)}
        </ol>
        {actions.length > 3 && <small className="attention-overflow">+{actions.length - 3} more recorded items</small>}
      </aside>}

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

      <div className={`agent-graph-stage ${projectRoleRows.length ? "has-agents" : ""} ${projectRoleRows.length > 5 ? "is-dense" : ""}`}>
        <div className="graph-role-tree">
          <div className={`graph-overseer-node ${hasRunningProject ? "is-active" : ""}`} role="group" title="Persistent Main Overseer role coordinates project work; Codex Project Managers and Workers perform repository changes" aria-label={`One persistent logical Main Overseer role${hasRunningProject ? ", coordinating active project managers" : ""}`}>
            <span className={`graph-overseer-orbit ${hasRunningProject ? "is-active" : ""}`}><BrandMark /></span>
            <span className="graph-overseer-label"><small>{snapshot.pendingAgentActions.length ? "ACTION NEEDED" : hasRunningProject ? staleCount > 0 && activeCount === 0 ? "CHECK AGENT SIGNAL" : "COORDINATING" : "STANDING BY"}</small><strong>Main Overseer</strong></span>
            <span className="role-indicators">
              <i className="role-indicator is-configured" aria-label={`Main context scope: ${mainRole?.memoryScopeKey ?? "global"}`} title={`Shared context · ${mainRole?.memoryScopeKey ?? "global"}`}><b>C</b></i>
              <i className="role-indicator is-configured" aria-label={`Main tool profile: ${mainRole?.toolProfileKey ?? "main-overseer-v1"}; actual chat tools are not observable`} title={`Tool profile · ${mainRole?.toolProfileKey ?? "main-overseer-v1"}`}><b>T</b></i>
            </span>
            <i className={`graph-status-dot state-${logicalState}`} title={hasRunningProject ? "Recorded project activity" : issueCount > 0 ? "Recorded blocker or failed check" : "Idle"} />
          </div>

          {projectRoleRows.length ? <ol className="graph-project-list" aria-label="Persistent Project Manager roles">
            {projectRoleRows.map(({ role, project, item, assigned, workers, state, statusLabel, latestUpdate, unlinkedHooks, run }) => {
              const roleTitle = `${project.name} · persistent Project Manager role · ${statusLabel}${assigned ? ` · ${assigned.taskTitle}` : ""}`;
              const updateTitle = latestUpdate ? `\nLast agent checkpoint (${updateAge(latestUpdate.createdAt)}): ${latestUpdate.summary}` : "\nNo agent checkpoint has been recorded for this assignment.";
              const selected = assigned && selectedAgentId === String(assigned.id);
              return (
                <li className="graph-project-row" key={role.roleKey}>
                  <button
                    type="button"
                    className={`graph-project-node state-${state} ${selected ? "is-selected" : ""}`}
                    onClick={() => assigned && setSelectedAgentId(selected ? null : String(assigned.id))}
                    title={`${roleTitle}${updateTitle}\nContext scope: ${role.memoryScopeKey}\nTool profile: ${role.toolProfileKey}; actual Codex chat availability is unobserved.`}
                    aria-label={`${roleTitle}${latestUpdate ? `; latest agent checkpoint ${latestUpdate.summary}, ${updateAge(latestUpdate.createdAt)}` : "; no agent checkpoint recorded"}; context scope ${role.memoryScopeKey}; tool profile ${role.toolProfileKey}; actual chat tool availability is unobserved`}
                  >
                    <i className={`project-state-indicator state-${state}`} aria-hidden="true"><i /></i>
                    <span className="graph-project-copy"><strong>{project.name}</strong><small>{statusLabel}</small></span>
                    <span className="role-indicators">
                      <i className={`role-indicator ${assigned?.activationStatus === "ready" ? "is-ready" : assigned?.activationStatus === "incomplete" ? "is-warning" : "is-configured"}`} aria-label={`Persistent project context scope ${role.memoryScopeKey}; latest activation ${assigned?.activationStatus ?? "not observed"}`} title={`Context · ${role.memoryScopeKey} · ${assigned?.activationStatus ?? "not activated in current assignment"}`}><b>C</b></i>
                      <i className="role-indicator is-configured" aria-label={`Project Manager tool profile ${role.toolProfileKey}; actual chat availability is unobserved`} title={`Tool profile · ${role.toolProfileKey}`}><b>T</b></i>
                    </span>
                    <i className={`graph-status-dot state-${state}`} aria-hidden="true" />
                  </button>
                  <div className="graph-worker-cluster" role="group" aria-label={`${project.name} recorded child workers`}>
                    {workers.map((worker, index) => (
                      <button
                        key={worker.id}
                        type="button"
                        className={`graph-worker-node state-${agentState(worker, snapshot.agentUpdates)} ${selectedAgentId === String(worker.id) ? "is-selected" : ""}`}
                        onClick={() => setSelectedAgentId(selectedAgentId === String(worker.id) ? null : String(worker.id))}
                        title={`${project.name} · Worker ${index + 1} · ${worker.status} · ${worker.taskTitle}${latestAgentUpdate(snapshot, worker.id) ? ` · ${latestAgentUpdate(snapshot, worker.id)?.summary}` : " · no checkpoint recorded"}`}
                        aria-label={`${project.name} Worker ${index + 1}: ${agentState(worker, snapshot.agentUpdates)}${latestAgentUpdate(snapshot, worker.id) ? `; latest checkpoint ${latestAgentUpdate(snapshot, worker.id)?.summary}` : "; no checkpoint recorded"}`}
                      ><b>W</b><i /></button>
                    ))}
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
        {selected && <div className="graph-agent-detail">
          <i className={`detail-state state-${agentState(selected, snapshot.agentUpdates)}`} />
          <span><strong>{selectedUpdate?.summary ?? selected.taskTitle}</strong><small>{agentRole(selected)} · {selectedUpdate?.phase ?? selected.status}{selectedUpdate ? ` · ${updateAge(selectedUpdate.createdAt)}` : " · no checkpoint"}{selected.report ? ` · report ${selected.report.reviewStatus.replace(/_/g, " ")}` : ""}</small></span>
          <button type="button" onClick={() => setSelectedAgentId(null)} aria-label="Close agent details"><Icon name="close" /></button>
        </div>}
      </div>
    </section>
  );
}

function CompactProjectSignals({ snapshot, loading }: { snapshot: DashboardSnapshot | null; loading: boolean }) {
  const items = snapshot ? projectMonitorItems(snapshot) : [];
  return (
    <span className="monitor-project-signals" title={loading ? "Reading project activity" : `${items.length} project statuses`} aria-label={loading ? "Reading project activity" : `${items.length} project statuses`}>
      {items.map((item) => <i key={item.project.id} className={`project-signal-dot state-${item.state}`} role="img" aria-label={`${item.project.name}: ${monitorStateLabel(item.state)}`} title={`${item.project.name}: ${monitorStateLabel(item.state)}`} />)}
    </span>
  );
}

function MonitorWindow({ snapshot, loading, error }: { snapshot: DashboardSnapshot | null; loading: boolean; error: string | null }) {
  const [expanded, setExpanded] = useState(false);
  const [pinned, setPinned] = useState(true);
  const [actionError, setActionError] = useState<string | null>(null);
  const projectItems = snapshot ? projectMonitorItems(snapshot) : [];

  const togglePinned = async () => {
    try {
      await getCurrentWindow().setAlwaysOnTop(!pinned);
      setPinned(!pinned);
      setActionError(null);
    } catch (reason) {
      setActionError(reason instanceof Error ? reason.message : "Could not update the floating window pin.");
    }
  };

  const toggleExpanded = async () => {
    const nextExpanded = !expanded;
    try {
      const window = getCurrentWindow();
      const nextWidth = nextExpanded ? 360 : 174;
      const nextHeight = nextExpanded ? 268 : 58;
      const display = await currentMonitor();
      if (display) {
        const origin = display.position.toLogical(display.scaleFactor);
        const bounds = display.size.toLogical(display.scaleFactor);
        const current = (await window.outerPosition()).toLogical(display.scaleFactor);
        const currentWidth = expanded ? 360 : 174;
        const currentHeight = expanded ? 268 : 58;
        const maxX = Math.max(origin.x + 8, origin.x + bounds.width - nextWidth - 8);
        const maxY = Math.max(origin.y + 8, origin.y + bounds.height - nextHeight - 8);
        const x = Math.min(Math.max(current.x + currentWidth - nextWidth, origin.x + 8), maxX);
        const y = Math.min(Math.max(current.y + currentHeight - nextHeight, origin.y + 8), maxY);
        await window.setPosition(new LogicalPosition(x, y));
      }
      await window.setSize(new LogicalSize(nextWidth, nextHeight));
      setExpanded(nextExpanded);
    } catch (reason) {
      setActionError(reason instanceof Error ? reason.message : "Could not resize the floating monitor.");
    }
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
          <CompactProjectSignals snapshot={snapshot} loading={loading} />
          <div className="monitor-actions">
            <button className={pinned ? "monitor-icon-button is-pinned" : "monitor-icon-button"} type="button" onMouseDown={(event) => event.stopPropagation()} onClick={() => void togglePinned()} aria-label={pinned ? "Turn always on top off" : "Keep monitor always on top"} title={pinned ? "Always on top" : "Pin above other windows"}><Icon name="pin" /></button>
            <button className="monitor-icon-button" type="button" onMouseDown={(event) => event.stopPropagation()} onClick={() => void toggleExpanded()} aria-label={expanded ? "Collapse monitor" : "Expand monitor"} title={expanded ? "Collapse" : "Expand"}><Icon name={expanded ? "collapse" : "expand"} /></button>
            <button className="monitor-icon-button" type="button" onMouseDown={(event) => event.stopPropagation()} onClick={closeWindow} aria-label="Close floating monitor" title="Close"><Icon name="close" /></button>
          </div>
        </header>
        {expanded && <div className="monitor-project-board" aria-label="Project activity" aria-live="polite">
            <div className="monitor-board-heading">
              <span>Projects</span>
              <button className="monitor-open" type="button" onMouseDown={(event) => event.stopPropagation()} onClick={() => void restoreDashboard()} aria-label="Open dashboard" title="Open dashboard"><Icon name="open" /></button>
            </div>
            {!snapshot ? <p className="monitor-error">{error ?? (loading ? "Reading local state…" : "No snapshot available")}</p> : projectItems.length === 0 ? <p className="monitor-empty">No registered projects</p> : projectItems.map((item) => (
              <article className={`monitor-project-row state-${item.state} ${item.attentionCount ? "has-attention" : ""} ${item.staleAgentCount ? "has-stale" : ""}`} key={item.project.id} title={`${item.project.name} — ${item.attentionCount} owner actions; ${item.staleAgentCount} stale agent signals; ${monitorStateLabel(item.state)}: ${item.detail}`}>
                <span className={`project-state-indicator state-${item.state}`} aria-hidden="true"><i />{item.state === "working" && <span className="project-activity-wave"><i /><i /><i /></span>}</span>
                <span className="monitor-project-copy"><strong>{item.project.name}</strong><small>{item.detail}</small></span>
                <span className="monitor-project-meta">
                  {item.issueCount > 0 && <b className={`project-issue-count ${item.attentionCount ? "is-attention" : ""} ${item.staleAgentCount ? "is-stale" : ""}`} title={`${item.attentionCount} owner actions; ${item.staleAgentCount} stale agent signals; ${item.issueCount} total attention signals`}>{item.issueCount}</b>}
                  {item.state === "working" && item.activeAgents > 0 && <b className="project-agent-count" title={`${item.activeAgents} active agents`}>{item.activeAgents}</b>}
                  <span className={`project-state-label state-${item.state}`}>{monitorStateLabel(item.state)}</span>
                </span>
              </article>
            ))}
        </div>}
        {actionError && <p className="monitor-error" role="status">{actionError}</p>}
      </section>
    </main>
  );
}
