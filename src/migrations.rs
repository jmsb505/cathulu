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

pub(crate) const VERSION_3: &str = r#"
ALTER TABLE events ADD COLUMN external_event_id TEXT;
CREATE UNIQUE INDEX events_external_event_id_idx ON events(external_event_id);

CREATE TABLE codex_turn_runs (
  session_hash TEXT NOT NULL,
  turn_hash TEXT NOT NULL,
  run_id INTEGER NOT NULL UNIQUE REFERENCES runs(id) ON DELETE CASCADE,
  last_seen_at INTEGER NOT NULL,
  PRIMARY KEY(session_hash, turn_hash)
);

INSERT INTO schema_migrations(version, applied_at)
VALUES (3, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_4: &str = r#"
ALTER TABLE evidence_artifacts ADD COLUMN archived_at INTEGER;
ALTER TABLE evidence_artifacts ADD COLUMN uri_hash TEXT;

CREATE VIRTUAL TABLE memory_fts USING fts5(body, kind, tokenize='unicode61');
CREATE TRIGGER memories_fts_after_insert AFTER INSERT ON memories
WHEN new.review_state = 'accepted' AND new.memory_class IN ('canonical', 'episodic')
  AND new.temperature = 'warm' AND new.superseded_by_id IS NULL AND new.invalidated_at IS NULL
BEGIN
  INSERT INTO memory_fts(rowid, body, kind) VALUES (new.id, new.body, new.kind);
END;
CREATE TRIGGER memories_fts_after_delete AFTER DELETE ON memories
BEGIN
  DELETE FROM memory_fts WHERE rowid = old.id;
END;
CREATE TRIGGER memories_fts_after_update
AFTER UPDATE OF body, kind, memory_class, temperature, review_state, superseded_by_id, invalidated_at
ON memories
BEGIN
  DELETE FROM memory_fts WHERE rowid = old.id;
  INSERT INTO memory_fts(rowid, body, kind)
  SELECT new.id, new.body, new.kind
  WHERE new.review_state = 'accepted' AND new.memory_class IN ('canonical', 'episodic')
    AND new.temperature = 'warm' AND new.superseded_by_id IS NULL AND new.invalidated_at IS NULL;
END;
INSERT INTO memory_fts(rowid, body, kind)
SELECT id, body, kind FROM memories
WHERE review_state = 'accepted' AND memory_class IN ('canonical', 'episodic')
  AND temperature = 'warm' AND superseded_by_id IS NULL AND invalidated_at IS NULL;

CREATE TABLE memory_maintenance_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  started_at INTEGER NOT NULL,
  finished_at INTEGER NOT NULL,
  proposal_count INTEGER NOT NULL CHECK(proposal_count >= 0),
  findings_json TEXT NOT NULL,
  sqlite_bytes INTEGER NOT NULL CHECK(sqlite_bytes >= 0)
);

CREATE TABLE memory_maintenance_proposals (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  maintenance_run_id INTEGER NOT NULL REFERENCES memory_maintenance_runs(id) ON DELETE CASCADE,
  proposal_type TEXT NOT NULL,
  subject_ids_json TEXT NOT NULL,
  reason TEXT NOT NULL,
  minimum_support INTEGER CHECK(minimum_support IS NULL OR minimum_support BETWEEN 2 AND 100),
  status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed', 'accepted', 'rejected')),
  created_at INTEGER NOT NULL,
  reviewed_at INTEGER
);
CREATE INDEX memory_proposals_run_idx ON memory_maintenance_proposals(maintenance_run_id, id);

CREATE TABLE memory_retention_policy (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  minimum_age_seconds INTEGER CHECK(minimum_age_seconds IS NULL OR minimum_age_seconds >= 0),
  require_extraction INTEGER NOT NULL DEFAULT 1 CHECK(require_extraction IN (0, 1)),
  updated_at INTEGER NOT NULL
);
INSERT INTO memory_retention_policy(singleton_id, minimum_age_seconds, require_extraction, updated_at)
VALUES (1, NULL, 1, CAST(strftime('%s','now') AS INTEGER));

CREATE TABLE evidence_extraction_receipts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  artifact_id INTEGER NOT NULL,
  artifact_content_hash TEXT,
  artifact_uri_hash TEXT NOT NULL,
  preserved_memories_json TEXT NOT NULL,
  extracted_at INTEGER NOT NULL
);
CREATE INDEX evidence_extraction_artifact_idx ON evidence_extraction_receipts(artifact_id, extracted_at DESC);

CREATE TABLE evidence_holds (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  artifact_id INTEGER NOT NULL REFERENCES evidence_artifacts(id) ON DELETE RESTRICT,
  reason TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  released_at INTEGER
);
CREATE INDEX evidence_holds_active_idx ON evidence_holds(artifact_id, released_at);

CREATE TABLE evidence_gc_ledger (
  artifact_id INTEGER PRIMARY KEY,
  run_id INTEGER,
  artifact_kind TEXT NOT NULL,
  content_hash TEXT,
  uri_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  collected_at INTEGER NOT NULL,
  extraction_receipt_id INTEGER,
  logical_bytes_collected INTEGER NOT NULL CHECK(logical_bytes_collected >= 0)
);

CREATE TABLE memory_policies (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  policy_key TEXT NOT NULL,
  version INTEGER NOT NULL CHECK(version > 0),
  retrieval_method TEXT NOT NULL CHECK(retrieval_method IN ('sql_fts')),
  hot_budget_bytes INTEGER NOT NULL CHECK(hot_budget_bytes >= 0),
  warm_candidate_limit INTEGER NOT NULL CHECK(warm_candidate_limit BETWEEN 1 AND 50),
  minimum_promotion_support INTEGER NOT NULL CHECK(minimum_promotion_support BETWEEN 2 AND 100),
  retention_minimum_age_seconds INTEGER CHECK(retention_minimum_age_seconds IS NULL OR retention_minimum_age_seconds >= 0),
  created_at INTEGER NOT NULL,
  UNIQUE(policy_key, version)
);

CREATE TABLE memory_policy_evaluations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  policy_id INTEGER NOT NULL REFERENCES memory_policies(id) ON DELETE RESTRICT,
  experiment_key TEXT NOT NULL,
  task_type TEXT NOT NULL,
  run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
  missing_context_failures INTEGER NOT NULL CHECK(missing_context_failures >= 0),
  regressions INTEGER NOT NULL CHECK(regressions >= 0),
  rework_count INTEGER NOT NULL CHECK(rework_count >= 0),
  context_bytes INTEGER NOT NULL CHECK(context_bytes >= 0),
  reintroduced_bytes INTEGER NOT NULL CHECK(reintroduced_bytes >= 0),
  useful_retrievals INTEGER NOT NULL CHECK(useful_retrievals >= 0),
  retrieval_count INTEGER NOT NULL CHECK(retrieval_count >= useful_retrievals),
  latency_ms INTEGER CHECK(latency_ms IS NULL OR latency_ms >= 0),
  human_interventions INTEGER NOT NULL CHECK(human_interventions >= 0),
  created_at INTEGER NOT NULL,
  UNIQUE(policy_id, run_id)
);
CREATE INDEX memory_policy_experiment_idx ON memory_policy_evaluations(experiment_key, task_type, policy_id);

CREATE TABLE memory_storage_snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER UNIQUE REFERENCES runs(id) ON DELETE SET NULL,
  project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
  workflow_version_id INTEGER REFERENCES workflow_versions(id) ON DELETE SET NULL,
  sampled_at INTEGER NOT NULL,
  sqlite_bytes INTEGER NOT NULL CHECK(sqlite_bytes >= 0),
  growth_bytes INTEGER NOT NULL,
  hot_bytes INTEGER NOT NULL CHECK(hot_bytes >= 0),
  warm_bytes INTEGER NOT NULL CHECK(warm_bytes >= 0),
  cold_bytes INTEGER NOT NULL CHECK(cold_bytes >= 0),
  artifact_reference_bytes INTEGER NOT NULL CHECK(artifact_reference_bytes >= 0),
  orphaned_evidence_count INTEGER NOT NULL CHECK(orphaned_evidence_count >= 0),
  duplicate_artifact_groups INTEGER NOT NULL CHECK(duplicate_artifact_groups >= 0),
  bytes_compacted INTEGER NOT NULL DEFAULT 0 CHECK(bytes_compacted >= 0),
  bytes_collected INTEGER NOT NULL DEFAULT 0 CHECK(bytes_collected >= 0)
);
CREATE INDEX memory_storage_project_idx ON memory_storage_snapshots(project_id, sampled_at DESC);
CREATE INDEX memory_storage_workflow_idx ON memory_storage_snapshots(workflow_version_id, sampled_at DESC);

CREATE TRIGGER memory_policies_no_update BEFORE UPDATE ON memory_policies
BEGIN SELECT RAISE(ABORT, 'memory policy versions are immutable'); END;
CREATE TRIGGER memory_policies_no_delete BEFORE DELETE ON memory_policies
BEGIN SELECT RAISE(ABORT, 'memory policy versions are immutable'); END;
CREATE TRIGGER memory_policy_evaluations_no_update BEFORE UPDATE ON memory_policy_evaluations
BEGIN SELECT RAISE(ABORT, 'memory policy evaluations are immutable'); END;
CREATE TRIGGER memory_policy_evaluations_no_delete BEFORE DELETE ON memory_policy_evaluations
BEGIN SELECT RAISE(ABORT, 'memory policy evaluations are immutable'); END;
CREATE TRIGGER memory_maintenance_runs_no_update BEFORE UPDATE ON memory_maintenance_runs
BEGIN SELECT RAISE(ABORT, 'memory maintenance runs are immutable'); END;
CREATE TRIGGER memory_maintenance_runs_no_delete BEFORE DELETE ON memory_maintenance_runs
BEGIN SELECT RAISE(ABORT, 'memory maintenance runs are immutable'); END;
CREATE TRIGGER memory_maintenance_proposals_no_delete BEFORE DELETE ON memory_maintenance_proposals
BEGIN SELECT RAISE(ABORT, 'memory maintenance proposals are retained for audit'); END;
CREATE TRIGGER memory_maintenance_proposals_review_once BEFORE UPDATE ON memory_maintenance_proposals
WHEN OLD.status <> 'proposed' OR NEW.status NOT IN ('accepted', 'rejected') OR NEW.reviewed_at IS NULL
  OR NEW.id <> OLD.id OR NEW.maintenance_run_id <> OLD.maintenance_run_id
  OR NEW.proposal_type <> OLD.proposal_type OR NEW.subject_ids_json <> OLD.subject_ids_json
  OR NEW.reason <> OLD.reason OR NEW.minimum_support IS NOT OLD.minimum_support
  OR NEW.created_at <> OLD.created_at
BEGIN SELECT RAISE(ABORT, 'maintenance proposals can be reviewed only once'); END;
CREATE TRIGGER evidence_extraction_receipts_no_update BEFORE UPDATE ON evidence_extraction_receipts
BEGIN SELECT RAISE(ABORT, 'evidence extraction receipts are immutable'); END;
CREATE TRIGGER evidence_extraction_receipts_no_delete BEFORE DELETE ON evidence_extraction_receipts
BEGIN SELECT RAISE(ABORT, 'evidence extraction receipts are immutable'); END;
CREATE TRIGGER evidence_gc_ledger_no_update BEFORE UPDATE ON evidence_gc_ledger
BEGIN SELECT RAISE(ABORT, 'evidence collection ledger is immutable'); END;
CREATE TRIGGER evidence_gc_ledger_no_delete BEFORE DELETE ON evidence_gc_ledger
BEGIN SELECT RAISE(ABORT, 'evidence collection ledger is immutable'); END;
CREATE TRIGGER evidence_holds_no_delete BEFORE DELETE ON evidence_holds
BEGIN SELECT RAISE(ABORT, 'evidence holds are retained for audit'); END;
CREATE TRIGGER evidence_holds_release_once BEFORE UPDATE ON evidence_holds
WHEN OLD.released_at IS NOT NULL OR NEW.released_at IS NULL
  OR NEW.id <> OLD.id OR NEW.artifact_id <> OLD.artifact_id
  OR NEW.reason <> OLD.reason OR NEW.created_at <> OLD.created_at
BEGIN SELECT RAISE(ABORT, 'evidence holds may be released only once'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (4, CAST(strftime('%s','now') AS INTEGER));
"#;
