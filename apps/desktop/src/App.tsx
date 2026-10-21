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
  projectManagerAssignmentStatus: string | null;
  activationStatus: string | null;
  parentRunStatus: string;
  createdAt: number;
  updatedAt: number;
  report: WorkerReportRecord | null;
  worktree: DelegationWorktreeState | null;
  workerTelemetry: DelegationWorkerTelemetry | null;
};
type DashboardSnapshot = {
  projects: ProjectSnapshot[];
  objectives: { id: number; projectId: number; projectName: string; title: string; status: string; updatedAt: number }[];
  blockers: { id: number; objectiveId: number; projectId: number; projectName: string; title: string; updatedAt: number }[];
  recentRuns: {
    id: number;
    projectId: number;
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
    lastEventKind: string;
    lastEventAt: number;
    eventCount: number;
  }[];
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

type DashboardTab = "activity" | "repositories" | "memory";
type RunRecord = DashboardSnapshot["recentRuns"][number];
type HookAgentRecord = DashboardSnapshot["agentActivity"][number];
type ProjectMonitorState = "working" | "blocked" | "failed" | "stopped" | "idle";
type ProjectMonitorItem = {
  project: ProjectSnapshot;
  state: ProjectMonitorState;
  detail: string;
  activeAgents: number;
  issueCount: number;
};
type GraphAgentNode = {
  id: string;
  state: AgentVisualState;
  label: string;
  initial: string;
  delegation?: DelegationSnapshot;
  activity?: HookAgentRecord;
  parentId?: string;
};

type ProjectActivitySnapshot = {
  project: ProjectSnapshot;
  state: ProjectMonitorState;
  run: RunRecord | null;
  managers: DelegationSnapshot[];
  childrenByManager: Map<number, DelegationSnapshot[]>;
  unlinkedNodes: GraphAgentNode[];
  delegations: DelegationSnapshot[];
  hookAgents: HookAgentRecord[];
  attentionCount: number;
};

const dashboardTabs: { id: DashboardTab; label: string; icon: "activity" | "repositories" | "memory" }[] = [
  { id: "activity", label: "Agent activity", icon: "activity" },
  { id: "repositories", label: "Repositories", icon: "repositories" },
  { id: "memory", label: "Memory", icon: "memory" },
];

function nativeRuntimeAvailable() {
  // Test harnesses may mock only `invoke`; the real browser import always exposes isTauri().
  return typeof isTauri === "function" ? isTauri() : true;
}

function shortHead(head: string | null) {
  return head ? head.slice(0, 8) : "No commit yet";
}

function runTitle(run: RunRecord) {
  return (run.taskTitle ?? run.objectiveTitle ?? run.workflowKey ?? `Run ${run.id}`).replace(/_/g, " ");
}

function projectMonitorItems(snapshot: DashboardSnapshot): ProjectMonitorItem[] {
  const priority: Record<ProjectMonitorState, number> = { working: 0, blocked: 1, failed: 2, stopped: 3, idle: 4 };
  return snapshot.projects.map((project) => {
    const latestRun = snapshot.recentRuns
      .filter((run) => run.projectId === project.id)
      .sort((left, right) => right.startedAt - left.startedAt || right.id - left.id)[0] ?? null;
    const blockers = snapshot.blockers.filter((blocker) => blocker.projectId === project.id);
    const mandatoryFailures = latestRun
      ? snapshot.failedChecks.filter((check) => check.runId === latestRun.id && check.mandatory && check.status.toLowerCase() === "failed")
      : [];
    const state: ProjectMonitorState = latestRun?.status === "running"
      ? "working"
      : latestRun?.status === "blocked" || (!latestRun && blockers.length > 0)
        ? "blocked"
        : latestRun?.status === "failed" || mandatoryFailures.length > 0
          ? "failed"
          : latestRun
            ? "stopped"
            : "idle";
    const delegations = latestRun ? snapshot.delegations.filter((item) => item.runId === latestRun.id) : [];
    const activeAgents = delegations.filter((item) => agentState(item) === "active").length;
    const detail = blockers.length > 0 && !latestRun
      ? blockers[0].title
      : latestRun
        ? runTitle(latestRun)
        : "No recorded work";
    return {
      project,
      state,
      detail,
      activeAgents,
      issueCount: blockers.length + mandatoryFailures.length,
    };
  }).sort((left, right) => {
    return priority[left.state] - priority[right.state] || left.project.name.localeCompare(right.project.name);
  });
}

function monitorStateLabel(state: ProjectMonitorState) {
  return state === "working" ? "Working" : state === "blocked" ? "Blocked" : state === "failed" ? "Failed" : state === "stopped" ? "Stopped" : "Idle";
}

function statusTone(status: string) {
  if (status === "verified_success" || status === "passed" || status === "accepted" || status === "clean") return "good";
  if (status === "running" || status === "dispatched" || status === "reported") return "live";
  if (status === "failed" || status === "blocked" || status === "rejected") return "danger";
  if (status === "inconclusive" || status === "not_run" || status === "needs_follow_up") return "review";
  return "quiet";
}

function formatBytes(bytes: number) {
  if (bytes < 1_024) return `${bytes} B`;
  return `${(bytes / 1_024).toFixed(bytes < 10_240 ? 1 : 0)} KiB`;
}

function BrandMark() {
  return <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>;
}

function Icon({ name }: { name: "activity" | "repositories" | "memory" | "monitor" | "settings" | "pin" | "expand" | "collapse" | "open" | "close" }) {
  const paths: Record<typeof name, ReactNode> = {
    activity: <><circle cx="5" cy="12" r="2" /><circle cx="18" cy="5" r="2" /><circle cx="18" cy="19" r="2" /><path d="m7 11 9-5M7 13l9 5" /></>,
    repositories: <><path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" /><path d="M3 10h18" /></>,
    memory: <><ellipse cx="12" cy="5" rx="8" ry="3" /><path d="M4 5v7c0 1.7 3.6 3 8 3s8-1.3 8-3V5M4 12v7c0 1.7 3.6 3 8 3s8-1.3 8-3v-7" /></>,
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
  const [activeTab, setActiveTab] = useState<DashboardTab>("activity");
  const [selectedRunId, setSelectedRunId] = useState<number | null>(null);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
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
          x: Math.max(origin.x + 8, origin.x + size.width - 148 - 8),
          y: Math.max(origin.y + 8, origin.y + size.height - 42 - 48),
        };
      }
      const monitor = new WebviewWindow("monitor", {
        url: "/?window=monitor",
        title: "Cathulu Monitor",
        width: 148,
        height: 42,
        minWidth: 148,
        minHeight: 42,
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
        <a className="support-brand" href="#dashboard" aria-label="Cathulu activity" onClick={() => { setSettingsOpen(false); setActiveTab("activity"); }}>
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
          <nav className="dashboard-rail" aria-label="Dashboard views">
            {dashboardTabs.map((tab) => (
              <button key={tab.id} type="button" className={activeTab === tab.id ? "rail-button is-active" : "rail-button"} aria-label={tab.label} aria-current={activeTab === tab.id ? "page" : undefined} title={tab.label} onClick={() => setActiveTab(tab.id)}>
                <Icon name={tab.icon} />
              </button>
            ))}
          </nav>

          <div className="support-content" key={activeTab}>
            {error && snapshot && <span className="refresh-indicator" role="status" title="Refresh failed. Showing the last saved snapshot.">!</span>}
            {monitorError && <p className="inline-action-error" role="status">{monitorError}</p>}
            {!snapshot ? (
              <SnapshotState loading={loading} error={error} />
            ) : activeTab === "activity" ? (
              <AgentActivityView snapshot={snapshot} selectedRunId={selectedRunId} onSelectRun={setSelectedRunId} selectedProjectId={selectedProjectId} onSelectProject={setSelectedProjectId} />
            ) : activeTab === "repositories" ? (
              <RepositoriesView projects={snapshot.projects} />
            ) : (
              <MemoryView health={snapshot.memoryHealth} />
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

type AgentVisualState = "active" | "waiting" | "finished" | "blocked" | "unknown";

function agentState(delegation: DelegationSnapshot): AgentVisualState {
  const lastEvent = [...(delegation.workerTelemetry?.events ?? [])]
    .sort((left, right) => left.createdAt - right.createdAt || left.eventId - right.eventId)
    .at(-1);
  const status = delegation.status.toLowerCase();
  const outcome = delegation.report?.report.outcome.toLowerCase();
  if (["blocked", "failed", "cancelled"].includes(status) || outcome === "blocked" || delegation.report?.report.blockers.length) return "blocked";
  if (lastEvent?.eventKind === "started" && delegation.parentRunStatus === "running") return "active";
  if (lastEvent?.eventKind === "stopped" || ["completed", "accepted", "reported"].includes(status) || delegation.report) return "finished";
  if (["pending", "waiting"].includes(status)) return "waiting";
  // Prepared or dispatched is a Cathulu record, not proof that Codex started the agent.
  return "unknown";
}

function unlinkedHookState(agent: HookAgentRecord): AgentVisualState {
  if (agent.lastEventKind === "stopped") return "finished";
  return "unknown";
}

function agentRole(delegation: DelegationSnapshot) {
  return delegation.role.replace(/_/g, " ");
}

function projectActivitySnapshot(snapshot: DashboardSnapshot, project: ProjectSnapshot): ProjectActivitySnapshot {
  const projectRuns = snapshot.recentRuns
    .filter((item) => item.projectId === project.id)
    .sort((left, right) => right.startedAt - left.startedAt || right.id - left.id);
  const run = projectRuns.find((item) => item.status === "running") ?? projectRuns[0] ?? null;
  const delegations = run ? snapshot.delegations.filter((item) => item.runId === run.id && item.projectId === project.id) : [];
  const managers = delegations.filter((item) => item.role === "sub_overseer" && item.parentDelegationId === null && item.projectManagerAssignmentStatus !== null);
  const managerIds = new Set(managers.map((item) => item.id));
  const childrenByManager = new Map<number, DelegationSnapshot[]>();
  for (const worker of delegations) {
    if (worker.role !== "worker" || worker.parentDelegationId === null || !managerIds.has(worker.parentDelegationId)) continue;
    const children = childrenByManager.get(worker.parentDelegationId) ?? [];
    children.push(worker);
    childrenByManager.set(worker.parentDelegationId, children);
  }
  const unlinkedDelegations = delegations.filter((item) => item.role === "worker" && (item.parentDelegationId === null || !managerIds.has(item.parentDelegationId)));
  const hookAgents = run ? snapshot.agentActivity.filter((item) => item.runId === run.id) : [];
  const unlinkedNodes: GraphAgentNode[] = [
    ...unlinkedDelegations.map((delegation) => ({
      id: `delegation-${delegation.id}`,
      state: agentState(delegation),
      label: "Unlinked worker",
      initial: "W",
      delegation,
    })),
    ...hookAgents.map((activity) => ({
      id: `hook-${activity.id}`,
      state: unlinkedHookState(activity),
      label: activity.agentType.replace(/_/g, " "),
      initial: activity.agentType.toLowerCase().includes("worker") ? "W" : "?",
      activity,
    })),
  ];
  const monitorState = projectMonitorItems(snapshot).find((item) => item.project.id === project.id)?.state ?? "idle";
  const attentionCount = delegations.filter((item) => agentState(item) === "blocked").length
    + (run ? snapshot.failedChecks.filter((check) => check.runId === run.id && check.mandatory && check.status.toLowerCase() === "failed").length : 0);
  return { project, state: monitorState, run, managers, childrenByManager, unlinkedNodes, delegations, hookAgents, attentionCount };
}

function PortfolioActivityGraph({ snapshot }: { snapshot: DashboardSnapshot }) {
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const projects = snapshot.projects.map((project) => projectActivitySnapshot(snapshot, project));
  const activeCount = projects.reduce((count, item) => count + item.delegations.filter((delegation) => agentState(delegation) === "active").length, 0);
  const attentionCount = projects.reduce((count, item) => count + item.attentionCount, 0);
  const branches = projects.map((item, index) => ({
    ...item,
    center: ((index + 0.5) * 1000) / Math.max(1, projects.length),
  }));
  const selectedNode = projects.flatMap((item) => [
    ...item.managers,
    ...[...item.childrenByManager.values()].flat(),
    ...item.unlinkedNodes.flatMap((node) => node.delegation ? [node.delegation] : []),
  ]).find((item) => `delegation-${item.id}` === selectedAgentId) ?? null;
  const selectedHook = projects.flatMap((item) => item.unlinkedNodes).find((node) => node.id === selectedAgentId)?.activity ?? null;

  return <>
    <div className="graph-toolbar">
      <div className="graph-run-copy portfolio-copy">
        <i className="run-light" aria-hidden="true" />
        <span>Portfolio</span>
        <small>One Main Overseer · {projects.length} project{projects.length === 1 ? "" : "s"}</small>
      </div>
      <div className="graph-counts" aria-label="Portfolio activity summary">
        <span className="count-active" title={`${activeCount} agents observed active`} aria-label={`${activeCount} agents observed active`}><i />{activeCount}</span>
        <span className={attentionCount ? "count-blocked has-issues" : "count-blocked"} title={`${attentionCount} blocked agents or failed required checks`} aria-label={`${attentionCount} items need attention`}><i />{attentionCount}</span>
      </div>
    </div>

    <div className="portfolio-stage" aria-label="Portfolio-wide Main Overseer and project activity">
      <div className="portfolio-root-row">
        <div className="hierarchy-node hierarchy-main state-coordinator" role="group" aria-label="Main Overseer, global coordinator">
          <span className="hierarchy-avatar"><BrandMark /></span><span className="hierarchy-copy"><small>One logical coordinator</small><strong>Main Overseer</strong></span>
        </div>
      </div>
      {projects.length > 0 && <svg className="portfolio-edges" viewBox="0 0 1000 42" preserveAspectRatio="none" aria-hidden="true">
        {branches.map((item) => <path key={item.project.id} d={`M500 0 C500 19 ${item.center} 19 ${item.center} 42`} className={`portfolio-edge state-${item.state}`} />)}
      </svg>}
      {projects.length > 0 ? <div className="portfolio-project-grid" style={{ gridTemplateColumns: `repeat(${Math.min(projects.length, 4)}, minmax(0, 1fr))` }}>
        {projects.map((item) => <article key={item.project.id} className={`portfolio-project state-${item.state}`} role="group" aria-label={`${item.project.name}: ${monitorStateLabel(item.state)}${item.run ? `, ${runTitle(item.run)}` : ", no recorded run"}`}>
          <header className="portfolio-project-heading">
            <i className={`project-state-dot state-${item.state}`} aria-hidden="true" />
            <span className="portfolio-project-copy"><strong title={item.project.name}>{item.project.name}</strong><small title={item.run ? `${item.run.status} · ${runTitle(item.run)}` : "No recorded run"}>{item.run ? `${item.run.status.replace(/_/g, " ")} · ${runTitle(item.run)}` : "No recorded run"}</small></span>
            <span className={`portfolio-state state-${item.state}`} title={monitorStateLabel(item.state)} aria-label={monitorStateLabel(item.state)} />
          </header>
          <div className="portfolio-assignment-flow">
            {item.managers.length > 0 ? item.managers.map((manager) => {
              const managerState = agentState(manager);
              const children = item.childrenByManager.get(manager.id) ?? [];
              return <div className="portfolio-assignment" key={manager.id}>
                <button type="button" className={`portfolio-agent hierarchy-manager state-${managerState}`} onClick={() => setSelectedAgentId(`delegation-${manager.id}`)} aria-label={`Project Manager, ${stateLabel(managerState)}: ${manager.taskTitle}`} title={`${manager.taskTitle} · ${stateLabel(managerState)}`}>
                  <span className="portfolio-agent-avatar">PM<i className="node-state-mark" /></span><span className="portfolio-agent-copy"><small>Project Manager</small><strong>{manager.taskTitle}</strong></span>
                </button>
                {children.length > 0 ? <div className="portfolio-workers">{children.map((worker) => {
                  const workerState = agentState(worker);
                  return <button key={worker.id} type="button" className={`portfolio-agent portfolio-worker hierarchy-worker state-${workerState}`} onClick={() => setSelectedAgentId(`delegation-${worker.id}`)} aria-label={`Worker, ${stateLabel(workerState)}: ${worker.taskTitle}`} title={`${worker.taskTitle} · ${stateLabel(workerState)}`}>
                    <span className="portfolio-agent-avatar">W<i className="node-state-mark" /></span><span className="portfolio-agent-copy"><small>Worker</small><strong>{worker.taskTitle}</strong></span>
                  </button>;
                })}</div> : <div className="portfolio-no-worker">No linked workers</div>}
              </div>;
            }) : <div className="portfolio-no-manager"><span className="portfolio-agent-avatar">PM</span><span><strong>No Project Manager</strong><small>{item.run ? "No assignment recorded for this run" : "No run to assign"}</small></span></div>}
            {item.unlinkedNodes.length > 0 && <div className="portfolio-unlinked">{item.unlinkedNodes.map((node) => <button key={node.id} type="button" className={`portfolio-unlinked-node state-${node.state}`} title={`${node.label} · ${stateLabel(node.state)}`} aria-label={`${node.label === "Unlinked worker" ? "worker" : node.label}, ${stateLabel(node.state)}`} onClick={() => setSelectedAgentId(node.id)}><i />{node.initial}</button>)}<small>Unlinked</small></div>}
            {(selectedNode && item.delegations.some((delegation) => delegation.id === selectedNode.id) || selectedHook && item.hookAgents.some((activity) => activity.id === selectedHook.id)) && <div className="portfolio-agent-detail" role="status">
              <i className={`detail-state state-${selectedNode ? agentState(selectedNode) : unlinkedHookState(selectedHook!)}`} />
              <span><strong>{selectedNode?.taskTitle ?? selectedHook?.agentType.replace(/_/g, " ")}</strong><small>{selectedNode ? `${agentRole(selectedNode)} · ${stateLabel(agentState(selectedNode))}${selectedNode.workerTelemetry?.effectiveModel ? ` · ${selectedNode.workerTelemetry.effectiveModel}` : ""}` : `${stateLabel(unlinkedHookState(selectedHook!))} · ${selectedHook?.eventCount} events, unlinked`}</small></span>
              <button type="button" onClick={() => setSelectedAgentId(null)} aria-label="Close agent details"><Icon name="close" /></button>
            </div>}
          </div>
        </article>)}
      </div> : <div className="portfolio-empty"><BrandMark /><span>Register projects through Cathulu MCP to see them here.</span></div>}
    </div>
  </>;
}

function AgentActivityView({ snapshot, selectedRunId, onSelectRun, selectedProjectId, onSelectProject }: {
  snapshot: DashboardSnapshot;
  selectedRunId: number | null;
  onSelectRun: (id: number) => void;
  selectedProjectId: number | null;
  onSelectProject: (id: number | null) => void;
}) {
  const project = selectedProjectId === null ? null : snapshot.projects.find((item) => item.id === selectedProjectId) ?? null;
  const projectRuns = project ? snapshot.recentRuns
    .filter((item) => item.projectId === project.id)
    .sort((left, right) => right.startedAt - left.startedAt || right.id - left.id) : [];
  const run = projectRuns.find((item) => item.id === selectedRunId)
    ?? projectRuns.find((item) => item.status === "running")
    ?? projectRuns[0]
    ?? null;
  const delegations = run ? snapshot.delegations.filter((item) => item.runId === run.id && item.projectId === project?.id) : [];
  const managers = delegations.filter((item) => item.role === "sub_overseer" && item.parentDelegationId === null && item.projectManagerAssignmentStatus !== null);
  const managerIds = new Set(managers.map((item) => item.id));
  const childrenByManager = new Map<number, DelegationSnapshot[]>();
  for (const worker of delegations) {
    if (worker.role !== "worker" || worker.parentDelegationId === null || !managerIds.has(worker.parentDelegationId)) continue;
    const children = childrenByManager.get(worker.parentDelegationId) ?? [];
    children.push(worker);
    childrenByManager.set(worker.parentDelegationId, children);
  }
  const unlinkedDelegations = delegations.filter((item) => item.role === "worker" && (item.parentDelegationId === null || !managerIds.has(item.parentDelegationId)));
  const hookAgents = run ? snapshot.agentActivity.filter((item) => item.runId === run.id) : [];
  const unlinkedNodes: GraphAgentNode[] = [
    ...unlinkedDelegations.map((delegation) => ({
      id: `delegation-${delegation.id}`,
      state: agentState(delegation),
      label: "Unlinked worker",
      initial: "W",
      delegation,
    })),
    ...hookAgents.map((activity) => ({
      id: `hook-${activity.id}`,
      state: unlinkedHookState(activity),
      label: activity.agentType.replace(/_/g, " "),
      initial: activity.agentType.toLowerCase().includes("worker") ? "W" : "?",
      activity,
    })),
  ];
  const activeCount = delegations.filter((item) => agentState(item) === "active").length;
  const attentionCount = delegations.filter((item) => agentState(item) === "blocked").length
    + (run ? snapshot.failedChecks.filter((check) => check.runId === run.id && check.mandatory && check.status.toLowerCase() === "failed").length : 0);
  const selectedNodes = [...managers, ...[...childrenByManager.values()].flat(), ...unlinkedDelegations];
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const selected = selectedNodes.find((item) => `delegation-${item.id}` === selectedAgentId)
    ?? unlinkedNodes.find((item) => item.id === selectedAgentId)?.delegation
    ?? null;
  const selectedHook = unlinkedNodes.find((item) => item.id === selectedAgentId)?.activity ?? null;
  useEffect(() => setSelectedAgentId(null), [run?.id, project?.id]);

  return (
    <section className="agent-activity-view" aria-label="Project agent activity">
      {snapshot.projects.length > 0 && <nav className="project-switcher" aria-label="Filter activity by project">
        <span className="project-switcher-label">Focus</span>
        <button type="button" className={`project-switcher-item ${project === null ? "is-selected" : ""}`} aria-pressed={project === null} title="Show the single Main Overseer and activity across every project" onClick={() => onSelectProject(null)}>All projects</button>
        {snapshot.projects.map((item) => {
          const state = projectMonitorItems(snapshot).find((status) => status.project.id === item.id)?.state ?? "idle";
          return <button key={item.id} type="button" className={`project-switcher-item ${project?.id === item.id ? "is-selected" : ""}`} aria-pressed={project?.id === item.id} title={`Focus activity for ${item.name}`} onClick={() => onSelectProject(item.id)}>
            <i className={`project-state-dot state-${state}`} aria-hidden="true" /><span>{item.name}</span>
          </button>;
        })}
      </nav>}

      {project === null ? <PortfolioActivityGraph snapshot={snapshot} /> : <>

      <div className="graph-toolbar">
        <div className="graph-run-copy">
          <i className={`run-light ${run?.status === "running" ? "is-active" : statusTone(run?.status ?? "quiet")}`} aria-hidden="true" />
          <span title={run ? `${project?.name} · ${run.status}` : "No run recorded"}>{project?.name ?? "No project"}</span>
          {run && <small>{runTitle(run)}</small>}
        </div>
        <div className="graph-counts" aria-label="Activity summary">
          <span className="count-active" title={`${activeCount} agents observed active`} aria-label={`${activeCount} agents observed active`}><i />{activeCount}</span>
          <span className={attentionCount ? "count-blocked has-issues" : "count-blocked"} title={`${attentionCount} blocked agents or failed required checks`} aria-label={`${attentionCount} items need attention`}><i />{attentionCount}</span>
        </div>
      </div>

      <div className="hierarchy-stage" aria-label={run ? `Run ${run.id} hierarchy for ${project?.name}` : "Project hierarchy"}>
        {run ? <>
          <svg className="hierarchy-edges" viewBox="0 0 1000 620" preserveAspectRatio="none" aria-hidden="true">
            {managers.map((manager, managerIndex) => {
              const managerX = ((managerIndex + 1) * 1000) / (managers.length + 1);
              const edges = [<path key={`main-${manager.id}`} d={`M500 150 C500 210 ${managerX} 210 ${managerX} 285`} className={`hierarchy-edge state-${agentState(manager)}`} />];
              const workers = childrenByManager.get(manager.id) ?? [];
              return <g key={manager.id}>{edges}{workers.map((worker, workerIndex) => {
                const step = Math.min(200, 270 / Math.max(1, workers.length));
                const workerX = managerX + (workerIndex - (workers.length - 1) / 2) * step;
                return <path key={worker.id} d={`M${managerX} 355 C${managerX} 405 ${workerX} 405 ${workerX} 480`} className={`hierarchy-edge state-${agentState(worker)}`} />;
              })}</g>;
            })}
          </svg>
          <div className="hierarchy-node hierarchy-main state-coordinator" role="group" aria-label="Main Overseer, global coordinator">
            <span className="hierarchy-avatar"><BrandMark /></span><span className="hierarchy-copy"><small>Global · coordinating</small><strong>Main Overseer</strong></span>
          </div>
          {managers.map((manager, index) => {
            const x = `${((index + 1) * 100) / (managers.length + 1)}%`;
            const state = agentState(manager);
            return <button key={manager.id} type="button" className={`hierarchy-node hierarchy-manager state-${state}`} style={{ left: x }} onClick={() => setSelectedAgentId(`delegation-${manager.id}`)} aria-label={`Project Manager, ${stateLabel(state)}: ${manager.taskTitle}`} title={`${manager.taskTitle} · ${stateLabel(state)}`}>
              <span className="hierarchy-avatar">PM<i className="node-state-mark" /></span><span className="hierarchy-copy"><small>Project Manager</small><strong>{manager.taskTitle}</strong></span>
            </button>;
          })}
          {[...childrenByManager.entries()].flatMap(([managerId, workers]) => {
            const managerIndex = managers.findIndex((item) => item.id === managerId);
            const managerX = ((managerIndex + 1) * 100) / (managers.length + 1);
            const step = Math.min(20, 27 / Math.max(1, workers.length));
            return workers.map((worker, workerIndex) => {
              const x = `${managerX + (workerIndex - (workers.length - 1) / 2) * step}%`;
              const state = agentState(worker);
              return <button key={worker.id} type="button" className={`hierarchy-node hierarchy-worker state-${state}`} style={{ left: x }} onClick={() => setSelectedAgentId(`delegation-${worker.id}`)} aria-label={`Worker, ${stateLabel(state)}: ${worker.taskTitle}`} title={`${worker.taskTitle} · ${stateLabel(state)}`}>
                <span className="hierarchy-avatar">W<i className="node-state-mark" /></span><span className="hierarchy-copy"><small>Worker</small><strong>{worker.taskTitle}</strong></span>
              </button>;
            });
          })}
          {managers.length === 0 && <div className="hierarchy-empty"><span className="hierarchy-empty-orbit" /><small>No Project Manager assignment recorded</small></div>}
          {(selected || selectedHook) && <div className="hierarchy-detail" role="status">
            <i className={`detail-state state-${selected ? agentState(selected) : unlinkedHookState(selectedHook!)}`} /><span><strong>{selected?.taskTitle ?? selectedHook?.agentType.replace(/_/g, " ")}</strong><small>{selected ? `${agentRole(selected)} · ${stateLabel(agentState(selected))}${selected.workerTelemetry?.effectiveModel ? ` · ${selected.workerTelemetry.effectiveModel}` : ""}` : `${stateLabel(unlinkedHookState(selectedHook!))}${selectedHook?.model ? ` · ${selectedHook.model}` : ""} · ${selectedHook?.eventCount} events, unlinked`}</small></span>
            <button type="button" onClick={() => setSelectedAgentId(null)} aria-label="Close agent details"><Icon name="close" /></button>
          </div>}
        </> : <div className="graph-empty-state"><span className="graph-empty-orbit"><BrandMark /></span><small>{project ? "No recorded runs" : "Register a project through Cathulu MCP"}</small></div>}
      </div>

      {unlinkedNodes.length > 0 && <div className="unlinked-observations" aria-label="Unlinked activity observations">
        <span>Unlinked</span>{unlinkedNodes.map((node) => <button key={node.id} type="button" className={`unlinked-observation state-${node.state}`} title={`${node.label} · ${stateLabel(node.state)}`} aria-label={`${node.label}, ${stateLabel(node.state)}`} onClick={() => setSelectedAgentId(node.id)}><i />{node.initial}</button>)}
      </div>}

      {projectRuns.length > 1 && <div className="run-history" aria-label="Project run history">
        {projectRuns.slice(0, 8).reverse().map((item) => <button key={item.id} type="button" className={`run-history-mark state-${statusTone(item.status)} ${run?.id === item.id ? "is-selected" : ""}`} onClick={() => onSelectRun(item.id)} title={`${runTitle(item)} · ${item.status}`} aria-label={`Show ${runTitle(item)}, ${item.status}`} />)}
      </div>}
      </>}
    </section>
  );
}

function stateLabel(state: AgentVisualState) {
  return state === "active" ? "Observed active" : state === "blocked" ? "Blocked" : state === "finished" ? "Stopped" : state === "waiting" ? "Waiting" : "Unobserved";
}

function RepositoriesView({ projects }: { projects: ProjectSnapshot[] }) {
  return projects.length ? (
    <ul className="repository-signals" aria-label="Repository status">
      {projects.map((project) => <li key={project.id} className={project.dirty ? "is-dirty" : "is-clean"} title={`${project.name} · ${project.branch ?? "Detached HEAD"} · ${project.changedPaths.length} changed paths`}>
        <span className="repository-signal-dot" />
        <span className="repository-signal-copy"><strong>{project.name}</strong><small>{project.branch ?? "Detached HEAD"}</small></span>
        {project.dirty ? <b className="repository-change-count">{project.changedPaths.length}</b> : <i className="repository-clean-mark" aria-label="Clean" />}
        <span className="repository-head">{shortHead(project.head)}</span>
      </li>)}
    </ul>
  ) : <div className="visual-empty"><span className="repository-empty-icon"><Icon name="repositories" /></span><small>No repositories</small></div>;
}

function MemoryView({ health }: { health: DashboardSnapshot["memoryHealth"] }) {
  const totalBytes = Math.max(1, health.hotBytes + health.warmBytes + health.coldBytes);
  const tiers = [
    { key: "hot", label: "HOT", bytes: health.hotBytes, fill: Math.max(3, (health.hotBytes / totalBytes) * 100) },
    { key: "warm", label: "WARM", bytes: health.warmBytes, fill: Math.max(3, (health.warmBytes / totalBytes) * 100) },
    { key: "cold", label: "COLD", bytes: health.coldBytes, fill: Math.max(3, (health.coldBytes / totalBytes) * 100) },
  ];
  return (
    <section className="memory-visual" aria-label="Memory health">
      <div className="memory-tier-visuals">{tiers.map((tier) => <div key={tier.key} className={`memory-tier ${tier.key}`} title={`${tier.label} · ${formatBytes(tier.bytes)}`}>
        <div className="memory-tier-ring"><span style={{ "--fill": `${tier.fill}%` } as CSSProperties} /></div>
        <strong>{formatBytes(tier.bytes)}</strong><small>{tier.label}</small>
      </div>)}</div>
      <div className="memory-type-visuals" aria-label="Memory record counts">
        <span title={`${health.canonicalCount} canonical memories`}><i>C</i><b>{health.canonicalCount}</b></span>
        <span title={`${health.episodicCount} episodic memories`}><i>E</i><b>{health.episodicCount}</b></span>
        <span title={`${health.evidenceCount} evidence records`}><i>A</i><b>{health.evidenceCount}</b></span>
      </div>
      <div className="memory-storage-track" title={`SQLite ${formatBytes(health.sqliteBytes)}`}><span style={{ width: `${Math.min(100, Math.max(4, health.sqliteBytes / 10_000_000 * 100))}%` }} /><small>{formatBytes(health.sqliteBytes)} local</small></div>
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
      const nextWidth = nextExpanded ? 324 : 148;
      const nextHeight = nextExpanded ? 204 : 42;
      const display = await currentMonitor();
      if (display) {
        const origin = display.position.toLogical(display.scaleFactor);
        const bounds = display.size.toLogical(display.scaleFactor);
        const current = (await window.outerPosition()).toLogical(display.scaleFactor);
        const currentWidth = expanded ? 324 : 148;
        const currentHeight = expanded ? 204 : 42;
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
              <article className={`monitor-project-row state-${item.state}`} key={item.project.id} title={`${item.project.name} — ${monitorStateLabel(item.state)}: ${item.detail}`}>
                <span className={`project-state-indicator state-${item.state}`} aria-hidden="true"><i />{item.state === "working" && <span className="project-activity-wave"><i /><i /><i /></span>}</span>
                <span className="monitor-project-copy"><strong>{item.project.name}</strong><small>{item.detail}</small></span>
                <span className="monitor-project-meta">
                  {item.issueCount > 0 && <b className="project-issue-count" title={`${item.issueCount} open issue${item.issueCount === 1 ? "" : "s"}`}>{item.issueCount}</b>}
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
