use crate::events::append_event;
use crate::hash::content_hash;
use crate::Store;
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SUPPORTED_GRAPHIFY_VERSION: &str = "0.9.82";
pub const SUPPORTED_GRAPHIFY_SCHEMA: i64 = 1;
const MAX_SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024;
pub const DEFAULT_MAX_HOPS: usize = 3;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GraphifySnapshotSummary {
    pub project_id: i64,
    pub snapshot_id: String,
    pub content_hash: String,
    pub source_commit: String,
    pub schema_version: i64,
    pub graphify_version: String,
    pub status: String,
    pub status_reason: Option<String>,
    pub validated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GraphifyPathNode {
    pub id: String,
    pub label: String,
    pub source_file: String,
    pub source_location: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GraphifyPathEdge {
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
    pub confidence: String,
    pub source_file: String,
    pub source_location: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GraphifyPathResult {
    pub project_id: i64,
    pub status: String,
    pub snapshot_id: Option<String>,
    pub source_commit: Option<String>,
    pub schema_version: Option<i64>,
    pub graphify_version: Option<String>,
    pub nodes: Vec<GraphifyPathNode>,
    pub edges: Vec<GraphifyPathEdge>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphifyHeader {
    schema_version: i64,
    graphify_version: String,
}

#[derive(Debug, Deserialize)]
struct GraphifyDocument {
    graph: GraphifyHeader,
    built_at_commit: String,
    directed: bool,
    nodes: Vec<GraphifyNode>,
    links: Vec<GraphifyLink>,
}

#[derive(Clone, Debug, Deserialize)]
struct GraphifyNode {
    id: String,
    label: String,
    file_type: String,
    source_file: String,
    source_location: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GraphifyLink {
    source: String,
    target: String,
    relation: String,
    confidence: String,
    source_file: String,
    source_location: String,
}

impl Store {
    pub fn trace_code_path(
        &self,
        project_id: i64,
        source_symbol: &str,
        target_symbol: &str,
        source_file: Option<&str>,
        target_file: Option<&str>,
        max_hops: usize,
    ) -> Result<GraphifyPathResult, String> {
        validate_query(
            source_symbol,
            target_symbol,
            source_file,
            target_file,
            max_hops,
        )?;
        let project = self
            .get_project(project_id)?
            .ok_or_else(|| format!("project {project_id} is not registered"))?;
        let live = match crate::inspect_git_repository(Path::new(&project.path)) {
            Ok(snapshot) => snapshot,
            Err(_) => {
                self.mark_graphify_unavailable(project_id, "repository_unavailable")?;
                return Ok(unavailable(project_id, "repository_unavailable"));
            }
        };
        if project.head.as_deref() != live.head.as_deref() {
            self.mark_graphify_unavailable(project_id, "project_reconcile_required")?;
            return Ok(unavailable(project_id, "project_reconcile_required"));
        }
        let Some(source_commit) = live.head.as_deref() else {
            self.mark_graphify_unavailable(project_id, "repository_has_no_commit")?;
            return Ok(unavailable(project_id, "repository_has_no_commit"));
        };

        let root = match PathBuf::from(&live.path).canonicalize() {
            Ok(root) => root,
            Err(_) => {
                self.mark_graphify_unavailable(project_id, "repository_unavailable")?;
                return Ok(unavailable(project_id, "repository_unavailable"));
            }
        };
        let graph_path = root.join("graphify-out").join("graph.json");
        let metadata = match fs::metadata(&graph_path) {
            Ok(metadata) => metadata,
            Err(_) => {
                self.mark_graphify_unavailable(project_id, "snapshot_missing")?;
                return Ok(unavailable(project_id, "snapshot_missing"));
            }
        };
        if metadata.len() > MAX_SNAPSHOT_BYTES {
            self.mark_graphify_unavailable(project_id, "snapshot_too_large")?;
            return Ok(unavailable(project_id, "snapshot_too_large"));
        }
        let resolved_graph_path = match graph_path.canonicalize() {
            Ok(path) if path.starts_with(&root) => path,
            _ => {
                self.mark_graphify_unavailable(project_id, "snapshot_outside_repository")?;
                return Ok(unavailable(project_id, "snapshot_outside_repository"));
            }
        };
        let snapshot_bytes = match fs::read(&resolved_graph_path) {
            Ok(bytes) => bytes,
            Err(_) => {
                self.mark_graphify_unavailable(project_id, "snapshot_unreadable")?;
                return Ok(unavailable(project_id, "snapshot_unreadable"));
            }
        };
        if snapshot_bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
            self.mark_graphify_unavailable(project_id, "snapshot_too_large")?;
            return Ok(unavailable(project_id, "snapshot_too_large"));
        }
        let document: GraphifyDocument = match serde_json::from_slice(&snapshot_bytes) {
            Ok(document) => document,
            Err(_) => {
                self.mark_graphify_unavailable(project_id, "malformed_snapshot")?;
                return Ok(unavailable(project_id, "malformed_snapshot"));
            }
        };
        if document.graph.schema_version != SUPPORTED_GRAPHIFY_SCHEMA
            || document.graph.graphify_version != SUPPORTED_GRAPHIFY_VERSION
        {
            self.mark_graphify_unavailable(project_id, "unsupported_snapshot_version")?;
            return Ok(unavailable(project_id, "unsupported_snapshot_version"));
        }
        if !document.directed {
            self.mark_graphify_unavailable(project_id, "undirected_snapshot")?;
            return Ok(unavailable(project_id, "undirected_snapshot"));
        }
        if !document.built_at_commit.eq_ignore_ascii_case(source_commit) {
            self.mark_graphify_unavailable(project_id, "source_commit_mismatch")?;
            return Ok(unavailable(project_id, "source_commit_mismatch"));
        }
        if let Err(reason) = validate_document_sources(&root, &live.changed_paths, &document) {
            self.mark_graphify_unavailable(project_id, reason)?;
            return Ok(unavailable(project_id, reason));
        }

        let content_hash_value = content_hash(
            std::str::from_utf8(&snapshot_bytes)
                .map_err(|_| "Graphify snapshot is not valid UTF-8".to_string())?,
        );
        let canonical_root = root.to_string_lossy();
        let snapshot_id = content_hash(&format!(
            "graphify-snapshot-v1\0{project_id}\0{canonical_root}\0{source_commit}\0{content_hash_value}"
        ));
        let mut source_files = document
            .nodes
            .iter()
            .map(|node| node.source_file.clone())
            .chain(document.links.iter().map(|edge| edge.source_file.clone()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        source_files.sort();
        let summary = self.record_graphify_snapshot(
            project_id,
            &snapshot_id,
            &content_hash_value,
            source_commit,
            document.graph.schema_version,
            &document.graph.graphify_version,
            &source_files,
        )?;

        let node_map = document
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let source_matches = resolve_nodes(&document.nodes, source_symbol, source_file);
        let target_matches = resolve_nodes(&document.nodes, target_symbol, target_file);
        if source_matches.len() > 1 || target_matches.len() > 1 {
            return Ok(path_result(
                project_id,
                "ambiguous",
                &summary,
                Vec::new(),
                Vec::new(),
                Some("symbol_matches_multiple_nodes"),
            ));
        }
        let (Some(source), Some(target)) = (source_matches.first(), target_matches.first()) else {
            return Ok(path_result(
                project_id,
                "no_path",
                &summary,
                Vec::new(),
                Vec::new(),
                None,
            ));
        };
        let Some(edge_path) = shortest_path(
            &document.nodes,
            &document.links,
            &node_map,
            source.id.as_str(),
            target.id.as_str(),
            max_hops,
        ) else {
            return Ok(path_result(
                project_id,
                "no_path",
                &summary,
                Vec::new(),
                Vec::new(),
                None,
            ));
        };
        let mut nodes = Vec::with_capacity(edge_path.len() + 1);
        if let Some(node) = node_map.get(source.id.as_str()) {
            nodes.push(path_node(node));
        }
        for edge in &edge_path {
            if let Some(node) = node_map.get(edge.target.as_str()) {
                nodes.push(path_node(node));
            }
        }
        let edges = edge_path.iter().map(|edge| path_edge(edge)).collect();
        Ok(path_result(
            project_id, "found", &summary, nodes, edges, None,
        ))
    }

    pub fn get_graphify_snapshot(
        &self,
        project_id: i64,
    ) -> Result<Option<GraphifySnapshotSummary>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT project_id, snapshot_id, content_hash, source_commit, schema_version,
                        graphify_version, status, status_reason, validated_at
                 FROM project_graphify_snapshots WHERE project_id = ?1",
                params![project_id],
                snapshot_summary_from_row,
            )
            .optional()
            .map_err(|error| format!("cannot read Graphify snapshot metadata: {error}"))
    }

    fn record_graphify_snapshot(
        &self,
        project_id: i64,
        snapshot_id: &str,
        content_hash: &str,
        source_commit: &str,
        schema_version: i64,
        graphify_version: &str,
        source_files: &[String],
    ) -> Result<GraphifySnapshotSummary, String> {
        let source_files_json = serde_json::to_string(source_files)
            .map_err(|error| format!("cannot serialize Graphify source files: {error}"))?;
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction()
            .map_err(|error| format!("cannot begin Graphify snapshot update: {error}"))?;
        let previous = transaction
            .query_row(
                "SELECT snapshot_id, status FROM project_graphify_snapshots WHERE project_id = ?1",
                params![project_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| format!("cannot inspect previous Graphify snapshot: {error}"))?;
        transaction
            .execute(
                "INSERT INTO project_graphify_snapshots
                 (project_id, snapshot_id, content_hash, source_commit, schema_version,
                  graphify_version, source_files_json, status, status_reason, validated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'available', NULL, ?8)
                 ON CONFLICT(project_id) DO UPDATE SET
                   snapshot_id = excluded.snapshot_id, content_hash = excluded.content_hash,
                   source_commit = excluded.source_commit, schema_version = excluded.schema_version,
                   graphify_version = excluded.graphify_version,
                   source_files_json = excluded.source_files_json, status = 'available',
                   status_reason = NULL, validated_at = excluded.validated_at",
                params![
                    project_id,
                    snapshot_id,
                    content_hash,
                    source_commit,
                    schema_version,
                    graphify_version,
                    source_files_json,
                    now
                ],
            )
            .map_err(|error| format!("cannot save Graphify snapshot metadata: {error}"))?;
        let is_new_identity = previous.as_ref().is_none_or(|(previous_id, status)| {
            previous_id != snapshot_id || status != "available"
        });
        if is_new_identity {
            let invalidated = Store::invalidate_graph_snapshot_memories(
                &transaction,
                project_id,
                Some(snapshot_id),
                now,
            )?;
            append_event(
                &transaction,
                "project",
                project_id,
                if previous.is_some() {
                    "graphify_snapshot_changed"
                } else {
                    "graphify_snapshot_validated"
                },
                &json!({
                    "previousSnapshotId": previous.as_ref().map(|(id, _)| id),
                    "snapshotId": snapshot_id,
                    "contentHash": content_hash,
                    "sourceCommit": source_commit,
                    "schemaVersion": schema_version,
                    "graphifyVersion": graphify_version,
                    "invalidatedMemoryCount": invalidated.len()
                }),
                now,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| format!("cannot commit Graphify snapshot metadata: {error}"))?;
        self.get_graphify_snapshot(project_id)?
            .ok_or_else(|| "validated Graphify snapshot disappeared from the database".to_string())
    }

    fn mark_graphify_unavailable(&self, project_id: i64, reason: &str) -> Result<(), String> {
        let status = match reason {
            "project_reconcile_required"
            | "source_commit_mismatch"
            | "source_files_changed"
            | "source_file_missing" => "stale",
            _ => "unavailable",
        };
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction()
            .map_err(|error| format!("cannot begin Graphify freshness update: {error}"))?;
        let previous_status = transaction
            .query_row(
                "SELECT status FROM project_graphify_snapshots WHERE project_id = ?1",
                params![project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("cannot inspect Graphify freshness: {error}"))?;
        if let Some(previous_status) = previous_status {
            if previous_status != status {
                transaction
                    .execute(
                        "UPDATE project_graphify_snapshots
                         SET status = ?1, status_reason = ?2, validated_at = ?3
                         WHERE project_id = ?4",
                        params![status, reason, now, project_id],
                    )
                    .map_err(|error| {
                        format!("cannot mark Graphify snapshot unavailable: {error}")
                    })?;
                let invalidated =
                    Store::invalidate_graph_snapshot_memories(&transaction, project_id, None, now)?;
                append_event(
                    &transaction,
                    "project",
                    project_id,
                    "graphify_snapshot_unavailable",
                    &json!({ "status": status, "reason": reason, "invalidatedMemoryCount": invalidated.len() }),
                    now,
                )?;
            } else {
                transaction
                    .execute(
                        "UPDATE project_graphify_snapshots SET status_reason = ?1, validated_at = ?2
                         WHERE project_id = ?3 AND status_reason IS NOT ?1",
                        params![reason, now, project_id],
                    )
                    .map_err(|error| format!("cannot refresh Graphify unavailability reason: {error}"))?;
            }
        }
        transaction
            .commit()
            .map_err(|error| format!("cannot commit Graphify freshness update: {error}"))
    }
}

impl Store {
    pub(crate) fn invalidate_graphify_if_stale(
        transaction: &Transaction<'_>,
        project_id: i64,
        repository_root: &Path,
        current_commit: Option<&str>,
        changed_paths: &[String],
        now: i64,
    ) -> Result<(), String> {
        let snapshot = transaction
            .query_row(
                "SELECT source_commit, source_files_json, status
                 FROM project_graphify_snapshots WHERE project_id = ?1",
                params![project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| {
                format!("cannot inspect Graphify snapshot during reconciliation: {error}")
            })?;
        let Some((source_commit, source_files_json, status)) = snapshot else {
            return Ok(());
        };
        if status != "available" {
            return Ok(());
        }
        let source_files: Vec<String> = serde_json::from_str(&source_files_json)
            .map_err(|error| format!("stored Graphify source file list is invalid: {error}"))?;
        let changed = changed_paths
            .iter()
            .map(|path| normalized_path(path))
            .collect::<HashSet<_>>();
        let reason = if current_commit != Some(source_commit.as_str()) {
            Some(("stale", "source_commit_changed"))
        } else if source_files
            .iter()
            .any(|path| changed.contains(&normalized_path(path)))
        {
            Some(("stale", "source_files_changed"))
        } else if !repository_root
            .join("graphify-out")
            .join("graph.json")
            .is_file()
        {
            Some(("unavailable", "snapshot_missing"))
        } else {
            None
        };
        let Some((next_status, reason)) = reason else {
            return Ok(());
        };
        transaction
            .execute(
                "UPDATE project_graphify_snapshots
                 SET status = ?1, status_reason = ?2, validated_at = ?3
                 WHERE project_id = ?4 AND status = 'available'",
                params![next_status, reason, now, project_id],
            )
            .map_err(|error| format!("cannot mark reconciled Graphify snapshot stale: {error}"))?;
        let invalidated =
            Store::invalidate_graph_snapshot_memories(transaction, project_id, None, now)?;
        append_event(
            transaction,
            "project",
            project_id,
            "graphify_snapshot_unavailable",
            &json!({ "status": next_status, "reason": reason, "invalidatedMemoryCount": invalidated.len() }),
            now,
        )?;
        Ok(())
    }
}

fn validate_query(
    source_symbol: &str,
    target_symbol: &str,
    source_file: Option<&str>,
    target_file: Option<&str>,
    max_hops: usize,
) -> Result<(), String> {
    for (label, value) in [
        ("source symbol", source_symbol),
        ("target symbol", target_symbol),
    ] {
        if value.trim().is_empty() || value.len() > 256 || value.contains('\0') {
            return Err(format!("{label} must be 1 to 256 bytes and contain no NUL"));
        }
    }
    for (label, value) in [("source file", source_file), ("target file", target_file)] {
        if value.is_some_and(|value| value.len() > 1_024 || value.contains('\0')) {
            return Err(format!(
                "{label} must be at most 1024 bytes and contain no NUL"
            ));
        }
    }
    if !(1..=4).contains(&max_hops) {
        return Err("Graphify max_hops must be between 1 and 4".to_string());
    }
    Ok(())
}

fn validate_document_sources(
    root: &Path,
    changed_paths: &[String],
    document: &GraphifyDocument,
) -> Result<(), &'static str> {
    let changed = changed_paths
        .iter()
        .map(|path| normalized_path(path))
        .collect::<HashSet<_>>();
    let mut file_cache = HashMap::<String, usize>::new();
    let mut node_ids = HashSet::new();
    for node in &document.nodes {
        if node.id.trim().is_empty()
            || node.label.trim().is_empty()
            || node.file_type != "code"
            || !node_ids.insert(node.id.as_str())
        {
            return Err("malformed_snapshot");
        }
        validate_source_location(
            root,
            &changed,
            &node.source_file,
            &node.source_location,
            &mut file_cache,
        )?;
    }
    for edge in &document.links {
        if !node_ids.contains(edge.source.as_str())
            || !node_ids.contains(edge.target.as_str())
            || edge.relation.trim().is_empty()
            || !matches!(edge.confidence.as_str(), "EXTRACTED" | "INFERRED")
        {
            return Err("malformed_snapshot");
        }
        validate_source_location(
            root,
            &changed,
            &edge.source_file,
            &edge.source_location,
            &mut file_cache,
        )?;
    }
    Ok(())
}

fn validate_source_location(
    root: &Path,
    changed: &HashSet<String>,
    source_file: &str,
    source_location: &str,
    file_cache: &mut HashMap<String, usize>,
) -> Result<(), &'static str> {
    let relative = Path::new(source_file);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("source_outside_repository");
    }
    let normalized = normalized_path(source_file);
    if changed.contains(&normalized) {
        return Err("source_files_changed");
    }
    let line = parse_source_line(source_location).ok_or("malformed_snapshot")?;
    let lines = if let Some(lines) = file_cache.get(&normalized) {
        *lines
    } else {
        let source_path = root.join(relative);
        let canonical = source_path
            .canonicalize()
            .map_err(|_| "source_file_missing")?;
        if !canonical.starts_with(root) {
            return Err("source_outside_repository");
        }
        let content = fs::read_to_string(&canonical).map_err(|_| "source_file_missing")?;
        let lines = content.lines().count();
        file_cache.insert(normalized, lines);
        lines
    };
    if line == 0 || line > lines {
        return Err("source_location_out_of_range");
    }
    Ok(())
}

fn parse_source_line(location: &str) -> Option<usize> {
    let range = location.strip_prefix('L')?;
    let first = range.split(['-', '–']).next()?;
    first.parse().ok()
}

fn resolve_nodes<'a>(
    nodes: &'a [GraphifyNode],
    symbol: &str,
    source_file: Option<&str>,
) -> Vec<&'a GraphifyNode> {
    let requested = normalize_symbol(symbol);
    let requested_file = source_file.map(normalized_path);
    nodes
        .iter()
        .filter(|node| {
            let label_matches = normalize_symbol(&node.label) == requested;
            let id_matches = node.id.eq_ignore_ascii_case(symbol.trim());
            (label_matches || id_matches)
                && requested_file
                    .as_ref()
                    .is_none_or(|path| normalized_path(&node.source_file) == *path)
        })
        .collect()
}

fn shortest_path<'a>(
    nodes: &[GraphifyNode],
    links: &'a [GraphifyLink],
    node_map: &HashMap<&str, &GraphifyNode>,
    source: &str,
    target: &str,
    max_hops: usize,
) -> Option<Vec<&'a GraphifyLink>> {
    if source == target {
        return Some(Vec::new());
    }
    let valid_nodes = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let mut adjacency = HashMap::<&str, Vec<&GraphifyLink>>::new();
    for edge in links {
        if valid_nodes.contains(edge.source.as_str())
            && valid_nodes.contains(edge.target.as_str())
            && node_map.contains_key(edge.source.as_str())
            && node_map.contains_key(edge.target.as_str())
        {
            adjacency.entry(&edge.source).or_default().push(edge);
        }
    }
    for edges in adjacency.values_mut() {
        edges.sort_by(|a, b| {
            (&a.target, &a.relation, &a.source_location).cmp(&(
                &b.target,
                &b.relation,
                &b.source_location,
            ))
        });
    }
    let mut queue = VecDeque::from([(source, Vec::<&GraphifyLink>::new())]);
    let mut visited = HashSet::from([source]);
    while let Some((node_id, path)) = queue.pop_front() {
        if path.len() >= max_hops {
            continue;
        }
        for edge in adjacency.get(node_id).into_iter().flatten() {
            if !visited.insert(edge.target.as_str()) {
                continue;
            }
            let mut next = path.clone();
            next.push(*edge);
            if edge.target == target {
                return Some(next);
            }
            queue.push_back((edge.target.as_str(), next));
        }
    }
    None
}

fn path_node(node: &GraphifyNode) -> GraphifyPathNode {
    GraphifyPathNode {
        id: node.id.clone(),
        label: node.label.clone(),
        source_file: node.source_file.clone(),
        source_location: node.source_location.clone(),
    }
}

fn path_edge(edge: &GraphifyLink) -> GraphifyPathEdge {
    GraphifyPathEdge {
        source_id: edge.source.clone(),
        target_id: edge.target.clone(),
        relation: edge.relation.clone(),
        confidence: edge.confidence.clone(),
        source_file: edge.source_file.clone(),
        source_location: edge.source_location.clone(),
    }
}

fn path_result(
    project_id: i64,
    status: &str,
    summary: &GraphifySnapshotSummary,
    nodes: Vec<GraphifyPathNode>,
    edges: Vec<GraphifyPathEdge>,
    reason: Option<&str>,
) -> GraphifyPathResult {
    GraphifyPathResult {
        project_id,
        status: status.to_string(),
        snapshot_id: Some(summary.snapshot_id.clone()),
        source_commit: Some(summary.source_commit.clone()),
        schema_version: Some(summary.schema_version),
        graphify_version: Some(summary.graphify_version.clone()),
        nodes,
        edges,
        reason: reason.map(str::to_string),
    }
}

fn unavailable(project_id: i64, reason: &str) -> GraphifyPathResult {
    GraphifyPathResult {
        project_id,
        status: "unavailable".to_string(),
        snapshot_id: None,
        source_commit: None,
        schema_version: None,
        graphify_version: None,
        nodes: Vec::new(),
        edges: Vec::new(),
        reason: Some(reason.to_string()),
    }
}

fn normalize_symbol(symbol: &str) -> String {
    symbol
        .trim()
        .trim_start_matches('.')
        .trim_end_matches("()")
        .to_ascii_lowercase()
}

fn normalized_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

fn snapshot_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphifySnapshotSummary> {
    Ok(GraphifySnapshotSummary {
        project_id: row.get(0)?,
        snapshot_id: row.get(1)?,
        content_hash: row.get(2)?,
        source_commit: row.get(3)?,
        schema_version: row.get(4)?,
        graphify_version: row.get(5)?,
        status: row.get(6)?,
        status_reason: row.get(7)?,
        validated_at: row.get(8)?,
    })
}

fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::parse_source_line;

    #[test]
    fn parses_graphify_line_locations() {
        assert_eq!(parse_source_line("L17"), Some(17));
        assert_eq!(parse_source_line("L17-L19"), Some(17));
        assert_eq!(parse_source_line("unknown"), None);
    }
}
