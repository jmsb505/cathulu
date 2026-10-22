use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::process::Command;

const MAX_CATALOG_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexModelOption {
    pub model_slug: String,
    pub display_name: String,
    pub reasoning_efforts: Vec<String>,
    pub default_reasoning_effort: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexModelCatalog {
    pub source: String,
    pub cli_version: String,
    pub account_availability_verified: bool,
    pub models: Vec<CodexModelOption>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodexRoleKind {
    MainOverseer,
    SubagentDefault,
    NamedAgent,
}

impl CodexRoleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MainOverseer => "main_overseer",
            Self::SubagentDefault => "subagent_default",
            Self::NamedAgent => "named_agent",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexAgentPreferenceInput {
    pub role_kind: CodexRoleKind,
    pub agent_name: Option<String>,
    pub model_slug: String,
    pub reasoning_effort: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexAgentPreference {
    pub role_id: String,
    pub role_kind: CodexRoleKind,
    pub agent_name: Option<String>,
    pub model_slug: String,
    pub reasoning_effort: String,
    pub catalog_version: String,
    pub last_applied_target: Option<String>,
    pub last_applied_at: Option<i64>,
    pub last_applied_config_hash: Option<String>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexSelectorSnapshot {
    pub catalog: CodexModelCatalog,
    pub preferences: Vec<CodexAgentPreference>,
    pub preference_state: String,
    pub runtime_effectiveness_verified: bool,
}

#[derive(Deserialize)]
struct RawCatalog {
    models: Vec<RawModel>,
}

#[derive(Deserialize)]
struct RawModel {
    slug: String,
    display_name: String,
    visibility: String,
    supported_in_api: bool,
    supported_reasoning_levels: Vec<RawReasoningLevel>,
    default_reasoning_level: Option<String>,
}

#[derive(Deserialize)]
struct RawReasoningLevel {
    effort: String,
}

pub fn load_bundled_codex_catalog() -> Result<CodexModelCatalog, String> {
    let mut version_command = Command::new("codex");
    crate::suppress_windows_console(&mut version_command);
    let version_output = version_command
        .arg("--version")
        .output()
        .map_err(|_| "cannot start the local Codex CLI to read its version".to_string())?;
    if !version_output.status.success() {
        return Err("the local Codex CLI could not report its version".to_string());
    }
    let version_text = String::from_utf8_lossy(&version_output.stdout);
    let cli_version = version_text
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if cli_version.is_empty() || cli_version.len() > 128 {
        return Err("the local Codex CLI returned an invalid version string".to_string());
    }

    let mut catalog_command = Command::new("codex");
    crate::suppress_windows_console(&mut catalog_command);
    let output = catalog_command
        .args(["debug", "models", "--bundled"])
        .output()
        .map_err(|_| "cannot start the local Codex CLI model catalog command".to_string())?;
    if !output.status.success() {
        return Err("the local Codex CLI could not provide its bundled model catalog".to_string());
    }
    parse_bundled_catalog(&output.stdout, cli_version)
}

fn parse_bundled_catalog(json: &[u8], cli_version: String) -> Result<CodexModelCatalog, String> {
    if json.len() > MAX_CATALOG_BYTES {
        return Err("the local Codex CLI model catalog exceeds the size limit".to_string());
    }
    let raw: RawCatalog = serde_json::from_slice(json)
        .map_err(|_| "the local Codex CLI returned an unsupported model catalog".to_string())?;

    let mut model_slugs = HashSet::new();
    let mut models = Vec::new();
    for model in raw
        .models
        .into_iter()
        .filter(|model| model.visibility == "list" && model.supported_in_api)
    {
        if !is_safe_identifier(&model.slug) || model.display_name.trim().is_empty() {
            return Err("the local Codex CLI catalog contains an invalid model entry".to_string());
        }
        if !model_slugs.insert(model.slug.clone()) {
            return Err("the local Codex CLI catalog contains a duplicate model entry".to_string());
        }

        let mut effort_set = HashSet::new();
        let mut reasoning_efforts = Vec::new();
        for level in model.supported_reasoning_levels {
            if !is_safe_identifier(&level.effort) {
                return Err(
                    "the local Codex CLI catalog contains an invalid reasoning effort".to_string(),
                );
            }
            if !effort_set.insert(level.effort.clone()) {
                return Err(
                    "the local Codex CLI catalog contains a duplicate reasoning effort".to_string(),
                );
            }
            reasoning_efforts.push(level.effort);
        }
        if reasoning_efforts.is_empty() {
            continue;
        }
        if model
            .default_reasoning_level
            .as_ref()
            .is_some_and(|default| !effort_set.contains(default))
        {
            return Err(
                "the local Codex CLI catalog has a default effort it does not support".to_string(),
            );
        }
        models.push(CodexModelOption {
            model_slug: model.slug,
            display_name: model.display_name,
            reasoning_efforts,
            default_reasoning_effort: model.default_reasoning_level,
        });
    }
    if models.is_empty() {
        return Err("the local Codex CLI catalog contains no selectable model options".to_string());
    }

    Ok(CodexModelCatalog {
        source: "bundled_local_codex_cli".to_string(),
        cli_version,
        account_availability_verified: false,
        models,
    })
}

impl CodexAgentPreferenceInput {
    pub fn validate(
        &self,
        catalog: &CodexModelCatalog,
    ) -> Result<(String, Option<String>), String> {
        if !is_safe_identifier(&self.model_slug) {
            return Err(
                "model slug must use letters, numbers, dots, hyphens, or underscores".to_string(),
            );
        }
        if !is_safe_identifier(&self.reasoning_effort) {
            return Err(
                "reasoning effort must use letters, numbers, dots, hyphens, or underscores"
                    .to_string(),
            );
        }

        let agent_name = match (self.role_kind, self.agent_name.as_deref()) {
            (CodexRoleKind::MainOverseer | CodexRoleKind::SubagentDefault, None) => None,
            (CodexRoleKind::NamedAgent, Some(name))
                if !name.is_empty()
                    && name.len() <= 80
                    && name.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
                    }) =>
            {
                Some(name.to_string())
            }
            (CodexRoleKind::MainOverseer | CodexRoleKind::SubagentDefault, Some(_)) => {
                return Err("built-in Codex roles cannot have a custom agent name".to_string());
            }
            (CodexRoleKind::NamedAgent, _) => {
                return Err(
                    "named Codex agents require a 1–80 character alphanumeric name".to_string(),
                );
            }
        };

        let model = catalog
            .models
            .iter()
            .find(|model| model.model_slug == self.model_slug)
            .ok_or_else(|| "model is not present in the current local Codex catalog".to_string())?;
        if !model
            .reasoning_efforts
            .iter()
            .any(|effort| effort == &self.reasoning_effort)
        {
            return Err("reasoning effort is not supported by the selected model".to_string());
        }

        let role_id = match self.role_kind {
            CodexRoleKind::MainOverseer => "main_overseer".to_string(),
            CodexRoleKind::SubagentDefault => "subagent_default".to_string(),
            CodexRoleKind::NamedAgent => {
                format!("agent:{}", agent_name.as_deref().unwrap_or_default())
            }
        };
        Ok((role_id, agent_name))
    }
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_bundled_catalog, CodexAgentPreferenceInput, CodexModelCatalog, CodexRoleKind,
    };
    use crate::Store;
    use std::time::{SystemTime, UNIX_EPOCH};

    const CATALOG_FIXTURE: &str = r#"{
      "models": [
        {
          "slug": "model-a",
          "display_name": "Model A",
          "visibility": "list",
          "supported_in_api": true,
          "supported_reasoning_levels": [
            { "effort": "low", "description": "internal label ignored" },
            { "effort": "high", "description": "another internal label ignored" }
          ],
          "default_reasoning_level": "low",
          "base_instructions": "SENSITIVE INSTRUCTIONS MUST NOT ESCAPE"
        },
        {
          "slug": "hidden-model",
          "display_name": "Hidden",
          "visibility": "hidden",
          "supported_in_api": true,
          "supported_reasoning_levels": [{ "effort": "low" }]
        },
        {
          "slug": "unsupported-model",
          "display_name": "Unsupported",
          "visibility": "list",
          "supported_in_api": false,
          "supported_reasoning_levels": [{ "effort": "low" }]
        }
      ]
    }"#;

    fn fixture_catalog() -> CodexModelCatalog {
        parse_bundled_catalog(CATALOG_FIXTURE.as_bytes(), "codex-cli test".into()).unwrap()
    }

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("cathulu-codex-settings-{label}-{suffix}"))
    }

    #[test]
    fn bundled_catalog_exposes_only_allowlisted_model_and_effort_fields() {
        let catalog = fixture_catalog();
        assert_eq!(catalog.source, "bundled_local_codex_cli");
        assert!(!catalog.account_availability_verified);
        assert_eq!(catalog.models.len(), 1);
        assert_eq!(catalog.models[0].model_slug, "model-a");
        assert_eq!(catalog.models[0].reasoning_efforts, ["low", "high"]);
        let serialized = serde_json::to_string(&catalog).unwrap();
        assert!(!serialized.contains("SENSITIVE INSTRUCTIONS"));
        assert!(!serialized.contains("internal label"));
    }

    #[test]
    fn preference_validation_rejects_unsupported_model_effort_pairs_and_names() {
        let catalog = fixture_catalog();
        let unsupported_effort = CodexAgentPreferenceInput {
            role_kind: CodexRoleKind::MainOverseer,
            agent_name: None,
            model_slug: "model-a".into(),
            reasoning_effort: "ultra".into(),
        };
        assert!(unsupported_effort.validate(&catalog).is_err());

        let unsafe_name = CodexAgentPreferenceInput {
            role_kind: CodexRoleKind::NamedAgent,
            agent_name: Some("../outside".into()),
            model_slug: "model-a".into(),
            reasoning_effort: "low".into(),
        };
        assert!(unsafe_name.validate(&catalog).is_err());
    }

    #[test]
    fn selector_preferences_are_audited_and_persist_across_store_reopen() {
        let data_dir = temp_dir("reopen");
        let store = Store::open(&data_dir).unwrap();
        let catalog = fixture_catalog();
        let main = CodexAgentPreferenceInput {
            role_kind: CodexRoleKind::MainOverseer,
            agent_name: None,
            model_slug: "model-a".into(),
            reasoning_effort: "low".into(),
        };
        let first = store
            .save_codex_selector_preference(&main, &catalog)
            .unwrap();
        assert_eq!(first.role_id, "main_overseer");
        assert_eq!(first.catalog_version, "codex-cli test");

        let named = CodexAgentPreferenceInput {
            role_kind: CodexRoleKind::NamedAgent,
            agent_name: Some("reviewer".into()),
            model_slug: "model-a".into(),
            reasoning_effort: "high".into(),
        };
        store
            .save_codex_selector_preference(&named, &catalog)
            .unwrap();
        store
            .mark_codex_selector_preferences_applied(
                &["main_overseer".into()],
                "project:1",
                &"a".repeat(64),
            )
            .unwrap();

        let reopened = Store::open(&data_dir).unwrap();
        let saved = reopened.codex_selector_preferences().unwrap();
        let expected_hash = "a".repeat(64);
        assert_eq!(saved.len(), 2);
        assert!(saved.iter().any(|preference| {
            preference.role_id == "main_overseer"
                && preference.last_applied_target.as_deref() == Some("project:1")
                && preference.last_applied_config_hash.as_deref() == Some(expected_hash.as_str())
        }));
        assert!(saved.iter().any(|preference| {
            preference.role_id == "agent:reviewer" && preference.reasoning_effort == "high"
        }));
        reopened
            .save_codex_selector_preference(&main, &catalog)
            .unwrap();
        assert!(reopened
            .codex_selector_preferences()
            .unwrap()
            .iter()
            .find(|preference| preference.role_id == "main_overseer")
            .unwrap()
            .last_applied_target
            .is_none());
        let event_count: i64 = reopened
            .connection()
            .unwrap()
            .raw()
            .query_row(
                "SELECT COUNT(*) FROM events WHERE aggregate_type = 'codex_selector_preference'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_count, 4);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
