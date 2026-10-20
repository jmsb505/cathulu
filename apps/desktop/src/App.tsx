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
  const [projects, setProjects] = useState<ProjectSnapshot[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const loadSnapshot = () => {
      invoke<ProjectSnapshot[]>("project_snapshot")
        .then((snapshot) => {
          if (!cancelled) {
            setProjects(snapshot);
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
      </main>

      <footer className="footer">
        <span><span className="footer-mark">C</span> Cathulu local evidence layer</span>
        <span>Interaction stays in Codex <span className="footer-arrow">↗</span></span>
      </footer>
    </div>
  );
}
