use crate::Store;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogicalAgentRole {
    pub role_key: String,
    pub role_kind: String,
    pub project_id: Option<i64>,
    pub project_name: Option<String>,
    pub memory_scope_key: String,
    pub receiver_scope_key: String,
    pub tool_profile_key: String,
    pub created_at: i64,
}

impl Store {
    /// Return durable role records; task assignments and Codex chat identity stay separate.
    pub fn logical_agent_roles(&self) -> Result<Vec<LogicalAgentRole>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .raw()
            .prepare(
                "SELECT role.role_key, role.role_kind, role.project_id, project.name,
                        role.memory_scope_key, role.receiver_scope_key, role.tool_profile_key,
                        role.created_at
                 FROM logical_agent_roles role
                 LEFT JOIN projects project ON project.id = role.project_id
                 ORDER BY CASE role.role_kind WHEN 'main_overseer' THEN 0 ELSE 1 END,
                          project.name COLLATE NOCASE, role.project_id",
            )
            .map_err(|error| format!("cannot query logical agent roles: {error}"))?;
        let roles = statement
            .query_map([], |row| {
                Ok(LogicalAgentRole {
                    role_key: row.get(0)?,
                    role_kind: row.get(1)?,
                    project_id: row.get(2)?,
                    project_name: row.get(3)?,
                    memory_scope_key: row.get(4)?,
                    receiver_scope_key: row.get(5)?,
                    tool_profile_key: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(|error| format!("cannot read logical agent roles: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("cannot decode logical agent role: {error}"))?;
        Ok(roles)
    }
}
