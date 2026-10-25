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

pub(crate) const VERSION_5: &str = r#"
CREATE TABLE delegations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
  run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
  role TEXT NOT NULL CHECK(role IN ('worker', 'sub_overseer')),
  status TEXT NOT NULL DEFAULT 'prepared' CHECK(status IN ('prepared', 'dispatched', 'cancelled')),
  packet_schema_version INTEGER NOT NULL CHECK(packet_schema_version = 1),
  packet_json TEXT NOT NULL CHECK(length(packet_json) <= 16384),
  packet_hash TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  requested_model TEXT,
  requested_effort TEXT,
  external_correlation_hash TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX delegations_parent_run_idx ON delegations(run_id, created_at DESC);
CREATE INDEX delegations_task_idx ON delegations(task_id, created_at DESC);

CREATE TABLE delegation_memory_refs (
  delegation_id INTEGER NOT NULL REFERENCES delegations(id) ON DELETE RESTRICT,
  memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE RESTRICT,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(delegation_id, memory_id)
);
CREATE INDEX delegation_memory_refs_memory_idx ON delegation_memory_refs(memory_id);

CREATE TABLE delegation_artifact_refs (
  delegation_id INTEGER NOT NULL REFERENCES delegations(id) ON DELETE RESTRICT,
  artifact_id INTEGER NOT NULL REFERENCES evidence_artifacts(id) ON DELETE RESTRICT,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(delegation_id, artifact_id)
);
CREATE INDEX delegation_artifact_refs_artifact_idx ON delegation_artifact_refs(artifact_id);

CREATE TRIGGER delegations_update_guard BEFORE UPDATE ON delegations
WHEN NEW.id IS NOT OLD.id OR NEW.project_id IS NOT OLD.project_id
  OR NEW.task_id IS NOT OLD.task_id OR NEW.run_id IS NOT OLD.run_id
  OR NEW.role IS NOT OLD.role OR NEW.packet_schema_version IS NOT OLD.packet_schema_version
  OR NEW.packet_json IS NOT OLD.packet_json OR NEW.packet_hash IS NOT OLD.packet_hash
  OR NEW.request_hash IS NOT OLD.request_hash
  OR NEW.idempotency_key_hash IS NOT OLD.idempotency_key_hash
  OR NEW.requested_model IS NOT OLD.requested_model
  OR NEW.requested_effort IS NOT OLD.requested_effort
  OR NEW.external_correlation_hash IS NOT OLD.external_correlation_hash
  OR NEW.created_at IS NOT OLD.created_at OR NEW.updated_at < OLD.updated_at
  OR NOT ((OLD.status = 'prepared' AND NEW.status IN ('dispatched', 'cancelled'))
          OR (OLD.status = 'dispatched' AND NEW.status = 'cancelled'))
BEGIN SELECT RAISE(ABORT, 'delegation identity is immutable and lifecycle transitions are restricted'); END;
CREATE TRIGGER delegations_no_delete BEFORE DELETE ON delegations
BEGIN SELECT RAISE(ABORT, 'delegation records are retained for audit'); END;
CREATE TRIGGER delegation_memory_refs_no_update BEFORE UPDATE ON delegation_memory_refs
BEGIN SELECT RAISE(ABORT, 'delegation memory references are immutable'); END;
CREATE TRIGGER delegation_memory_refs_no_delete BEFORE DELETE ON delegation_memory_refs
BEGIN SELECT RAISE(ABORT, 'delegation memory references are retained for audit'); END;
CREATE TRIGGER delegation_artifact_refs_no_update BEFORE UPDATE ON delegation_artifact_refs
BEGIN SELECT RAISE(ABORT, 'delegation artifact references are immutable'); END;
CREATE TRIGGER delegation_artifact_refs_no_delete BEFORE DELETE ON delegation_artifact_refs
BEGIN SELECT RAISE(ABORT, 'delegation artifact references are retained for audit'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (5, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_6: &str = r#"
CREATE TABLE worker_reports (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  delegation_id INTEGER NOT NULL REFERENCES delegations(id) ON DELETE RESTRICT,
  schema_version INTEGER NOT NULL CHECK(schema_version = 1),
  report_json TEXT NOT NULL CHECK(length(CAST(report_json AS BLOB)) <= 16384),
  report_hash TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  reported_at INTEGER NOT NULL
);
CREATE INDEX worker_reports_delegation_idx ON worker_reports(delegation_id, reported_at DESC);

CREATE TABLE worker_report_artifact_refs (
  report_id INTEGER NOT NULL REFERENCES worker_reports(id) ON DELETE RESTRICT,
  artifact_id INTEGER NOT NULL REFERENCES evidence_artifacts(id) ON DELETE RESTRICT,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(report_id, artifact_id)
);
CREATE INDEX worker_report_artifact_refs_artifact_idx ON worker_report_artifact_refs(artifact_id);

CREATE TABLE worker_report_quality_check_refs (
  report_id INTEGER NOT NULL REFERENCES worker_reports(id) ON DELETE RESTRICT,
  quality_check_id INTEGER NOT NULL REFERENCES quality_checks(id) ON DELETE RESTRICT,
  check_key_snapshot TEXT NOT NULL,
  mandatory_snapshot INTEGER NOT NULL CHECK(mandatory_snapshot IN (0, 1)),
  status_snapshot TEXT NOT NULL CHECK(status_snapshot IN ('pending', 'passed', 'failed', 'inconclusive')),
  details_hash_snapshot TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(report_id, quality_check_id)
);
CREATE INDEX worker_report_quality_check_refs_check_idx ON worker_report_quality_check_refs(quality_check_id);

CREATE TABLE worker_report_reviews (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  report_id INTEGER NOT NULL REFERENCES worker_reports(id) ON DELETE RESTRICT,
  decision TEXT NOT NULL CHECK(decision IN ('accepted', 'rejected', 'needs_follow_up')),
  note TEXT NOT NULL DEFAULT '' CHECK(length(CAST(note AS BLOB)) <= 2000),
  request_hash TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL
);
CREATE INDEX worker_report_reviews_report_idx ON worker_report_reviews(report_id, id DESC);

CREATE TRIGGER worker_reports_no_update BEFORE UPDATE ON worker_reports
BEGIN SELECT RAISE(ABORT, 'worker reports are immutable'); END;
CREATE TRIGGER worker_reports_no_delete BEFORE DELETE ON worker_reports
BEGIN SELECT RAISE(ABORT, 'worker reports are retained for audit'); END;
CREATE TRIGGER worker_report_artifact_refs_no_update BEFORE UPDATE ON worker_report_artifact_refs
BEGIN SELECT RAISE(ABORT, 'worker report artifact references are immutable'); END;
CREATE TRIGGER worker_report_artifact_refs_no_delete BEFORE DELETE ON worker_report_artifact_refs
BEGIN SELECT RAISE(ABORT, 'worker report artifact references are retained for audit'); END;
CREATE TRIGGER worker_report_quality_check_refs_no_update BEFORE UPDATE ON worker_report_quality_check_refs
BEGIN SELECT RAISE(ABORT, 'worker report quality-check references are immutable'); END;
CREATE TRIGGER worker_report_quality_check_refs_no_delete BEFORE DELETE ON worker_report_quality_check_refs
BEGIN SELECT RAISE(ABORT, 'worker report quality-check references are retained for audit'); END;
CREATE TRIGGER worker_report_reviews_no_update BEFORE UPDATE ON worker_report_reviews
BEGIN SELECT RAISE(ABORT, 'worker report review history is append-only'); END;
CREATE TRIGGER worker_report_reviews_no_delete BEFORE DELETE ON worker_report_reviews
BEGIN SELECT RAISE(ABORT, 'worker report review history is retained for audit'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (6, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_7: &str = r#"
CREATE TABLE delegation_worktree_states (
  delegation_id INTEGER PRIMARY KEY REFERENCES delegations(id) ON DELETE RESTRICT,
  base_commit TEXT,
  worktree_path TEXT,
  worktree_path_hash TEXT,
  branch TEXT,
  observed_head TEXT,
  is_clean INTEGER CHECK(is_clean IN (0, 1)),
  changed_paths_json TEXT,
  report_verification_json TEXT,
  last_reconciled_at INTEGER,
  created_at INTEGER NOT NULL,
  CHECK(
    (worktree_path IS NULL AND worktree_path_hash IS NULL AND branch IS NULL
      AND observed_head IS NULL AND is_clean IS NULL AND changed_paths_json IS NULL
      AND report_verification_json IS NULL AND last_reconciled_at IS NULL)
    OR
    (worktree_path IS NOT NULL AND worktree_path_hash IS NOT NULL AND is_clean IS NOT NULL
      AND changed_paths_json IS NOT NULL AND report_verification_json IS NOT NULL
      AND last_reconciled_at IS NOT NULL)
  )
);

CREATE TABLE codex_subagent_hook_events (
  event_id INTEGER PRIMARY KEY REFERENCES events(id) ON DELETE RESTRICT,
  run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
  event_kind TEXT NOT NULL CHECK(event_kind IN ('started', 'stopped')),
  session_hash TEXT NOT NULL,
  turn_hash TEXT NOT NULL,
  agent_id_hash TEXT NOT NULL,
  agent_type TEXT NOT NULL,
  model_slug TEXT,
  created_at INTEGER NOT NULL
);
CREATE INDEX codex_subagent_hooks_run_idx
  ON codex_subagent_hook_events(run_id, session_hash, turn_hash, agent_id_hash, event_kind);

CREATE TABLE delegation_worker_bindings (
  delegation_id INTEGER PRIMARY KEY REFERENCES delegations(id) ON DELETE RESTRICT,
  hook_event_id INTEGER NOT NULL UNIQUE REFERENCES codex_subagent_hook_events(event_id) ON DELETE RESTRICT,
  session_hash TEXT NOT NULL,
  turn_hash TEXT NOT NULL,
  agent_id_hash TEXT NOT NULL,
  agent_type TEXT NOT NULL,
  effective_model TEXT,
  effective_effort TEXT,
  request_hash TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  linked_at INTEGER NOT NULL,
  UNIQUE(session_hash, turn_hash, agent_id_hash)
);
CREATE INDEX delegation_worker_bindings_agent_idx
  ON delegation_worker_bindings(session_hash, turn_hash, agent_id_hash);

CREATE TABLE delegation_worker_event_refs (
  delegation_id INTEGER NOT NULL REFERENCES delegations(id) ON DELETE RESTRICT,
  event_id INTEGER NOT NULL UNIQUE REFERENCES codex_subagent_hook_events(event_id) ON DELETE RESTRICT,
  event_kind TEXT NOT NULL CHECK(event_kind IN ('started', 'stopped')),
  created_at INTEGER NOT NULL,
  PRIMARY KEY(delegation_id, event_id)
);

CREATE TRIGGER delegation_worktree_base_immutable BEFORE UPDATE ON delegation_worktree_states
WHEN NEW.delegation_id IS NOT OLD.delegation_id
  OR NEW.base_commit IS NOT OLD.base_commit
  OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'delegation worktree baseline is immutable'); END;
CREATE TRIGGER delegation_worktree_no_delete BEFORE DELETE ON delegation_worktree_states
BEGIN SELECT RAISE(ABORT, 'delegation worktree state is retained for audit'); END;

CREATE TRIGGER codex_subagent_hook_events_no_update BEFORE UPDATE ON codex_subagent_hook_events
BEGIN SELECT RAISE(ABORT, 'Codex subagent hook metadata is immutable'); END;
CREATE TRIGGER codex_subagent_hook_events_no_delete BEFORE DELETE ON codex_subagent_hook_events
BEGIN SELECT RAISE(ABORT, 'Codex subagent hook metadata is retained for audit'); END;

CREATE TRIGGER delegation_worker_bindings_no_update BEFORE UPDATE ON delegation_worker_bindings
BEGIN SELECT RAISE(ABORT, 'delegation worker bindings are immutable'); END;
CREATE TRIGGER delegation_worker_bindings_no_delete BEFORE DELETE ON delegation_worker_bindings
BEGIN SELECT RAISE(ABORT, 'delegation worker bindings are retained for audit'); END;
CREATE TRIGGER delegation_worker_event_refs_no_update BEFORE UPDATE ON delegation_worker_event_refs
BEGIN SELECT RAISE(ABORT, 'delegation worker event references are immutable'); END;
CREATE TRIGGER delegation_worker_event_refs_no_delete BEFORE DELETE ON delegation_worker_event_refs
BEGIN SELECT RAISE(ABORT, 'delegation worker event references are retained for audit'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (7, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_8: &str = r#"
CREATE TABLE codex_selector_preferences (
  role_id TEXT PRIMARY KEY,
  role_kind TEXT NOT NULL CHECK(role_kind IN ('main_overseer', 'subagent_default', 'named_agent')),
  agent_name TEXT,
  model_slug TEXT NOT NULL,
  reasoning_effort TEXT NOT NULL,
  catalog_version TEXT NOT NULL,
  last_applied_target TEXT,
  last_applied_at INTEGER,
  last_applied_config_hash TEXT,
  updated_at INTEGER NOT NULL,
  CHECK(
    (role_kind IN ('main_overseer', 'subagent_default') AND agent_name IS NULL)
    OR (role_kind = 'named_agent' AND agent_name IS NOT NULL AND length(agent_name) BETWEEN 1 AND 80)
  ),
  CHECK(
    (last_applied_target IS NULL AND last_applied_at IS NULL AND last_applied_config_hash IS NULL)
    OR (last_applied_target IS NOT NULL AND last_applied_at IS NOT NULL AND last_applied_config_hash IS NOT NULL)
  )
);

CREATE INDEX codex_selector_preferences_role_idx ON codex_selector_preferences(role_kind, role_id);

INSERT INTO schema_migrations(version, applied_at)
VALUES (8, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_9: &str = r#"
CREATE TABLE project_graphify_snapshots (
  project_id INTEGER PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
  snapshot_id TEXT NOT NULL CHECK(length(snapshot_id) = 64),
  content_hash TEXT NOT NULL CHECK(length(content_hash) = 64),
  source_commit TEXT NOT NULL,
  schema_version INTEGER NOT NULL CHECK(schema_version > 0),
  graphify_version TEXT NOT NULL,
  source_files_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL CHECK(status IN ('available', 'stale', 'unavailable')),
  status_reason TEXT,
  validated_at INTEGER NOT NULL,
  CHECK((status = 'available' AND status_reason IS NULL)
     OR (status <> 'available' AND status_reason IS NOT NULL))
);

CREATE INDEX project_graphify_snapshots_status_idx
  ON project_graphify_snapshots(status, validated_at DESC);

INSERT INTO schema_migrations(version, applied_at)
VALUES (9, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_10: &str = r#"
ALTER TABLE delegations
  ADD COLUMN parent_delegation_id INTEGER REFERENCES delegations(id) ON DELETE RESTRICT;

CREATE INDEX delegations_parent_idx
  ON delegations(parent_delegation_id, created_at DESC)
  WHERE parent_delegation_id IS NOT NULL;

CREATE TRIGGER delegations_parent_immutable BEFORE UPDATE OF parent_delegation_id ON delegations
WHEN NEW.parent_delegation_id IS NOT OLD.parent_delegation_id
BEGIN SELECT RAISE(ABORT, 'delegation parent is immutable'); END;

CREATE TABLE project_manager_assignments (
  manager_delegation_id INTEGER PRIMARY KEY REFERENCES delegations(id) ON DELETE RESTRICT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
  run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
  status TEXT NOT NULL CHECK(status IN ('active', 'completed', 'cancelled')),
  created_at INTEGER NOT NULL,
  closed_at INTEGER,
  CHECK((status = 'active' AND closed_at IS NULL)
     OR (status IN ('completed', 'cancelled') AND closed_at IS NOT NULL))
);

CREATE UNIQUE INDEX project_manager_one_active_per_project
  ON project_manager_assignments(project_id) WHERE status = 'active';

CREATE INDEX project_manager_assignments_run_idx
  ON project_manager_assignments(run_id, status, manager_delegation_id);

CREATE TRIGGER project_manager_assignment_identity_immutable
BEFORE UPDATE OF manager_delegation_id, project_id, task_id, run_id ON project_manager_assignments
WHEN NEW.manager_delegation_id IS NOT OLD.manager_delegation_id
  OR NEW.project_id IS NOT OLD.project_id
  OR NEW.task_id IS NOT OLD.task_id
  OR NEW.run_id IS NOT OLD.run_id
BEGIN SELECT RAISE(ABORT, 'Project Manager assignment identity is immutable'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (10, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_11: &str = r#"
CREATE TABLE delegation_activation_receipts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  delegation_id INTEGER NOT NULL REFERENCES delegations(id) ON DELETE RESTRICT,
  revision INTEGER NOT NULL CHECK(revision > 0),
  retrieval_event_id INTEGER REFERENCES events(id) ON DELETE RESTRICT,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  request_hash TEXT NOT NULL,
  receiver_scope_key TEXT NOT NULL,
  disposition TEXT NOT NULL CHECK(disposition IN (
    'context_selected', 'no_relevant_memory', 'memory_tools_unavailable', 'retrieval_incomplete'
  )),
  status TEXT NOT NULL CHECK(status IN ('ready', 'incomplete')),
  memory_tools_available INTEGER NOT NULL CHECK(memory_tools_available IN (0, 1)),
  retrieval_completed INTEGER NOT NULL CHECK(retrieval_completed IN (0, 1)),
  created_at INTEGER NOT NULL,
  UNIQUE(delegation_id, revision),
  CHECK(
    (status = 'ready' AND memory_tools_available = 1 AND retrieval_completed = 1
      AND retrieval_event_id IS NOT NULL
      AND disposition IN ('context_selected', 'no_relevant_memory'))
    OR (status = 'incomplete' AND disposition = 'memory_tools_unavailable'
      AND memory_tools_available = 0 AND retrieval_completed = 0
      AND retrieval_event_id IS NULL)
    OR (status = 'incomplete' AND disposition = 'retrieval_incomplete'
      AND memory_tools_available = 1 AND retrieval_completed = 0
      AND retrieval_event_id IS NULL)
  )
);

CREATE TABLE delegation_activation_memory_refs (
  activation_id INTEGER NOT NULL REFERENCES delegation_activation_receipts(id) ON DELETE RESTRICT,
  memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE RESTRICT,
  content_hash TEXT NOT NULL,
  delivery TEXT NOT NULL CHECK(delivery IN ('transferred', 'already_known')),
  transfer_id INTEGER REFERENCES context_transfers(id) ON DELETE RESTRICT,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(activation_id, memory_id),
  CHECK((delivery = 'transferred' AND transfer_id IS NOT NULL)
     OR (delivery = 'already_known' AND transfer_id IS NULL))
);

CREATE INDEX delegation_activation_receipts_latest_idx
  ON delegation_activation_receipts(delegation_id, revision DESC);

CREATE TRIGGER delegation_activation_receipts_no_update BEFORE UPDATE ON delegation_activation_receipts
BEGIN SELECT RAISE(ABORT, 'delegation activation receipts are append-only'); END;
CREATE TRIGGER delegation_activation_receipts_no_delete BEFORE DELETE ON delegation_activation_receipts
BEGIN SELECT RAISE(ABORT, 'delegation activation receipts are append-only'); END;
CREATE TRIGGER delegation_activation_memory_refs_no_update BEFORE UPDATE ON delegation_activation_memory_refs
BEGIN SELECT RAISE(ABORT, 'delegation activation memory references are append-only'); END;
CREATE TRIGGER delegation_activation_memory_refs_no_delete BEFORE DELETE ON delegation_activation_memory_refs
BEGIN SELECT RAISE(ABORT, 'delegation activation memory references are append-only'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (11, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_12: &str = r#"
CREATE TABLE logical_agent_roles (
  role_key TEXT PRIMARY KEY,
  role_kind TEXT NOT NULL CHECK(role_kind IN ('main_overseer', 'project_manager')),
  project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
  memory_scope_key TEXT NOT NULL,
  receiver_scope_key TEXT NOT NULL UNIQUE,
  tool_profile_key TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  CHECK(
    (role_kind = 'main_overseer'
      AND role_key = 'main-overseer'
      AND project_id IS NULL
      AND memory_scope_key = 'global'
      AND receiver_scope_key = 'scope:main-overseer'
      AND tool_profile_key GLOB 'main-overseer-v[1-9]*')
    OR
    (role_kind = 'project_manager'
      AND project_id IS NOT NULL
      AND role_key = 'project-manager:' || project_id
      AND memory_scope_key = 'project:' || project_id
      AND receiver_scope_key = 'scope:project-manager:' || project_id
      AND tool_profile_key GLOB 'project-manager-v[1-9]*')
  )
);

CREATE UNIQUE INDEX logical_agent_single_main_idx
  ON logical_agent_roles(role_kind) WHERE role_kind = 'main_overseer';
CREATE UNIQUE INDEX logical_agent_one_pm_per_project_idx
  ON logical_agent_roles(project_id) WHERE role_kind = 'project_manager';

INSERT INTO logical_agent_roles
  (role_key, role_kind, project_id, memory_scope_key, receiver_scope_key, tool_profile_key, created_at)
VALUES
  ('main-overseer', 'main_overseer', NULL, 'global', 'scope:main-overseer', 'main-overseer-v1',
   CAST(strftime('%s','now') AS INTEGER));

INSERT INTO logical_agent_roles
  (role_key, role_kind, project_id, memory_scope_key, receiver_scope_key, tool_profile_key, created_at)
SELECT 'project-manager:' || id, 'project_manager', id, 'project:' || id,
       'scope:project-manager:' || id, 'project-manager-v1', registered_at
FROM projects;

CREATE TRIGGER logical_agent_role_identity_immutable
BEFORE UPDATE OF role_key, role_kind, project_id, memory_scope_key, receiver_scope_key,
                 created_at ON logical_agent_roles
WHEN NEW.role_key IS NOT OLD.role_key
  OR NEW.role_kind IS NOT OLD.role_kind
  OR NEW.project_id IS NOT OLD.project_id
  OR NEW.memory_scope_key IS NOT OLD.memory_scope_key
  OR NEW.receiver_scope_key IS NOT OLD.receiver_scope_key
  OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'logical agent role identity and memory scope are immutable'); END;

CREATE TRIGGER projects_create_logical_pm AFTER INSERT ON projects
BEGIN
  INSERT INTO logical_agent_roles
    (role_key, role_kind, project_id, memory_scope_key, receiver_scope_key, tool_profile_key, created_at)
  VALUES ('project-manager:' || NEW.id, 'project_manager', NEW.id, 'project:' || NEW.id,
          'scope:project-manager:' || NEW.id, 'project-manager-v1', NEW.registered_at);
END;

ALTER TABLE project_manager_assignments
  ADD COLUMN logical_role_key TEXT REFERENCES logical_agent_roles(role_key) ON DELETE RESTRICT;

UPDATE project_manager_assignments
SET logical_role_key = 'project-manager:' || project_id;

CREATE INDEX project_manager_assignments_role_idx
  ON project_manager_assignments(logical_role_key, status, manager_delegation_id);

CREATE TRIGGER project_manager_assignment_role_valid BEFORE INSERT ON project_manager_assignments
WHEN NEW.logical_role_key IS NULL OR NOT EXISTS (
  SELECT 1 FROM logical_agent_roles role
  WHERE role.role_key = NEW.logical_role_key
    AND role.role_kind = 'project_manager'
    AND role.project_id = NEW.project_id
)
BEGIN SELECT RAISE(ABORT, 'Project Manager assignment must occupy its project logical role'); END;

CREATE TRIGGER project_manager_assignment_role_immutable
BEFORE UPDATE OF logical_role_key ON project_manager_assignments
WHEN NEW.logical_role_key IS NOT OLD.logical_role_key
BEGIN SELECT RAISE(ABORT, 'Project Manager logical role binding is immutable'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (12, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_13: &str = r#"
CREATE TABLE role_work_plans (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
  plan_level TEXT NOT NULL CHECK(plan_level IN ('main', 'project_manager')),
  manager_delegation_id INTEGER REFERENCES delegations(id) ON DELETE RESTRICT,
  revision INTEGER NOT NULL CHECK(revision > 0),
  plan_json TEXT NOT NULL,
  plan_hash TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,
  CHECK(
    (plan_level = 'main' AND manager_delegation_id IS NULL)
    OR (plan_level = 'project_manager' AND manager_delegation_id IS NOT NULL)
  )
);

CREATE UNIQUE INDEX role_work_plans_main_revision_idx
  ON role_work_plans(run_id, revision) WHERE plan_level = 'main';
CREATE UNIQUE INDEX role_work_plans_pm_revision_idx
  ON role_work_plans(manager_delegation_id, revision) WHERE plan_level = 'project_manager';
CREATE INDEX role_work_plans_project_run_idx
  ON role_work_plans(project_id, run_id, plan_level, revision DESC);

CREATE TABLE delegation_work_plan_refs (
  delegation_id INTEGER PRIMARY KEY REFERENCES delegations(id) ON DELETE RESTRICT,
  plan_id INTEGER NOT NULL REFERENCES role_work_plans(id) ON DELETE RESTRICT,
  task_key TEXT,
  created_at INTEGER NOT NULL,
  CHECK(task_key IS NULL OR length(task_key) BETWEEN 1 AND 80)
);
CREATE UNIQUE INDEX delegation_plan_task_once_idx
  ON delegation_work_plan_refs(plan_id, task_key) WHERE task_key IS NOT NULL;

CREATE TABLE role_work_plan_reviews (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  plan_id INTEGER NOT NULL REFERENCES role_work_plans(id) ON DELETE RESTRICT,
  reviewer_role TEXT NOT NULL CHECK(reviewer_role = 'main_overseer'),
  decision TEXT NOT NULL CHECK(decision IN ('accepted', 'rejected', 'needs_follow_up')),
  note TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL
);
CREATE INDEX role_work_plan_reviews_latest_idx
  ON role_work_plan_reviews(plan_id, created_at DESC, id DESC);

ALTER TABLE worker_report_reviews
  ADD COLUMN reviewer_role TEXT NOT NULL DEFAULT 'unknown'
  CHECK(reviewer_role IN ('unknown', 'project_manager', 'main_overseer'));

CREATE TABLE owner_approval_requests (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
  manager_delegation_id INTEGER NOT NULL REFERENCES delegations(id) ON DELETE RESTRICT,
  source_action_event_id INTEGER NOT NULL REFERENCES events(id) ON DELETE RESTRICT,
  action_key TEXT NOT NULL,
  request_summary TEXT NOT NULL,
  owner_action TEXT NOT NULL,
  reason TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL
);
CREATE INDEX owner_approval_requests_project_idx
  ON owner_approval_requests(project_id, run_id, created_at DESC);

CREATE TABLE owner_approval_decisions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  request_id INTEGER NOT NULL REFERENCES owner_approval_requests(id) ON DELETE RESTRICT,
  decision TEXT NOT NULL CHECK(decision IN ('approved', 'declined', 'clarification_received')),
  response_summary TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL
);
CREATE INDEX owner_approval_decisions_latest_idx
  ON owner_approval_decisions(request_id, id DESC);
CREATE UNIQUE INDEX owner_approval_decision_once_idx
  ON owner_approval_decisions(request_id);

CREATE TRIGGER role_work_plans_no_update BEFORE UPDATE ON role_work_plans
BEGIN SELECT RAISE(ABORT, 'role work plans are immutable'); END;
CREATE TRIGGER role_work_plans_no_delete BEFORE DELETE ON role_work_plans
BEGIN SELECT RAISE(ABORT, 'role work plans are immutable'); END;
CREATE TRIGGER role_work_plan_reviews_no_update BEFORE UPDATE ON role_work_plan_reviews
BEGIN SELECT RAISE(ABORT, 'role work plan reviews are append-only'); END;
CREATE TRIGGER role_work_plan_reviews_no_delete BEFORE DELETE ON role_work_plan_reviews
BEGIN SELECT RAISE(ABORT, 'role work plan reviews are append-only'); END;
CREATE TRIGGER delegation_work_plan_refs_no_update BEFORE UPDATE ON delegation_work_plan_refs
BEGIN SELECT RAISE(ABORT, 'delegation work plan references are immutable'); END;
CREATE TRIGGER delegation_work_plan_refs_no_delete BEFORE DELETE ON delegation_work_plan_refs
BEGIN SELECT RAISE(ABORT, 'delegation work plan references are immutable'); END;
CREATE TRIGGER owner_approval_requests_no_update BEFORE UPDATE ON owner_approval_requests
BEGIN SELECT RAISE(ABORT, 'owner approval requests are immutable'); END;
CREATE TRIGGER owner_approval_requests_no_delete BEFORE DELETE ON owner_approval_requests
BEGIN SELECT RAISE(ABORT, 'owner approval requests are immutable'); END;
CREATE TRIGGER owner_approval_decisions_no_update BEFORE UPDATE ON owner_approval_decisions
BEGIN SELECT RAISE(ABORT, 'owner approval decisions are append-only'); END;
CREATE TRIGGER owner_approval_decisions_no_delete BEFORE DELETE ON owner_approval_decisions
BEGIN SELECT RAISE(ABORT, 'owner approval decisions are append-only'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (13, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_14: &str = r#"
CREATE TABLE role_dispatcher_lease (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  owner_id TEXT,
  lease_expires_at INTEGER NOT NULL DEFAULT 0,
  heartbeat_at INTEGER,
  status TEXT NOT NULL DEFAULT 'idle'
    CHECK(status IN ('idle', 'running', 'failed')),
  last_error TEXT,
  updated_at INTEGER NOT NULL
);

CREATE TABLE role_thread_bindings (
  binding_key TEXT PRIMARY KEY,
  role_key TEXT REFERENCES logical_agent_roles(role_key) ON DELETE RESTRICT,
  delegation_id INTEGER REFERENCES delegations(id) ON DELETE RESTRICT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  thread_id TEXT NOT NULL UNIQUE CHECK(length(thread_id) BETWEEN 1 AND 256),
  status TEXT NOT NULL CHECK(status IN ('active', 'inactive', 'unknown')),
  last_turn_id TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  CHECK(length(binding_key) BETWEEN 1 AND 256),
  CHECK(role_key IS NOT NULL OR delegation_id IS NOT NULL)
);
CREATE UNIQUE INDEX role_thread_active_role_idx
  ON role_thread_bindings(role_key) WHERE role_key IS NOT NULL AND status = 'active';
CREATE UNIQUE INDEX role_thread_active_delegation_idx
  ON role_thread_bindings(delegation_id) WHERE delegation_id IS NOT NULL AND status = 'active';

CREATE TABLE role_dispatch_outbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  delegation_id INTEGER NOT NULL UNIQUE REFERENCES delegations(id) ON DELETE RESTRICT,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  status TEXT NOT NULL CHECK(status IN (
    'queued', 'preflighting', 'thread_ready', 'turn_starting', 'dispatched',
    'running', 'completed', 'blocked', 'failed', 'needs_reconcile', 'cancelled'
  )),
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
  last_error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  acknowledged_at INTEGER
);
CREATE INDEX role_dispatch_queue_idx
  ON role_dispatch_outbox(status, created_at, id);
CREATE INDEX role_dispatch_recent_idx
  ON role_dispatch_outbox(updated_at DESC, id DESC);

CREATE TABLE role_dispatch_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  outbox_id INTEGER NOT NULL REFERENCES role_dispatch_outbox(id) ON DELETE RESTRICT,
  event_type TEXT NOT NULL,
  details_json TEXT NOT NULL DEFAULT '{}',
  created_at INTEGER NOT NULL
);
CREATE INDEX role_dispatch_events_outbox_idx
  ON role_dispatch_events(outbox_id, id);

CREATE TRIGGER role_dispatch_events_no_update BEFORE UPDATE ON role_dispatch_events
BEGIN SELECT RAISE(ABORT, 'role dispatch events are append-only'); END;
CREATE TRIGGER role_dispatch_events_no_delete BEFORE DELETE ON role_dispatch_events
BEGIN SELECT RAISE(ABORT, 'role dispatch events are retained for audit'); END;
CREATE TRIGGER role_thread_binding_identity_guard BEFORE UPDATE ON role_thread_bindings
WHEN NEW.binding_key IS NOT OLD.binding_key OR NEW.role_key IS NOT OLD.role_key
  OR NEW.delegation_id IS NOT OLD.delegation_id OR NEW.project_id IS NOT OLD.project_id
  OR NEW.created_at IS NOT OLD.created_at OR NEW.updated_at < OLD.updated_at
BEGIN SELECT RAISE(ABORT, 'role thread binding identity is immutable'); END;
CREATE TRIGGER role_thread_binding_scope_valid BEFORE INSERT ON role_thread_bindings
WHEN (NEW.role_key IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM logical_agent_roles role
        WHERE role.role_key = NEW.role_key AND role.role_kind = 'project_manager'
          AND role.project_id = NEW.project_id
      ))
  OR (NEW.delegation_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM delegations delegation
        WHERE delegation.id = NEW.delegation_id AND delegation.project_id = NEW.project_id
      ))
  OR (NEW.role_key IS NOT NULL AND NEW.binding_key <> NEW.role_key)
  OR (NEW.role_key IS NULL AND NEW.binding_key <> ('worker:' || NEW.delegation_id))
BEGIN SELECT RAISE(ABORT, 'role thread binding must match its persistent project role or delegation'); END;
CREATE TRIGGER role_thread_binding_no_delete BEFORE DELETE ON role_thread_bindings
BEGIN SELECT RAISE(ABORT, 'role thread bindings are retained for audit'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (14, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_15: &str = r#"
ALTER TABLE owner_approval_decisions
  ADD COLUMN decision_origin TEXT NOT NULL DEFAULT 'legacy_unverified'
  CHECK(decision_origin IN ('legacy_unverified', 'desktop_owner_ui'));

INSERT INTO schema_migrations(version, applied_at)
VALUES (15, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_16: &str = r#"
CREATE TABLE role_thread_capabilities (
  binding_key TEXT PRIMARY KEY CHECK(length(binding_key) BETWEEN 1 AND 256),
  role_kind TEXT NOT NULL CHECK(role_kind IN ('sub_overseer', 'worker')),
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  delegation_id INTEGER NOT NULL UNIQUE REFERENCES delegations(id) ON DELETE RESTRICT,
  capability_hash TEXT NOT NULL UNIQUE CHECK(length(capability_hash) = 64),
  status TEXT NOT NULL CHECK(status IN ('active', 'revoked')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  CHECK((role_kind = 'sub_overseer' AND binding_key = 'project-manager:' || project_id)
     OR (role_kind = 'worker' AND binding_key = 'worker:' || delegation_id))
);

CREATE TRIGGER role_thread_capability_scope_valid BEFORE INSERT ON role_thread_capabilities
WHEN NOT EXISTS (
  SELECT 1 FROM delegations delegation
  WHERE delegation.id = NEW.delegation_id
    AND delegation.project_id = NEW.project_id
    AND delegation.role = NEW.role_kind
    AND delegation.status IN ('prepared', 'dispatched')
    AND ((NEW.role_kind = 'sub_overseer' AND delegation.parent_delegation_id IS NULL)
      OR (NEW.role_kind = 'worker' AND delegation.parent_delegation_id IS NOT NULL))
)
BEGIN SELECT RAISE(ABORT, 'role thread capability must match an active prepared delegation'); END;

CREATE TRIGGER role_thread_capability_identity_immutable
BEFORE UPDATE OF binding_key, role_kind, project_id, created_at ON role_thread_capabilities
WHEN NEW.binding_key IS NOT OLD.binding_key OR NEW.role_kind IS NOT OLD.role_kind
  OR NEW.project_id IS NOT OLD.project_id
  OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'role thread capability identity is immutable'); END;

CREATE TRIGGER role_thread_capability_scope_valid_update
BEFORE UPDATE OF delegation_id ON role_thread_capabilities
WHEN NOT EXISTS (
  SELECT 1 FROM delegations delegation
  WHERE delegation.id = NEW.delegation_id
    AND delegation.project_id = NEW.project_id
    AND delegation.role = NEW.role_kind
    AND delegation.status IN ('prepared', 'dispatched')
    AND ((NEW.role_kind = 'sub_overseer' AND delegation.parent_delegation_id IS NULL)
      OR (NEW.role_kind = 'worker' AND delegation.parent_delegation_id IS NOT NULL))
)
BEGIN SELECT RAISE(ABORT, 'role thread capability must match an active prepared delegation'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (16, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_17: &str = r#"
CREATE TABLE role_turn_inbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_event_id INTEGER NOT NULL UNIQUE REFERENCES events(id) ON DELETE RESTRICT,
  source_delegation_id INTEGER NOT NULL REFERENCES delegations(id) ON DELETE RESTRICT,
  target_binding_key TEXT NOT NULL CHECK(length(target_binding_key) BETWEEN 1 AND 256),
  target_delegation_id INTEGER REFERENCES delegations(id) ON DELETE RESTRICT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN (
    'queued', 'starting', 'dispatched', 'running', 'completed', 'blocked',
    'needs_reconcile', 'unroutable'
  )),
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
  last_error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX role_turn_inbox_queue_idx ON role_turn_inbox(status, created_at, id);
CREATE INDEX role_turn_inbox_project_idx ON role_turn_inbox(project_id, created_at DESC);

CREATE TABLE role_turn_inbox_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  inbox_id INTEGER NOT NULL REFERENCES role_turn_inbox(id) ON DELETE RESTRICT,
  event_type TEXT NOT NULL,
  details_json TEXT NOT NULL DEFAULT '{}',
  created_at INTEGER NOT NULL
);
CREATE INDEX role_turn_inbox_events_inbox_idx ON role_turn_inbox_events(inbox_id, id);
CREATE TRIGGER role_turn_inbox_events_no_update BEFORE UPDATE ON role_turn_inbox_events
BEGIN SELECT RAISE(ABORT, 'role inbox events are append-only'); END;
CREATE TRIGGER role_turn_inbox_events_no_delete BEFORE DELETE ON role_turn_inbox_events
BEGIN SELECT RAISE(ABORT, 'role inbox events are retained for audit'); END;
CREATE TRIGGER role_turn_inbox_identity_immutable
BEFORE UPDATE OF source_event_id, source_delegation_id, target_binding_key, target_delegation_id, project_id,
                 event_type, payload_json, created_at ON role_turn_inbox
WHEN NEW.source_event_id IS NOT OLD.source_event_id
  OR NEW.source_delegation_id IS NOT OLD.source_delegation_id
  OR NEW.target_binding_key IS NOT OLD.target_binding_key
  OR NEW.target_delegation_id IS NOT OLD.target_delegation_id
  OR NEW.project_id IS NOT OLD.project_id OR NEW.event_type IS NOT OLD.event_type
  OR NEW.payload_json IS NOT OLD.payload_json OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'role inbox message identity is immutable'); END;

CREATE TABLE role_event_cursor (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  last_event_id INTEGER NOT NULL CHECK(last_event_id >= 0),
  updated_at INTEGER NOT NULL
);
INSERT INTO role_event_cursor(singleton, last_event_id, updated_at)
SELECT 1, COALESCE(MAX(id), 0), CAST(strftime('%s','now') AS INTEGER) FROM events;

INSERT INTO schema_migrations(version, applied_at)
VALUES (17, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_18: &str = r#"
DROP TRIGGER role_thread_binding_identity_guard;
DROP TRIGGER role_thread_binding_scope_valid;
DROP TRIGGER role_thread_binding_no_delete;
DROP INDEX role_thread_active_role_idx;
DROP INDEX role_thread_active_delegation_idx;
ALTER TABLE role_thread_bindings RENAME TO role_thread_bindings_v14;

CREATE TABLE role_thread_bindings (
  binding_key TEXT PRIMARY KEY,
  role_key TEXT REFERENCES logical_agent_roles(role_key) ON DELETE RESTRICT,
  delegation_id INTEGER REFERENCES delegations(id) ON DELETE RESTRICT,
  project_id INTEGER REFERENCES projects(id) ON DELETE RESTRICT,
  thread_id TEXT NOT NULL UNIQUE CHECK(length(thread_id) BETWEEN 1 AND 256),
  status TEXT NOT NULL CHECK(status IN ('active', 'inactive', 'unknown')),
  last_turn_id TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  CHECK(length(binding_key) BETWEEN 1 AND 256),
  CHECK(role_key IS NOT NULL OR delegation_id IS NOT NULL)
);
INSERT INTO role_thread_bindings
  (binding_key, role_key, delegation_id, project_id, thread_id, status, last_turn_id, created_at, updated_at)
SELECT binding_key, role_key, delegation_id, project_id, thread_id, status, last_turn_id, created_at, updated_at
FROM role_thread_bindings_v14;
DROP TABLE role_thread_bindings_v14;

CREATE UNIQUE INDEX role_thread_active_role_idx
  ON role_thread_bindings(role_key) WHERE role_key IS NOT NULL AND status = 'active';
CREATE UNIQUE INDEX role_thread_active_delegation_idx
  ON role_thread_bindings(delegation_id) WHERE delegation_id IS NOT NULL AND status = 'active';

CREATE TRIGGER role_thread_binding_identity_guard BEFORE UPDATE ON role_thread_bindings
WHEN NEW.binding_key IS NOT OLD.binding_key OR NEW.role_key IS NOT OLD.role_key
  OR NEW.delegation_id IS NOT OLD.delegation_id OR NEW.project_id IS NOT OLD.project_id
  OR NEW.created_at IS NOT OLD.created_at OR NEW.updated_at < OLD.updated_at
BEGIN SELECT RAISE(ABORT, 'role thread binding identity is immutable'); END;
CREATE TRIGGER role_thread_binding_scope_valid BEFORE INSERT ON role_thread_bindings
WHEN (NEW.binding_key = 'main-overseer' AND NOT EXISTS (
        SELECT 1 FROM logical_agent_roles role
        WHERE role.role_key = NEW.role_key AND role.role_kind = 'main_overseer'
          AND role.project_id IS NULL AND NEW.project_id IS NULL AND NEW.delegation_id IS NULL
      ))
  OR (NEW.role_key IS NOT NULL AND NEW.binding_key <> 'main-overseer' AND NOT EXISTS (
        SELECT 1 FROM logical_agent_roles role
        WHERE role.role_key = NEW.role_key AND role.role_kind = 'project_manager'
          AND role.project_id = NEW.project_id AND NEW.delegation_id IS NULL
      ))
  OR (NEW.delegation_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM delegations delegation
        WHERE delegation.id = NEW.delegation_id AND delegation.project_id = NEW.project_id
      ))
  OR (NEW.role_key IS NULL AND NEW.binding_key <> ('worker:' || NEW.delegation_id))
  OR (NEW.role_key IS NOT NULL AND NEW.binding_key <> NEW.role_key)
BEGIN SELECT RAISE(ABORT, 'role thread binding must match its persistent logical role or delegation'); END;
CREATE TRIGGER role_thread_binding_no_delete BEFORE DELETE ON role_thread_bindings
BEGIN SELECT RAISE(ABORT, 'role thread bindings are retained for audit'); END;

CREATE TABLE main_overseer_capability (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  role_key TEXT NOT NULL UNIQUE REFERENCES logical_agent_roles(role_key) ON DELETE RESTRICT,
  capability_hash TEXT NOT NULL UNIQUE CHECK(length(capability_hash) = 64),
  status TEXT NOT NULL CHECK(status IN ('active', 'revoked')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  CHECK(role_key = 'main-overseer')
);
CREATE TRIGGER main_overseer_capability_role_valid BEFORE INSERT ON main_overseer_capability
WHEN NOT EXISTS (
  SELECT 1 FROM logical_agent_roles role
  WHERE role.role_key = NEW.role_key AND role.role_kind = 'main_overseer' AND role.project_id IS NULL
)
BEGIN SELECT RAISE(ABORT, 'Main capability must bind the single global Main role'); END;

CREATE TABLE owner_approval_relays (
  request_id INTEGER PRIMARY KEY REFERENCES owner_approval_requests(id) ON DELETE RESTRICT,
  manager_delegation_id INTEGER NOT NULL REFERENCES delegations(id) ON DELETE RESTRICT,
  event_id INTEGER NOT NULL UNIQUE REFERENCES events(id) ON DELETE RESTRICT,
  created_at INTEGER NOT NULL
);
CREATE TRIGGER owner_approval_relays_no_update BEFORE UPDATE ON owner_approval_relays
BEGIN SELECT RAISE(ABORT, 'Owner decision relays are immutable'); END;
CREATE TRIGGER owner_approval_relays_no_delete BEFORE DELETE ON owner_approval_relays
BEGIN SELECT RAISE(ABORT, 'Owner decision relays are retained for audit'); END;

INSERT INTO schema_migrations(version, applied_at)
VALUES (18, CAST(strftime('%s','now') AS INTEGER));
"#;

pub(crate) const VERSION_19: &str = r#"
ALTER TABLE projects ADD COLUMN status TEXT NOT NULL DEFAULT 'active'
  CHECK(status IN ('active', 'paused', 'completed', 'archived'));
ALTER TABLE projects ADD COLUMN priority INTEGER NOT NULL DEFAULT 0
  CHECK(priority BETWEEN -100 AND 100);
ALTER TABLE projects ADD COLUMN revision INTEGER NOT NULL DEFAULT 1
  CHECK(revision > 0);

ALTER TABLE tasks ADD COLUMN parent_task_id INTEGER REFERENCES tasks(id) ON DELETE RESTRICT;
ALTER TABLE tasks ADD COLUMN priority INTEGER NOT NULL DEFAULT 0
  CHECK(priority BETWEEN -100 AND 100);
ALTER TABLE tasks ADD COLUMN due_date TEXT;
ALTER TABLE tasks ADD COLUMN effort_minutes INTEGER CHECK(effort_minutes IS NULL OR effort_minutes >= 0);
ALTER TABLE tasks ADD COLUMN next_action TEXT;
ALTER TABLE tasks ADD COLUMN revision INTEGER NOT NULL DEFAULT 1
  CHECK(revision > 0);
ALTER TABLE tasks ADD COLUMN completed_at INTEGER;
ALTER TABLE tasks ADD COLUMN completion_kind TEXT
  CHECK(completion_kind IS NULL OR completion_kind IN ('administrative', 'verified_run', 'legacy_unknown'));
ALTER TABLE tasks ADD COLUMN completion_run_id INTEGER REFERENCES runs(id) ON DELETE RESTRICT;
ALTER TABLE tasks ADD COLUMN cancelled_at INTEGER;
ALTER TABLE tasks ADD COLUMN cancellation_reason TEXT
  CHECK(cancellation_reason IS NULL OR length(trim(cancellation_reason)) BETWEEN 1 AND 1000);

-- Old direct status writes did not preserve the source of completion/blocking.
-- Carry that uncertainty forward explicitly instead of inventing provenance.
UPDATE tasks
SET completion_kind = 'legacy_unknown', completed_at = updated_at
WHERE status = 'completed';
UPDATE tasks
SET cancellation_reason = 'legacy_unknown', cancelled_at = updated_at
WHERE status = 'cancelled';

CREATE TABLE task_dependencies (
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  prerequisite_task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(task_id, prerequisite_task_id),
  CHECK(task_id <> prerequisite_task_id)
);
CREATE INDEX task_dependencies_prerequisite_idx
  ON task_dependencies(prerequisite_task_id, task_id);

CREATE TRIGGER task_dependency_scope_insert BEFORE INSERT ON task_dependencies
WHEN NOT EXISTS (
  SELECT 1 FROM tasks dependent
  JOIN objectives dependent_objective ON dependent_objective.id = dependent.objective_id
  JOIN tasks prerequisite ON prerequisite.id = NEW.prerequisite_task_id
  JOIN objectives prerequisite_objective ON prerequisite_objective.id = prerequisite.objective_id
  WHERE dependent.id = NEW.task_id
    AND dependent_objective.project_id = prerequisite_objective.project_id
)
BEGIN SELECT RAISE(ABORT, 'task prerequisites must belong to the same project'); END;

CREATE TRIGGER task_dependency_acyclic_insert BEFORE INSERT ON task_dependencies
WHEN EXISTS (
  WITH RECURSIVE prerequisites(task_id) AS (
    SELECT NEW.prerequisite_task_id
    UNION
    SELECT dependency.prerequisite_task_id
    FROM task_dependencies dependency
    JOIN prerequisites chain ON dependency.task_id = chain.task_id
  )
  SELECT 1 FROM prerequisites WHERE task_id = NEW.task_id
)
BEGIN SELECT RAISE(ABORT, 'task prerequisite cycle is not allowed'); END;

CREATE TRIGGER task_parent_scope_insert BEFORE INSERT ON tasks
WHEN NEW.parent_task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks parent
  JOIN objectives parent_objective ON parent_objective.id = parent.objective_id
  JOIN objectives child_objective ON child_objective.id = NEW.objective_id
  WHERE parent.id = NEW.parent_task_id
    AND parent_objective.project_id = child_objective.project_id
)
BEGIN SELECT RAISE(ABORT, 'parent task must belong to the same project'); END;

CREATE TRIGGER task_parent_scope_update BEFORE UPDATE OF parent_task_id, objective_id ON tasks
WHEN NEW.parent_task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks parent
  JOIN objectives parent_objective ON parent_objective.id = parent.objective_id
  JOIN objectives child_objective ON child_objective.id = NEW.objective_id
  WHERE parent.id = NEW.parent_task_id
    AND parent_objective.project_id = child_objective.project_id
)
BEGIN SELECT RAISE(ABORT, 'parent task must belong to the same project'); END;

CREATE TRIGGER task_parent_acyclic_insert BEFORE INSERT ON tasks
WHEN NEW.parent_task_id IS NOT NULL AND EXISTS (
  WITH RECURSIVE parents(task_id) AS (
    SELECT NEW.parent_task_id
    UNION
    SELECT task.parent_task_id FROM tasks task
    JOIN parents chain ON task.id = chain.task_id
    WHERE task.parent_task_id IS NOT NULL
  )
  SELECT 1 FROM parents WHERE task_id = NEW.id
)
BEGIN SELECT RAISE(ABORT, 'task parent cycle is not allowed'); END;

CREATE TRIGGER task_parent_acyclic_update BEFORE UPDATE OF parent_task_id ON tasks
WHEN NEW.parent_task_id IS NOT NULL AND EXISTS (
  WITH RECURSIVE parents(task_id) AS (
    SELECT NEW.parent_task_id
    UNION
    SELECT task.parent_task_id FROM tasks task
    JOIN parents chain ON task.id = chain.task_id
    WHERE task.parent_task_id IS NOT NULL
  )
  SELECT 1 FROM parents WHERE task_id = NEW.id
)
BEGIN SELECT RAISE(ABORT, 'task parent cycle is not allowed'); END;

CREATE TABLE task_blockers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
  reason_code TEXT NOT NULL CHECK(reason_code IN ('waiting', 'decision', 'external', 'resource', 'other', 'legacy_unknown')),
  reason TEXT NOT NULL CHECK(length(trim(reason)) BETWEEN 1 AND 1000),
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'resolved')),
  resolution TEXT,
  created_at INTEGER NOT NULL,
  resolved_at INTEGER,
  CHECK((status = 'open' AND resolution IS NULL AND resolved_at IS NULL)
     OR (status = 'resolved' AND resolution IS NOT NULL AND resolved_at IS NOT NULL))
);
CREATE INDEX task_blockers_open_idx ON task_blockers(task_id, created_at, id) WHERE status = 'open';
CREATE TRIGGER task_blocker_no_delete BEFORE DELETE ON task_blockers
BEGIN SELECT RAISE(ABORT, 'task blockers are retained for audit'); END;

CREATE TRIGGER task_completion_metadata_insert BEFORE INSERT ON tasks
WHEN NEW.status = 'completed' AND (
  NEW.completion_kind IS NULL OR NEW.completed_at IS NULL
  OR (NEW.completion_kind = 'verified_run' AND NEW.completion_run_id IS NULL)
)
BEGIN SELECT RAISE(ABORT, 'completed tasks require explicit completion provenance'); END;
CREATE TRIGGER task_completion_metadata_update BEFORE UPDATE OF status, completion_kind, completed_at, completion_run_id ON tasks
WHEN NEW.status = 'completed' AND (
  NEW.completion_kind IS NULL OR NEW.completed_at IS NULL
  OR (NEW.completion_kind = 'verified_run' AND NEW.completion_run_id IS NULL)
)
BEGIN SELECT RAISE(ABORT, 'completed tasks require explicit completion provenance'); END;
CREATE TRIGGER task_completion_run_scope_update BEFORE UPDATE OF status, completion_kind, completion_run_id ON tasks
WHEN NEW.status = 'completed' AND NEW.completion_kind = 'verified_run' AND NOT EXISTS (
  SELECT 1 FROM runs run
  JOIN objectives task_objective ON task_objective.id = (SELECT objective_id FROM tasks WHERE id = NEW.id)
  WHERE run.id = NEW.completion_run_id AND run.task_id = NEW.id
    AND run.project_id = task_objective.project_id AND run.status = 'verified_success'
)
BEGIN SELECT RAISE(ABORT, 'verified task completion must reference its successful linked run'); END;

CREATE TRIGGER task_cancellation_metadata_insert BEFORE INSERT ON tasks
WHEN NEW.status = 'cancelled' AND (NEW.cancellation_reason IS NULL OR NEW.cancelled_at IS NULL)
BEGIN SELECT RAISE(ABORT, 'cancelled tasks require an explicit reason and timestamp'); END;
CREATE TRIGGER task_cancellation_metadata_update BEFORE UPDATE OF status, cancellation_reason, cancelled_at ON tasks
WHEN NEW.status = 'cancelled' AND (NEW.cancellation_reason IS NULL OR NEW.cancelled_at IS NULL)
BEGIN SELECT RAISE(ABORT, 'cancelled tasks require an explicit reason and timestamp'); END;

INSERT INTO task_blockers(task_id, reason_code, reason, status, created_at)
SELECT id, 'legacy_unknown', 'Legacy task was marked blocked before structured blocker reasons were available.',
       'open', updated_at
FROM tasks WHERE status = 'blocked';

INSERT INTO schema_migrations(version, applied_at)
VALUES (19, CAST(strftime('%s','now') AS INTEGER));
"#;
