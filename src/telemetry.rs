use crate::events::{append_event, append_external_event};
use crate::hash::content_hash;
use crate::Store;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;

const OBSERVED_RUN_SUMMARY: &str = "Observed Codex turn; prompt and transcript content omitted.";
const UNVERIFIED_RUN_SUMMARY: &str = "Codex turn ended; Cathulu verification was not recorded.";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct CodexHookEvent {
    pub hook_event_name: String,
    pub session_id: String,
    pub turn_id: String,
    pub cwd: Option<String>,
    pub tool_name: Option<String>,
    pub tool_use_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookIngestResult {
    pub recorded: bool,
    pub duplicate: bool,
    pub run_id: Option<i64>,
    pub reason: Option<String>,
}

impl Store {
    pub fn record_codex_hook_event(
        &self,
        event: CodexHookEvent,
    ) -> Result<CodexHookIngestResult, String> {
        let Some(event_kind) = event_kind(&event.hook_event_name) else {
            return Ok(ignored("unsupported_event"));
        };
        if !valid_identifier(&event.session_id) || !valid_identifier(&event.turn_id) {
            return Ok(ignored("invalid_identifier"));
        }

        let tool_category = if event_kind == "tool_completed" {
            let (Some(tool_name), Some(tool_use_id)) =
                (event.tool_name.as_deref(), event.tool_use_id.as_deref())
            else {
                return Ok(ignored("missing_tool_metadata"));
            };
            if !valid_identifier(tool_use_id) {
                return Ok(ignored("invalid_identifier"));
            }
            if tool_name.is_empty()
                || tool_name.len() > 128
                || tool_name.chars().any(char::is_control)
            {
                return Ok(ignored("invalid_tool_metadata"));
            }
            Some(classify_tool(tool_name))
        } else {
            None
        };

        let session_hash = content_hash(&format!("codex-session-v1\0{}", event.session_id));
        let turn_hash = content_hash(&format!("codex-turn-v1\0{}", event.turn_id));
        let tool_use_hash = event
            .tool_use_id
            .as_deref()
            .map(|id| content_hash(&format!("codex-tool-use-v1\0{id}")));
        let external_event_id = content_hash(&format!(
            "codex-hook-event-v1\0{session_hash}\0{turn_hash}\0{event_kind}\0{}",
            tool_use_hash.as_deref().unwrap_or_default()
        ));

        let known_run = self.find_codex_turn_run(&session_hash, &turn_hash)?;
        let project_id = if known_run.is_none() {
            let Some(cwd) = event.cwd.as_deref() else {
                return Ok(ignored("missing_project_directory"));
            };
            if cwd.len() > 4_096 || cwd.chars().any(char::is_control) {
                return Ok(ignored("invalid_project_directory"));
            }
            match self.registered_project_id_for_path(Path::new(cwd))? {
                Some(project_id) => Some(project_id),
                None => return Ok(ignored("unregistered_project")),
            }
        } else {
            None
        };

        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction()
            .map_err(|error| format!("cannot begin Codex event ingestion: {error}"))?;

        if let Some(run_id) = transaction
            .query_row(
                "SELECT run_id FROM codex_turn_runs WHERE session_hash = ?1 AND turn_hash = ?2",
                params![session_hash, turn_hash],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sqlite_error)?
        {
            if transaction
                .query_row(
                    "SELECT 1 FROM events WHERE external_event_id = ?1",
                    params![external_event_id],
                    |_| Ok(()),
                )
                .optional()
                .map_err(sqlite_error)?
                .is_some()
            {
                transaction.commit().map_err(sqlite_error)?;
                return Ok(CodexHookIngestResult {
                    recorded: false,
                    duplicate: true,
                    run_id: Some(run_id),
                    reason: None,
                });
            }
        } else if transaction
            .query_row(
                "SELECT 1 FROM events WHERE external_event_id = ?1",
                params![external_event_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(sqlite_error)?
            .is_some()
        {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(CodexHookIngestResult {
                recorded: false,
                duplicate: true,
                run_id: None,
                reason: None,
            });
        }

        let run_id = match transaction
            .query_row(
                "SELECT run_id FROM codex_turn_runs WHERE session_hash = ?1 AND turn_hash = ?2",
                params![session_hash, turn_hash],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sqlite_error)?
        {
            Some(run_id) => {
                transaction
                    .execute(
                        "UPDATE codex_turn_runs SET last_seen_at = ?1
                         WHERE session_hash = ?2 AND turn_hash = ?3",
                        params![now, session_hash, turn_hash],
                    )
                    .map_err(sqlite_error)?;
                run_id
            }
            None => {
                let Some(project_id) = project_id else {
                    return Ok(ignored("unregistered_project"));
                };
                transaction
                    .execute(
                        "INSERT INTO runs (project_id, status, summary, started_at)
                         VALUES (?1, 'running', ?2, ?3)",
                        params![project_id, OBSERVED_RUN_SUMMARY, now],
                    )
                    .map_err(sqlite_error)?;
                let run_id = transaction.last_insert_rowid();
                transaction
                    .execute(
                        "INSERT INTO codex_turn_runs (session_hash, turn_hash, run_id, last_seen_at)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![session_hash, turn_hash, run_id, now],
                    )
                    .map_err(sqlite_error)?;
                append_event(
                    &transaction,
                    "run",
                    run_id,
                    "run_started",
                    &json!({
                        "source": "codex_hook",
                        "projectId": project_id,
                        "summary": OBSERVED_RUN_SUMMARY
                    }),
                    now,
                )?;
                run_id
            }
        };

        let payload = json!({
            "source": "codex_hook",
            "eventName": event.hook_event_name,
            "sessionHash": session_hash,
            "turnHash": turn_hash,
            "sourceEventHash": external_event_id,
            "toolCategory": tool_category,
            "toolUseHash": tool_use_hash
        });
        let inserted_event = append_external_event(
            &transaction,
            "run",
            run_id,
            &format!("codex_{event_kind}"),
            &external_event_id,
            &payload,
            now,
        )?;
        if inserted_event.is_none() {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(CodexHookIngestResult {
                recorded: false,
                duplicate: true,
                run_id: Some(run_id),
                reason: None,
            });
        }

        if matches!(event_kind, "turn_stopped" | "turn_interrupted") {
            let updated = transaction
                .execute(
                    "UPDATE runs SET status = 'inconclusive', summary = ?1, finished_at = ?2
                     WHERE id = ?3 AND status = 'running'",
                    params![UNVERIFIED_RUN_SUMMARY, now, run_id],
                )
                .map_err(sqlite_error)?;
            if updated > 0 {
                append_event(
                    &transaction,
                    "run",
                    run_id,
                    "run_finished",
                    &json!({
                        "status": "inconclusive",
                        "reason": "codex_turn_ended_without_cathulu_verification"
                    }),
                    now,
                )?;
            }
        }

        transaction.commit().map_err(sqlite_error)?;
        Ok(CodexHookIngestResult {
            recorded: true,
            duplicate: false,
            run_id: Some(run_id),
            reason: None,
        })
    }

    fn find_codex_turn_run(
        &self,
        session_hash: &str,
        turn_hash: &str,
    ) -> Result<Option<i64>, String> {
        self.connection()?
            .raw()
            .query_row(
                "SELECT run_id FROM codex_turn_runs WHERE session_hash = ?1 AND turn_hash = ?2",
                params![session_hash, turn_hash],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)
    }
}

fn event_kind(event_name: &str) -> Option<&'static str> {
    match event_name {
        "UserPromptSubmit" => Some("prompt_submitted"),
        "PostToolUse" => Some("tool_completed"),
        "Stop" => Some("turn_stopped"),
        "Interrupt" => Some("turn_interrupted"),
        _ => None,
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn classify_tool(tool_name: &str) -> &'static str {
    match tool_name {
        "Bash" => "shell",
        "apply_patch" | "Edit" | "Write" => "edit",
        value if value.starts_with("mcp__") => "mcp",
        _ => "other",
    }
}

fn ignored(reason: &str) -> CodexHookIngestResult {
    CodexHookIngestResult {
        recorded: false,
        duplicate: false,
        run_id: None,
        reason: Some(reason.to_string()),
    }
}

fn unix_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn sqlite_error(error: rusqlite::Error) -> String {
    format!("SQLite operation failed: {error}")
}
