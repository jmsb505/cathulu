pub(crate) const VERSION_2: &str = r#"
CREATE TABLE project_dependencies (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  from_project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  to_project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  relation TEXT NOT NULL DEFAULT 'depends_on',
  created_at INTEGER NOT NULL,
  UNIQUE(from_project_id, to_project_id, relation),
  CHECK(from_project_id <> to_project_id)
);

CREATE TABLE objectives (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'blocked', 'completed', 'cancelled', 'archived')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE tasks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  objective_id INTEGER NOT NULL REFERENCES objectives(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued', 'active', 'blocked', 'completed', 'failed', 'cancelled')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE decisions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  objective_id INTEGER REFERENCES objectives(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  supersedes_decision_id INTEGER REFERENCES decisions(id),
  created_at INTEGER NOT NULL,
  UNIQUE(supersedes_decision_id)
);

CREATE TABLE workflows (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  workflow_key TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE workflow_versions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  workflow_id INTEGER NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
  version INTEGER NOT NULL CHECK(version > 0),
  definition_json TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(workflow_id, version)
);

CREATE TABLE runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  objective_id INTEGER REFERENCES objectives(id) ON DELETE SET NULL,
  task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
  workflow_version_id INTEGER REFERENCES workflow_versions(id) ON DELETE SET NULL,
  status TEXT NOT NULL DEFAULT 'running' CHECK(status IN ('running', 'blocked', 'failed', 'inconclusive', 'verified_success', 'cancelled')),
  summary TEXT NOT NULL DEFAULT '',
  started_at INTEGER NOT NULL,
  finished_at INTEGER
);

CREATE TABLE quality_checks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  check_key TEXT NOT NULL,
  mandatory INTEGER NOT NULL CHECK(mandatory IN (0, 1)),
  status TEXT NOT NULL CHECK(status IN ('pending', 'passed', 'failed', 'inconclusive')),
  details_json TEXT NOT NULL DEFAULT '{}',
  updated_at INTEGER NOT NULL,
  UNIQUE(run_id, check_key)
);

CREATE TABLE evidence_artifacts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER REFERENCES runs(id) ON DELETE SET NULL,
  artifact_kind TEXT NOT NULL,
  uri TEXT NOT NULL,
  content_hash TEXT,
  protected INTEGER NOT NULL DEFAULT 0 CHECK(protected IN (0, 1)),
  created_at INTEGER NOT NULL,
  UNIQUE(uri)
);

CREATE TABLE events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  aggregate_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL DEFAULT '{}',
  created_at INTEGER NOT NULL
);

CREATE TABLE memory_scope_budgets (
  scope_key TEXT PRIMARY KEY,
  scope_type TEXT NOT NULL CHECK(scope_type IN ('global', 'project', 'objective', 'task', 'scope')),
  scope_id TEXT,
  hot_budget_bytes INTEGER NOT NULL CHECK(hot_budget_bytes >= 0),
  updated_at INTEGER NOT NULL,
  CHECK((scope_type = 'global' AND scope_id IS NULL) OR (scope_type <> 'global' AND scope_id IS NOT NULL))
);

CREATE TABLE memories (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  scope_key TEXT NOT NULL,
  scope_type TEXT NOT NULL CHECK(scope_type IN ('global', 'project', 'objective', 'task', 'scope')),
  scope_id TEXT,
  memory_class TEXT NOT NULL CHECK(memory_class IN ('working', 'canonical', 'episodic', 'evidence', 'aggregate')),
  temperature TEXT NOT NULL CHECK(temperature IN ('hot', 'warm', 'cold')),
  kind TEXT NOT NULL CHECK(kind IN ('fact', 'decision', 'constraint', 'contract', 'lesson', 'procedure', 'failure_pattern', 'summary')),
  body TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  review_state TEXT NOT NULL DEFAULT 'candidate' CHECK(review_state IN ('candidate', 'accepted', 'rejected')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  valid_from INTEGER,
  valid_until INTEGER,
  supersedes_id INTEGER REFERENCES memories(id),
  superseded_by_id INTEGER REFERENCES memories(id),
  freshness_basis_json TEXT NOT NULL DEFAULT '{}',
  freshness_repo_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
  freshness_git_commit TEXT,
  importance REAL NOT NULL DEFAULT 0.5 CHECK(importance >= 0.0 AND importance <= 1.0),
  confidence REAL NOT NULL DEFAULT 0.5 CHECK(confidence >= 0.0 AND confidence <= 1.0),
  reuse_count INTEGER NOT NULL DEFAULT 0 CHECK(reuse_count >= 0),
  last_used_at INTEGER,
  active INTEGER NOT NULL DEFAULT 0 CHECK(active IN (0, 1)),
  protected INTEGER NOT NULL DEFAULT 0 CHECK(protected IN (0, 1)),
  invalidated_at INTEGER,
  invalidation_reason TEXT,
  CHECK((scope_type = 'global' AND scope_id IS NULL) OR (scope_type <> 'global' AND scope_id IS NOT NULL)),
  CHECK(valid_from IS NULL OR valid_until IS NULL OR valid_until > valid_from)
);

CREATE TABLE memory_sources (
  memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
  source_type TEXT NOT NULL,
  source_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(memory_id, source_type, source_id)
);

CREATE TABLE memory_artifact_refs (
  memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
  artifact_id INTEGER NOT NULL REFERENCES evidence_artifacts(id) ON DELETE RESTRICT,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(memory_id, artifact_id)
);

CREATE TABLE memory_known_by (
  memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
  receiver_scope_key TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  known_at INTEGER NOT NULL,
  last_used_at INTEGER NOT NULL,
  use_count INTEGER NOT NULL DEFAULT 1 CHECK(use_count > 0),
  PRIMARY KEY(memory_id, receiver_scope_key, content_hash)
);

CREATE TABLE context_transfers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER REFERENCES runs(id) ON DELETE SET NULL,
  sender_scope_key TEXT NOT NULL,
  receiver_scope_key TEXT NOT NULL,
  memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE RESTRICT,
  content_hash TEXT NOT NULL,
  memory_class TEXT NOT NULL,
  temperature TEXT NOT NULL,
  size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
  token_count INTEGER CHECK(token_count IS NULL OR token_count >= 0),
  first_time_for_receiver INTEGER NOT NULL CHECK(first_time_for_receiver IN (0, 1)),
  reintroduced INTEGER NOT NULL CHECK(reintroduced IN (0, 1)),
  retrieval_method TEXT NOT NULL,
  used_downstream INTEGER CHECK(used_downstream IS NULL OR used_downstream IN (0, 1)),
  transferred_at INTEGER NOT NULL
);

CREATE INDEX events_aggregate_idx ON events(aggregate_type, aggregate_id, id);
CREATE INDEX objectives_status_idx ON objectives(status, updated_at DESC);
CREATE INDEX tasks_status_idx ON tasks(status, updated_at DESC);
CREATE INDEX runs_recent_idx ON runs(started_at DESC);
CREATE INDEX quality_checks_status_idx ON quality_checks(status, mandatory, updated_at DESC);
CREATE INDEX memories_scope_state_idx ON memories(scope_key, temperature, review_state, updated_at DESC);
CREATE INDEX memories_freshness_idx ON memories(freshness_repo_id, freshness_git_commit);
CREATE UNIQUE INDEX memories_current_dedupe_idx ON memories(scope_key, kind, content_hash)
  WHERE superseded_by_id IS NULL AND invalidated_at IS NULL AND valid_until IS NULL;
CREATE UNIQUE INDEX memories_single_successor_idx ON memories(supersedes_id)
  WHERE supersedes_id IS NOT NULL;
CREATE INDEX context_transfers_receiver_idx ON context_transfers(receiver_scope_key, transferred_at DESC);

CREATE TRIGGER events_no_update BEFORE UPDATE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;
CREATE TRIGGER events_no_delete BEFORE DELETE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;
CREATE TRIGGER decisions_no_update BEFORE UPDATE ON decisions
BEGIN SELECT RAISE(ABORT, 'decisions are immutable'); END;
CREATE TRIGGER decisions_no_delete BEFORE DELETE ON decisions
BEGIN SELECT RAISE(ABORT, 'decisions are immutable'); END;
CREATE TRIGGER workflow_versions_no_update BEFORE UPDATE ON workflow_versions
BEGIN SELECT RAISE(ABORT, 'workflow versions are immutable'); END;
CREATE TRIGGER workflow_versions_no_delete BEFORE DELETE ON workflow_versions
BEGIN SELECT RAISE(ABORT, 'workflow versions are immutable'); END;
CREATE TRIGGER artifacts_cannot_unprotect BEFORE UPDATE OF protected ON evidence_artifacts
WHEN OLD.protected = 1 AND NEW.protected = 0
BEGIN SELECT RAISE(ABORT, 'protected evidence cannot be automatically unprotected'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (2, CAST(strftime('%s','now') AS INTEGER));
"#;
