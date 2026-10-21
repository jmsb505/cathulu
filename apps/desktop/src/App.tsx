import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";

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

type WorkerCheckResult = {
  name: string;
  status: string;
  summary: string;
};

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
  projectName: string;
  taskTitle: string;
  runId: number;
  role: string;
  status: string;
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
  blockers: { id: number; objectiveId: number; projectName: string; title: string; updatedAt: number }[];
  recentRuns: {
    id: number; projectName: string; objectiveTitle: string | null; taskTitle: string | null;
    workflowKey: string | null; workflowVersion: number | null; status: string; summary: string;
    startedAt: number; finishedAt: number | null;
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

function shortHead(head: string | null) {
  return head ? head.slice(0, 8) : "No commit yet";
}

function formatSnapshotTime(timestamp: number) {
  if (!timestamp) return "Not reconciled yet";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp * 1000));
}

export default function App() {
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const projects = snapshot?.projects ?? [];
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const loadSnapshot = () => {
      invoke<DashboardSnapshot>("dashboard_snapshot")
        .then((snapshot) => {
          if (!cancelled) {
            setSnapshot(snapshot);
            setError(null);
          }
        })
        .catch((reason: unknown) => {
          if (!cancelled) {
            setError(reason instanceof Error ? reason.message : String(reason));
          }
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
  }, []);

  const changedProjects = useMemo(
    () => projects.filter((project) => project.dirty).length,
    [projects],
  );
  const latestSnapshot = useMemo(
    () => Math.max(0, ...projects.map((project) => project.reconciledAt)),
    [projects],
  );

  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="wordmark" href="#main" aria-label="Cathulu home">
          <span className="wordmark-mark" aria-hidden="true">
            <span />
            <span />
            <span />
          </span>
          <span>CATHULU</span>
        </a>
        <div className="topbar-meta">
          <span className="local-indicator"><span /> Local workspace</span>
          <span className="read-only-label">Read only</span>
        </div>
      </header>

      <main id="main" className="main-content">
        <section className="intro" aria-labelledby="page-title">
          <div className="intro-copy">
            <p className="eyebrow">PROJECT OVERVIEW <span> / </span> PHASE 01</p>
            <h1 id="page-title">Your work,<br /><em>in clear view.</em></h1>
            <p className="intro-description">
              A grounded snapshot of your repositories. Codex drives the work;
              Cathulu keeps the project state close at hand.
            </p>
          </div>
          <div className="orbit-art" aria-hidden="true">
            <div className="orbit orbit-one" />
            <div className="orbit orbit-two" />
            <div className="orbit-core"><span /></div>
            <span className="orbit-dot dot-one" />
            <span className="orbit-dot dot-two" />
          </div>
        </section>

        <section className="overview-strip" aria-label="Portfolio summary">
          <div className="summary-cell">
            <span className="summary-label">Repositories</span>
            <strong>{loading ? "—" : String(projects.length).padStart(2, "0")}</strong>
          </div>
          <div className="summary-cell">
            <span className="summary-label">With changes</span>
            <strong>{loading ? "—" : String(changedProjects).padStart(2, "0")}</strong>
          </div>
          <div className="summary-cell summary-updated">
            <span className="summary-label">Latest Git snapshot</span>
            <strong>{loading ? "Checking local state…" : formatSnapshotTime(latestSnapshot)}</strong>
          </div>
          <span className="summary-spark" aria-hidden="true">✳</span>
        </section>

        <section className="projects-section" aria-labelledby="projects-title">
          <div className="section-heading">
            <div>
              <p className="eyebrow">THE PORTFOLIO</p>
              <h2 id="projects-title">Repositories</h2>
            </div>
            <p className="section-note">Snapshots update when Codex asks Cathulu to reconcile.</p>
          </div>

          {loading && (
            <div className="state-panel" role="status">
              <span className="loading-mark" aria-hidden="true" />
              <p>Reading the local project snapshot…</p>
            </div>
          )}

          {!loading && error && (
            <div className="state-panel state-error" role="alert">
              <span className="state-symbol" aria-hidden="true">!</span>
              <div>
                <h3>Local state could not be read</h3>
                <p>{error}</p>
              </div>
            </div>
          )}

          {!loading && !error && projects.length === 0 && (
            <div className="state-panel state-empty">
              <span className="empty-glyph" aria-hidden="true">⌁</span>
              <div>
                <h3>A quiet starting point.</h3>
                <p>Register a repository through Codex to see its Git snapshot here.</p>
              </div>
              <span className="empty-index">001</span>
            </div>
          )}

          {!loading && !error && projects.length > 0 && (
            <div className="project-list">
              {projects.map((project, index) => (
                <article className="project-card" key={project.id}>
                  <div className="project-card-top">
                    <div className="project-identity">
                      <span className="project-index">{String(index + 1).padStart(2, "0")}</span>
                      <div>
                        <h3>{project.name}</h3>
                        <p className="project-path" title={project.path}>{project.path}</p>
                      </div>
                    </div>
                    <span className={`status-pill ${project.dirty ? "status-changed" : "status-clean"}`}>
                      <span /> {project.dirty ? "Changes" : "Clean"}
                    </span>
                  </div>
                  <div className="project-card-bottom">
                    <div className="project-facts">
                      <span className="fact-label">BRANCH</span>
                      <span className="fact-value branch-value">{project.branch ?? "Detached HEAD"}</span>
                    </div>
                    <div className="project-facts">
                      <span className="fact-label">HEAD</span>
                      <code className="fact-value">{shortHead(project.head)}</code>
                    </div>
                    <div className="project-facts changed-fact">
                      <span className="fact-label">WORKTREE</span>
                      <span className="fact-value">
                        {project.changedPaths.length === 0
                          ? "No changed paths"
                          : `${project.changedPaths.length} changed ${project.changedPaths.length === 1 ? "path" : "paths"}`}
                      </span>
                    </div>
                    <div className="project-facts snapshot-fact">
                      <span className="fact-label">RECONCILED</span>
                      <span className="fact-value">{formatSnapshotTime(project.reconciledAt)}</span>
                    </div>
                  </div>
                  {project.changedPaths.length > 0 && (
                    <ul className="changed-paths" aria-label={`Changed paths in ${project.name}`}>
                      {project.changedPaths.slice(0, 4).map((path) => (
                        <li key={path}><code>{path}</code></li>
                      ))}
                      {project.changedPaths.length > 4 && (
                        <li className="more-paths">+{project.changedPaths.length - 4} more</li>
                      )}
                    </ul>
                  )}
                </article>
              ))}
            </div>
          )}
        </section>

        {!loading && !error && snapshot && (
          <section className="operations-section" aria-label="Workflow and memory overview">
            <div className="section-heading">
              <div>
                <p className="eyebrow">WORK IN MOTION</p>
                <h2>Objectives &amp; runs</h2>
              </div>
              <p className="section-note">Current scope, recent verification, and review items.</p>
            </div>

            <div className="operations-grid">
              <section className="overview-panel" aria-labelledby="objectives-title">
                <div className="panel-heading">
                  <h3 id="objectives-title">Active objectives</h3>
                  <span>{snapshot.objectives.length.toString().padStart(2, "0")}</span>
                </div>
                {snapshot.objectives.length === 0 ? (
                  <p className="panel-empty">No active objectives yet.</p>
                ) : (
                  <ul className="overview-list">
                    {snapshot.objectives.map((objective) => (
                      <li key={objective.id}>
                        <div><strong>{objective.title}</strong><span>{objective.projectName}</span></div>
                        <span className={`status-pill ${objective.status === "blocked" ? "status-attention" : "status-clean"}`}>{objective.status}</span>
                      </li>
                    ))}
                  </ul>
                )}
                {snapshot.blockers.length > 0 && (
                  <div className="blocker-list" aria-label="Blocked tasks">
                    <p className="fact-label">BLOCKED TASKS</p>
                    {snapshot.blockers.map((blocker) => (
                      <div className="blocker-row" key={blocker.id}>
                        <span className="blocker-mark" aria-hidden="true">!</span>
                        <span><strong>{blocker.title}</strong><small>{blocker.projectName}</small></span>
                      </div>
                    ))}
                  </div>
                )}
              </section>

              <section className="overview-panel" aria-labelledby="runs-title">
                <div className="panel-heading">
                  <h3 id="runs-title">Recent runs</h3>
                  <span>{snapshot.recentRuns.length.toString().padStart(2, "0")}</span>
                </div>
                {snapshot.recentRuns.length === 0 ? (
                  <p className="panel-empty">No workflow runs recorded.</p>
                ) : (
                  <ul className="overview-list run-list">
                    {snapshot.recentRuns.map((run) => (
                      <li key={run.id}>
                        <div>
                          <strong>{(run.workflowKey ?? run.taskTitle ?? `Run ${run.id}`).replace(/_/g, " ")}</strong>
                          <span>{run.projectName} · {run.summary || "In progress"}</span>
                        </div>
                        <span className={`status-pill ${run.status === "verified_success" ? "status-clean" : run.status === "running" ? "status-running" : "status-attention"}`}>{run.status.replace(/_/g, " ")}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section className="overview-panel quality-panel" aria-labelledby="quality-title">
                <div className="panel-heading">
                  <h3 id="quality-title">Quality watch</h3>
                  <span>{snapshot.failedChecks.length.toString().padStart(2, "0")}</span>
                </div>
                {snapshot.failedChecks.length === 0 ? (
                  <p className="panel-empty">No failed or inconclusive checks.</p>
                ) : (
                  <ul className="overview-list">
                    {snapshot.failedChecks.map((check) => (
                      <li key={check.id}>
                        <div><strong>{check.checkKey}</strong><span>{check.projectName} · run {check.runId}{check.mandatory ? " · required" : ""}</span></div>
                        <span className="status-pill status-attention">{check.status}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section className="overview-panel memory-panel" aria-labelledby="memory-title">
                <div className="panel-heading">
                  <h3 id="memory-title">Memory health</h3>
                  <span>LOCAL</span>
                </div>
                <div className="memory-stats">
                  <div><span>HOT</span><strong>{formatBytes(snapshot.memoryHealth.hotBytes)}</strong></div>
                  <div><span>HOT scopes</span><strong>{snapshot.memoryHealth.hotScopeCount}</strong></div>
                  <div><span>Largest scope HOT</span><strong>{formatBytes(snapshot.memoryHealth.largestScopeHotBytes)}</strong></div>
                  <div><span>WARM</span><strong>{formatBytes(snapshot.memoryHealth.warmBytes)}</strong></div>
                  <div><span>COLD</span><strong>{formatBytes(snapshot.memoryHealth.coldBytes)}</strong></div>
                  <div><span>Canonical</span><strong>{snapshot.memoryHealth.canonicalCount}</strong></div>
                  <div><span>Episodic</span><strong>{snapshot.memoryHealth.episodicCount}</strong></div>
                  <div><span>Review candidates</span><strong>{snapshot.memoryHealth.candidateCount}</strong></div>
                  <div><span>Invalidated</span><strong>{snapshot.memoryHealth.invalidatedCount}</strong></div>
                  <div><span>Stale or superseded</span><strong>{snapshot.memoryHealth.staleMemoryCount}</strong></div>
                  <div><span>Protected artifacts</span><strong>{snapshot.memoryHealth.protectedArtifactCount}</strong></div>
                  <div><span>SQLite storage</span><strong>{formatBytes(snapshot.memoryHealth.sqliteBytes)}</strong></div>
                  <div><span>Storage growth</span><strong>{formatSignedBytes(snapshot.memoryHealth.sqliteGrowthBytes)}</strong></div>
                  <div><span>Orphan evidence</span><strong>{snapshot.memoryHealth.orphanedEvidenceCount}</strong></div>
                  <div><span>Maintenance proposals</span><strong>{snapshot.memoryHealth.pendingMaintenanceProposals}</strong></div>
                  <div><span>Context reintroduced</span><strong>{formatBytes(snapshot.memoryHealth.contextReintroducedBytes)} · {formatBasisPoints(snapshot.memoryHealth.contextReintroductionBasisPoints)}</strong></div>
                  <div><span>Context amplification</span><strong>{formatBasisPoints(snapshot.memoryHealth.contextAmplificationBasisPoints)}</strong></div>
                  <div><span>Retrieval usefulness</span><strong>{formatBasisPoints(snapshot.memoryHealth.retrievalUsefulnessBasisPoints)}</strong></div>
                </div>
                <p className="memory-footnote">HOT memory is kept bounded per scope; detailed health remains in local SQLite.</p>
              </section>

              <section className="overview-panel delegation-panel" aria-labelledby="delegations-title">
                <div className="panel-heading">
                  <h3 id="delegations-title">Delegations &amp; worker reports</h3>
                  <span>{snapshot.delegations.length.toString().padStart(2, "0")}</span>
                </div>
                {snapshot.delegations.length === 0 ? (
                  <p className="panel-empty">No worker handoffs yet. Prepared delegation status and returned evidence will appear here.</p>
                ) : (
                  <ul className="delegation-list">
                    {snapshot.delegations.map((delegation) => {
                      const report = delegation.report;
                      const latestReview = report
                        ? report.reviewHistory[report.reviewHistory.length - 1]
                        : undefined;
                      return (
                        <li className="delegation-item" key={delegation.id}>
                          <div className="delegation-header">
                            <div className="delegation-identity">
                              <strong>{delegation.taskTitle}</strong>
                              <span>{delegation.projectName} · run {delegation.runId} · {delegation.role.replace(/_/g, " ")}</span>
                            </div>
                            <div className="delegation-statuses">
                              <span className={`status-pill ${delegation.status === "dispatched" ? "status-running" : delegation.status === "cancelled" ? "status-neutral" : "status-attention"}`}>
                                Handoff {delegation.status.replace(/_/g, " ")}
                              </span>
                              <span className={`status-pill ${delegation.parentRunStatus === "verified_success" ? "status-clean" : "status-attention"}`}>
                                Parent run {delegation.parentRunStatus.replace(/_/g, " ")}
                              </span>
                            </div>
                          </div>
                          {report ? (
                            <div className="worker-report">
                              <div className="report-status-line">
                                <span className={`status-pill status-${report.reviewStatus.replace(/_/g, "-")}`}>
                                  {report.reviewStatus === "reported" ? "Reported · awaiting review" : report.reviewStatus.replace(/_/g, " ")}
                                </span>
                                <span className="report-outcome">Worker-reported outcome: {report.report.outcome.replace(/_/g, " ")}</span>
                                <time dateTime={new Date(report.reportedAt * 1_000).toISOString()}>{formatSnapshotTime(report.reportedAt)}</time>
                              </div>
                              <p className="report-summary">{report.report.summary}</p>
                              <dl className="report-facts">
                                <div>
                                  <dt>Changed files</dt>
                                  <dd>{report.report.changedFiles.length === 0 ? "None reported" : report.report.changedFiles.map((path) => <code key={path}>{path}</code>)}</dd>
                                </div>
                                <div>
                                  <dt>Commits</dt>
                                  <dd>{report.report.commits.length === 0 ? "None reported" : report.report.commits.map((commit) => <code key={commit}>{commit.slice(0, 12)}</code>)}</dd>
                                </div>
                                <div>
                                  <dt>Linked evidence</dt>
                                  <dd>{report.report.artifactIds.length === 0 ? "No artifacts" : report.report.artifactIds.map((id) => `Artifact ${id}`).join(", ")}</dd>
                                </div>
                                <div>
                                  <dt>Parent checks linked</dt>
                                  <dd>{report.qualityCheckRefs.length === 0 ? "None" : report.qualityCheckRefs.map((check) => `${check.checkKey} · ${check.status}`).join(", ")}</dd>
                                </div>
                              </dl>
                              {report.report.checkResults.length > 0 && (
                                <ul className="report-checks" aria-label={`Worker-reported checks for ${delegation.taskTitle}`}>
                                  {report.report.checkResults.map((check) => (
                                    <li key={check.name}>
                                      <span><strong>{check.name}</strong><small>{check.summary}</small></span>
                                      <span className={`check-result status-${check.status}`}>{check.status.replace(/_/g, " ")}</span>
                                    </li>
                                  ))}
                                </ul>
                              )}
                              {report.report.blockers.length > 0 && (
                                <ul className="report-blockers" aria-label={`Worker-reported blockers for ${delegation.taskTitle}`}>
                                  {report.report.blockers.map((blocker) => <li key={blocker}>{blocker}</li>)}
                                </ul>
                              )}
                              {latestReview?.note && <p className="review-note">Latest review: {latestReview.note}</p>}
                            </div>
                          ) : (
                            <p className="delegation-report-pending">
                              {delegation.status === "dispatched" ? "Awaiting worker report." : "No report has been submitted."}
                            </p>
                          )}
                          {(delegation.worktree || delegation.workerTelemetry) && (
                            <div className="worker-report" aria-label={`Git and worker evidence for ${delegation.taskTitle}`}>
                              <dl className="report-facts">
                                {delegation.worktree && (
                                  <>
                                    <div>
                                      <dt>Observed worktree</dt>
                                      <dd>
                                        {delegation.worktree.lastReconciledAt
                                          ? `${delegation.worktree.branch ?? "detached"} · ${delegation.worktree.isClean ? "clean" : "dirty"} · ${delegation.worktree.changedPaths.length} changed paths`
                                          : `Baseline ${delegation.worktree.baseCommit?.slice(0, 12) ?? "unavailable"} · not reconciled`}
                                      </dd>
                                    </div>
                                    <div>
                                      <dt>Report Git verification</dt>
                                      <dd>{delegation.worktree.reportVerification?.status.replace(/_/g, " ") ?? "not checked"}</dd>
                                    </div>
                                  </>
                                )}
                                {delegation.workerTelemetry && (
                                  <div>
                                    <dt>Observed worker</dt>
                                    <dd>
                                      {delegation.workerTelemetry.agentType} · {delegation.workerTelemetry.effectiveModel ?? "model unavailable"} · effort {delegation.workerTelemetry.effectiveEffort ?? "unavailable"}
                                    </dd>
                                  </div>
                                )}
                              </dl>
                            </div>
                          )}
                        </li>
                      );
                    })}
                  </ul>
                )}
                <p className="delegation-footnote">Worker reports are claims for review. Parent-run verification still depends on its own mandatory checks.</p>
              </section>
            </div>
          </section>
        )}
      </main>

      <footer className="footer">
        <span><span className="footer-mark">C</span> Cathulu local evidence layer</span>
        <span>Interaction stays in Codex <span className="footer-arrow">↗</span></span>
      </footer>
    </div>
  );
}

function formatBytes(bytes: number) {
  if (bytes < 1_024) return `${bytes} B`;
  return `${(bytes / 1_024).toFixed(bytes < 10_240 ? 1 : 0)} KiB`;
}

function formatSignedBytes(bytes: number) {
  return `${bytes < 0 ? "−" : "+"}${formatBytes(Math.abs(bytes))}`;
}

function formatBasisPoints(value: number) {
  return `${(value / 100).toFixed(1)}%`;
}
