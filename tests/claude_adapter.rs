use cathulu_core::{
    agent_provider::{
        CredentialSource, ExternalId, ExternalSession, FeatureSupport, McpToolConfig,
        PermissionManifest, ProviderError, ProviderEvent, ProviderEventKind, ProviderId,
        SessionConfig, TurnOutcome, WorkspaceAccess,
    },
    claude_code::{
        adapter_capabilities, build_invocation, read_bounded_line, scrub_environment, StreamMapper,
        MAX_STREAM_LINE_BYTES,
    },
};
use std::collections::BTreeMap;
use std::io::{BufReader, Cursor};

const SESSION: &str = "3f2c9a1e-6b7d-4c8e-9f0a-1b2c3d4e5f60";
const OTHER_SESSION: &str = "3f2c9a1e-6b7d-4c8e-9f0a-1b2c3d4e5f61";

fn turn_id() -> ExternalId {
    ExternalId::new("attempt-1").unwrap()
}

fn session_config(access: WorkspaceAccess) -> SessionConfig {
    let mut env = BTreeMap::new();
    env.insert(
        "CATHULU_ROLE_CAPABILITY".to_string(),
        "synthetic-token".to_string(),
    );
    SessionConfig {
        logical_role_key: "worker:1".to_string(),
        project_id: Some(1),
        permissions: PermissionManifest {
            schema_version: 1,
            workspace_root: std::env::current_dir().unwrap(),
            access,
            allowed_scope: Vec::new(),
            network_access: false,
            allowed_credential_sources: vec![CredentialSource::Subscription],
        },
        tools: McpToolConfig {
            server_name: "cathulu".to_string(),
            command: std::env::current_exe().unwrap(),
            args: vec!["mcp".to_string()],
            env,
            required_tools: vec!["read_memories".to_string()],
        },
        model: None,
    }
}

fn session() -> ExternalSession {
    ExternalSession {
        provider_id: ProviderId::claude(),
        session_id: ExternalId::new(SESSION).unwrap(),
    }
}

fn parent_env(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// A clean init line for the selected session: the Cathulu server is connected and nothing is loaded.
fn init_ok() -> &'static str {
    r#"{"type":"system","subtype":"init","session_id":"3f2c9a1e-6b7d-4c8e-9f0a-1b2c3d4e5f60","mcp_servers":[{"name":"cathulu","status":"connected"}],"uuid":"u-init"}"#
}

fn map_all(lines: &[&str]) -> Vec<Result<Vec<ProviderEvent>, ProviderError>> {
    let mut mapper = StreamMapper::new(session(), turn_id(), "cathulu").unwrap();
    lines.iter().map(|line| mapper.map_line(line)).collect()
}

fn kinds(results: Vec<Result<Vec<ProviderEvent>, ProviderError>>) -> Vec<ProviderEventKind> {
    results
        .into_iter()
        .flat_map(|result| result.expect("line should map without error"))
        .map(|event| event.event)
        .collect()
}

/// Runs one whole process: each stream line, then its end with the exit status.
fn run(lines: &[&str], exit: Option<i32>) -> Result<Vec<ProviderEvent>, ProviderError> {
    let mut mapper = StreamMapper::new(session(), turn_id(), "cathulu").unwrap();
    let mut events = Vec::new();
    for line in lines {
        events.extend(mapper.map_line(line)?);
    }
    events.extend(mapper.finish(exit)?);
    Ok(events)
}

#[test]
fn read_only_invocation_has_documented_flags_and_no_bypass() {
    let invocation = build_invocation(
        &session_config(WorkspaceAccess::ReadOnly),
        &ExternalId::new(SESSION).unwrap(),
        "C:\\cathulu\\claude-config",
        "C:\\cathulu\\mcp.json",
        &parent_env(&[("PATH", "C:\\bin")]),
    )
    .unwrap();
    for flag in [
        "-p",
        "stream-json",
        "--verbose",
        "--session-id",
        "dontAsk",
        "--strict-mcp-config",
        "--disable-slash-commands",
    ] {
        assert!(
            invocation.args.iter().any(|arg| arg == flag),
            "missing {flag}"
        );
    }
    let allowed = invocation
        .args
        .iter()
        .position(|arg| arg == "--allowedTools")
        .map(|index| invocation.args[index + 1].clone())
        .unwrap();
    assert_eq!(allowed, "Read,Grep,Glob");
    for forbidden in [
        "--bare",
        "--dangerously-skip-permissions",
        "bypassPermissions",
    ] {
        assert!(
            !invocation.args.iter().any(|arg| arg.contains(forbidden)),
            "forbidden {forbidden}"
        );
    }
}

#[test]
fn the_built_in_tool_set_is_limited_to_the_approved_list_in_both_modes() {
    // `--allowedTools` only approves calls; `--tools` decides which built-in tools exist at all.
    for (access, expected) in [
        (WorkspaceAccess::ReadOnly, "Read,Grep,Glob"),
        (WorkspaceAccess::WorkspaceWrite, "Read,Grep,Glob,Edit,Write"),
    ] {
        let invocation = build_invocation(
            &session_config(access),
            &ExternalId::new(SESSION).unwrap(),
            "C:\\cathulu\\claude-config",
            "C:\\cathulu\\mcp.json",
            &BTreeMap::new(),
        )
        .unwrap();
        let value_after = |flag: &str| {
            let index = invocation.args.iter().position(|arg| arg == flag).unwrap();
            invocation.args[index + 1].clone()
        };
        assert_eq!(value_after("--tools"), expected);
        assert_eq!(value_after("--allowedTools"), expected);
        assert_eq!(
            invocation
                .args
                .iter()
                .filter(|arg| arg.as_str() == "--tools")
                .count(),
            1
        );
        assert!(!value_after("--tools").contains("Bash"));
    }
}

#[test]
fn workspace_write_adds_edit_and_write_but_never_bash() {
    let invocation = build_invocation(
        &session_config(WorkspaceAccess::WorkspaceWrite),
        &ExternalId::new(SESSION).unwrap(),
        "C:\\cathulu\\claude-config",
        "C:\\cathulu\\mcp.json",
        &BTreeMap::new(),
    )
    .unwrap();
    let allowed = invocation
        .args
        .iter()
        .position(|arg| arg == "--allowedTools")
        .map(|index| invocation.args[index + 1].clone())
        .unwrap();
    assert_eq!(allowed, "Read,Grep,Glob,Edit,Write");
    assert!(!allowed.contains("Bash"));
}

#[test]
fn environment_strips_provider_selectors_in_any_case_and_sets_isolation() {
    let invocation = build_invocation(
        &session_config(WorkspaceAccess::ReadOnly),
        &ExternalId::new(SESSION).unwrap(),
        "C:\\cathulu\\claude-config",
        "C:\\cathulu\\mcp.json",
        &parent_env(&[
            ("PATH", "C:\\bin"),
            ("ANTHROPIC_API_KEY", "sk-test"),
            ("anthropic_auth_token", "token"),
            ("ANTHROPIC_BASE_URL", "https://example.invalid"),
            ("CLAUDE_CODE_OAUTH_TOKEN", "oauth"),
            ("claude_code_use_bedrock", "1"),
        ]),
    )
    .unwrap();
    let upper: Vec<String> = invocation
        .env
        .keys()
        .map(|key| key.to_ascii_uppercase())
        .collect();
    assert!(upper.contains(&"PATH".to_string()));
    assert!(!upper.iter().any(|key| key.starts_with("ANTHROPIC_")));
    assert!(!upper.contains(&"CLAUDE_CODE_OAUTH_TOKEN".to_string()));
    assert!(!upper.contains(&"CLAUDE_CODE_USE_BEDROCK".to_string()));
    assert_eq!(
        invocation.env["CLAUDE_CONFIG_DIR"],
        "C:\\cathulu\\claude-config"
    );
    assert_eq!(invocation.env["CLAUDE_CODE_DISABLE_AUTO_MEMORY"], "1");
    assert_eq!(invocation.env["CLAUDE_CODE_DISABLE_BACKGROUND_TASKS"], "1");
    assert_eq!(
        scrub_environment(&parent_env(&[("ANTHROPIC_KEY", "x")])).len(),
        0
    );
}

#[test]
fn non_uuid_session_ids_are_refused_before_any_invocation() {
    let result = build_invocation(
        &session_config(WorkspaceAccess::ReadOnly),
        &ExternalId::new("not-a-uuid").unwrap(),
        "C:\\cathulu\\claude-config",
        "C:\\cathulu\\mcp.json",
        &BTreeMap::new(),
    );
    assert!(result.is_err());
}

#[test]
fn mcp_config_carries_only_the_configured_server_and_its_environment() {
    let invocation = build_invocation(
        &session_config(WorkspaceAccess::ReadOnly),
        &ExternalId::new(SESSION).unwrap(),
        "C:\\cathulu\\claude-config",
        "C:\\cathulu\\mcp.json",
        &BTreeMap::new(),
    )
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&invocation.mcp_config_json).unwrap();
    let servers = parsed["mcpServers"].as_object().unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers["cathulu"]["type"], "stdio");
    assert_eq!(
        servers["cathulu"]["env"]["CATHULU_ROLE_CAPABILITY"],
        "synthetic-token"
    );
}

#[test]
fn clean_stream_completes_only_at_the_end_with_the_result_identity() {
    let events = run(
        &[
            init_ok(),
            r#"{"type":"assistant","message":{},"uuid":"u-a1"}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"result":"done","permission_denials":[],"uuid":"u-res"}"#,
        ],
        Some(0),
    )
    .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].provider_id, ProviderId::claude());
    assert_eq!(events[0].event_id.as_ref().unwrap().as_str(), "u-res");
    match &events[0].event {
        ProviderEventKind::TurnCompleted { turn, outcome } => {
            assert_eq!(*outcome, TurnOutcome::Completed);
            assert_eq!(turn.session.session_id.as_str(), SESSION);
            assert_eq!(turn.turn_id.as_str(), "attempt-1");
        }
        other => panic!("expected TurnCompleted, got {other:?}"),
    }
    for event in &events {
        event.validate().unwrap();
    }
}

#[test]
fn error_or_unproven_results_are_failed_never_completed() {
    for result_line in [
        r#"{"type":"result","subtype":"success","is_error":true,"result":"x","uuid":"u-res"}"#,
        r#"{"type":"result","subtype":"error_max_turns","is_error":false,"uuid":"u-res"}"#,
        r#"{"type":"result","subtype":"success","uuid":"u-res"}"#,
    ] {
        let events = run(&[init_ok(), result_line], Some(0)).unwrap();
        match events.as_slice() {
            [ProviderEvent {
                event: ProviderEventKind::TurnCompleted { outcome, .. },
                ..
            }] => assert_eq!(*outcome, TurnOutcome::Failed, "{result_line}"),
            other => panic!("unexpected {other:?} for {result_line}"),
        }
    }
}

#[test]
fn a_nonzero_exit_never_completes_a_success_result() {
    let success = r#"{"type":"result","subtype":"success","is_error":false,"uuid":"u-res"}"#;
    for exit in [Some(1), Some(2), None] {
        assert!(
            matches!(
                run(&[init_ok(), success], exit),
                Err(ProviderError::Uncertain { .. })
            ),
            "exit {exit:?} must not complete"
        );
    }
}

#[test]
fn an_authentication_failure_printed_as_a_result_is_failed_with_its_exit() {
    // In-run authentication failures reach stdout as the result, with a non-zero exit (CC-04 §2).
    let auth_failure = r#"{"type":"result","subtype":"success","is_error":true,"result":"Invalid API key · Please run /login","uuid":"u-auth"}"#;
    for exit in [Some(1), Some(0)] {
        let events = run(&[init_ok(), auth_failure], exit).unwrap();
        match events.as_slice() {
            [ProviderEvent {
                event: ProviderEventKind::TurnCompleted { outcome, .. },
                ..
            }] => assert_eq!(*outcome, TurnOutcome::Failed, "exit {exit:?}"),
            other => panic!("unexpected {other:?} for exit {exit:?}"),
        }
    }
}

#[test]
fn sigterm_after_a_failed_result_is_still_uncertain() {
    let failed =
        r#"{"type":"result","subtype":"error_during_execution","is_error":true,"uuid":"u-res"}"#;
    assert!(matches!(
        run(&[init_ok(), failed], Some(143)),
        Err(ProviderError::Uncertain { .. })
    ));
}

#[test]
fn a_second_result_line_is_uncertain() {
    let results = map_all(&[
        init_ok(),
        r#"{"type":"result","subtype":"success","is_error":false,"uuid":"u-res"}"#,
        r#"{"type":"result","subtype":"success","is_error":false,"uuid":"u-res-2"}"#,
    ]);
    assert!(matches!(
        results.last().unwrap(),
        Err(ProviderError::Uncertain { .. })
    ));
}

#[test]
fn permission_denials_are_approval_requests_before_completion() {
    let events = run(
        &[
            init_ok(),
            r#"{"type":"result","subtype":"success","is_error":false,"permission_denials":[{"tool_use_id":"toolu_1","tool_name":"Bash"},{"tool_use_id":"toolu_2","tool_name":"Edit"}],"uuid":"u-res"}"#,
        ],
        Some(0),
    )
    .unwrap();
    assert_eq!(events.len(), 3);
    // Each denial keeps a stable event identity taken from its tool use, so replays match.
    assert_eq!(
        events[0].event_id.as_ref().unwrap().as_str(),
        "permission-denied:toolu_1"
    );
    assert_eq!(
        events[1].event_id.as_ref().unwrap().as_str(),
        "permission-denied:toolu_2"
    );
    match (&events[0].event, &events[1].event) {
        (
            ProviderEventKind::ApprovalRequested {
                request_id: first, ..
            },
            ProviderEventKind::ApprovalRequested {
                request_id: second, ..
            },
        ) => {
            assert_eq!(first.as_str(), "toolu_1");
            assert_eq!(second.as_str(), "toolu_2");
        }
        other => panic!("expected two approval requests, got {other:?}"),
    }
    assert_eq!(events[2].event_id.as_ref().unwrap().as_str(), "u-res");
    assert!(matches!(
        events[2].event,
        ProviderEventKind::TurnCompleted { .. }
    ));
}

#[test]
fn a_denial_without_a_tool_use_identifier_is_uncertain() {
    let results = map_all(&[
        init_ok(),
        r#"{"type":"result","subtype":"success","is_error":false,"permission_denials":[{"tool_name":"Edit"}],"uuid":"u-res"}"#,
    ]);
    assert!(matches!(
        results.last().unwrap(),
        Err(ProviderError::Uncertain { .. })
    ));
}

#[test]
fn a_result_without_a_stable_identity_is_uncertain_never_completed() {
    let mut mapper = StreamMapper::new(session(), turn_id(), "cathulu").unwrap();
    mapper.map_line(init_ok()).unwrap();
    assert!(matches!(
        mapper.map_line(r#"{"type":"result","subtype":"success","is_error":false}"#),
        Err(ProviderError::Uncertain { .. })
    ));
    // The refused result must not count as a finished turn.
    assert!(matches!(
        mapper.finish(Some(0)),
        Err(ProviderError::Uncertain { .. })
    ));
}

#[test]
fn a_permission_denied_line_keeps_its_own_identity_or_takes_the_tool_use_identity() {
    let kinds_with_uuid = map_all(&[
        init_ok(),
        r#"{"type":"system","subtype":"permission_denied","tool_use_id":"toolu_3","uuid":"u-pd"}"#,
    ]);
    let event = kinds_with_uuid[1].as_ref().unwrap()[0].clone();
    assert_eq!(event.event_id.as_ref().unwrap().as_str(), "u-pd");

    let without_uuid = map_all(&[
        init_ok(),
        r#"{"type":"system","subtype":"permission_denied","tool_use_id":"toolu_3"}"#,
    ]);
    let event = without_uuid[1].as_ref().unwrap()[0].clone();
    assert_eq!(
        event.event_id.as_ref().unwrap().as_str(),
        "permission-denied:toolu_3"
    );
}

#[test]
fn capacity_retries_are_observations_and_bounded_retry_hints() {
    let kinds = kinds(map_all(&[
        init_ok(),
        r#"{"type":"system","subtype":"api_retry","error":"rate_limit","retry_delay_ms":1500,"attempt":1}"#,
    ]));
    assert_eq!(
        kinds,
        vec![ProviderEventKind::CapacityObserved {
            retry_after_seconds: Some(2)
        }]
    );
}

#[test]
fn auth_billing_and_request_errors_stay_distinct_and_terminal() {
    let cases = [
        ("authentication_failed", "NotAuthenticated"),
        ("billing_error", "AccountRestricted"),
        ("server_error", "Transport"),
    ];
    for (category, expected) in cases {
        let line = format!(r#"{{"type":"system","subtype":"api_retry","error":"{category}"}}"#);
        let error = map_all(&[init_ok(), &line])
            .pop()
            .unwrap()
            .expect_err("terminal category");
        match error {
            ProviderError::Unavailable { reason, .. } => {
                assert_eq!(format!("{reason:?}"), expected, "{category}")
            }
            other => panic!("{category}: unexpected {other:?}"),
        }
    }
    let rejected = map_all(&[
        init_ok(),
        r#"{"type":"system","subtype":"api_retry","error":"invalid_request"}"#,
    ])
    .pop()
    .unwrap();
    assert!(matches!(rejected, Err(ProviderError::Rejected { .. })));
}

#[test]
fn isolation_and_connection_failures_are_refused_at_init() {
    let cases: &[(&str, &str)] = &[
        (
            r#"{"type":"system","subtype":"init","session_id":"3f2c9a1e-6b7d-4c8e-9f0a-1b2c3d4e5f60","mcp_servers":[{"name":"cathulu","status":"connected"}],"mcp_server_errors":[{"name":"x","type":"unknown_type","message":"m"}]}"#,
            "Transport",
        ),
        (
            r#"{"type":"system","subtype":"init","session_id":"3f2c9a1e-6b7d-4c8e-9f0a-1b2c3d4e5f60","mcp_servers":[{"name":"cathulu","status":"connected"}],"plugin_errors":[{"plugin":"p","type":"load","message":"m"}]}"#,
            "Rejected",
        ),
        (
            r#"{"type":"system","subtype":"init","session_id":"3f2c9a1e-6b7d-4c8e-9f0a-1b2c3d4e5f60","mcp_servers":[{"name":"cathulu","status":"connected"}],"plugins":[{"name":"p","path":"/p"}]}"#,
            "Rejected",
        ),
        (
            r#"{"type":"system","subtype":"init","session_id":"3f2c9a1e-6b7d-4c8e-9f0a-1b2c3d4e5f60","mcp_servers":[{"name":"cathulu","status":"pending"}]}"#,
            "Transport",
        ),
    ];
    for (line, expected) in cases {
        let error = StreamMapper::new(session(), turn_id(), "cathulu")
            .unwrap()
            .map_line(line)
            .expect_err("init must refuse");
        let label = match error {
            ProviderError::Unavailable { reason, .. } => format!("{reason:?}"),
            ProviderError::Rejected { .. } => "Rejected".to_string(),
            other => format!("{other:?}"),
        };
        assert_eq!(label, *expected, "{line}");
    }
}

#[test]
fn a_mapper_refuses_a_session_that_is_not_claude() {
    let codex = ExternalSession {
        provider_id: ProviderId::codex(),
        session_id: ExternalId::new(SESSION).unwrap(),
    };
    assert!(matches!(
        StreamMapper::new(codex, turn_id(), "cathulu"),
        Err(ProviderError::Rejected { .. })
    ));
}

#[test]
fn a_session_identity_that_differs_from_the_selected_session_is_refused() {
    let other_init = format!(
        r#"{{"type":"system","subtype":"init","session_id":"{OTHER_SESSION}","mcp_servers":[{{"name":"cathulu","status":"connected"}}]}}"#
    );
    assert!(matches!(
        map_all(&[&other_init]).pop().unwrap(),
        Err(ProviderError::Rejected { .. })
    ));

    let unnamed_init = r#"{"type":"system","subtype":"init","mcp_servers":[{"name":"cathulu","status":"connected"}]}"#;
    assert!(matches!(
        map_all(&[unnamed_init]).pop().unwrap(),
        Err(ProviderError::Rejected { .. })
    ));

    let other_result = format!(
        r#"{{"type":"result","subtype":"success","is_error":false,"session_id":"{OTHER_SESSION}","uuid":"u-res"}}"#
    );
    assert!(matches!(
        map_all(&[init_ok(), &other_result]).pop().unwrap(),
        Err(ProviderError::Rejected { .. })
    ));
}

#[test]
fn result_before_a_clean_init_is_uncertain() {
    let error = map_all(&[r#"{"type":"result","subtype":"success","is_error":false}"#])
        .pop()
        .unwrap()
        .expect_err("result without init");
    assert!(matches!(error, ProviderError::Uncertain { .. }));
}

#[test]
fn truncated_lines_are_uncertain_and_unknown_types_are_bounded_observations() {
    let truncated = map_all(&[init_ok(), r#"{"type":"result","subtype":"succ"#])
        .pop()
        .unwrap();
    assert!(matches!(truncated, Err(ProviderError::Uncertain { .. })));

    let long = format!(r#"{{"type":"novel","text":"{}"}}"#, "é".repeat(400));
    let kinds = kinds(map_all(&[init_ok(), &long]));
    match kinds.as_slice() {
        [ProviderEventKind::Unrecognized {
            event_kind,
            size_bytes,
            excerpt: Some(excerpt),
        }] => {
            assert_eq!(event_kind, "novel");
            assert_eq!(*size_bytes as usize, long.len());
            assert!(excerpt.len() <= 512);
            assert!(excerpt.is_char_boundary(excerpt.len()));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn an_oversized_line_is_refused_before_it_is_parsed_or_buffered() {
    let oversized = "a".repeat(MAX_STREAM_LINE_BYTES + 1);
    let mut mapper = StreamMapper::new(session(), turn_id(), "cathulu").unwrap();
    assert!(matches!(
        mapper.map_line(&oversized),
        Err(ProviderError::Uncertain { .. })
    ));

    let mut reader = Cursor::new(vec![b'a'; MAX_STREAM_LINE_BYTES + 1]);
    assert!(matches!(
        read_bounded_line(&mut reader),
        Err(ProviderError::Uncertain { .. })
    ));

    // A line exactly at the cap is accepted by the reader; the cap is an upper bound, not a refusal.
    let mut at_cap = vec![b'a'; MAX_STREAM_LINE_BYTES];
    at_cap.push(b'\n');
    let mut reader = Cursor::new(at_cap);
    let line = read_bounded_line(&mut reader).unwrap().unwrap();
    assert_eq!(line.len(), MAX_STREAM_LINE_BYTES);
}

#[test]
fn the_reader_joins_split_chunks_strips_carriage_returns_and_ends_cleanly() {
    let mut reader = BufReader::with_capacity(3, Cursor::new(b"first\r\nsecond\n".to_vec()));
    assert_eq!(
        read_bounded_line(&mut reader).unwrap().as_deref(),
        Some("first")
    );
    assert_eq!(
        read_bounded_line(&mut reader).unwrap().as_deref(),
        Some("second")
    );
    assert_eq!(read_bounded_line(&mut reader).unwrap(), None);
}

#[test]
fn process_end_without_a_result_or_on_sigterm_is_uncertain_never_success() {
    let mut without_result = StreamMapper::new(session(), turn_id(), "cathulu").unwrap();
    without_result.map_line(init_ok()).unwrap();
    assert!(matches!(
        without_result.finish(Some(0)),
        Err(ProviderError::Uncertain { .. })
    ));

    let mut sigterm = StreamMapper::new(session(), turn_id(), "cathulu").unwrap();
    sigterm.map_line(init_ok()).unwrap();
    sigterm
        .map_line(r#"{"type":"result","subtype":"success","is_error":false,"uuid":"u-res"}"#)
        .unwrap();
    assert!(matches!(
        sigterm.finish(Some(143)),
        Err(ProviderError::Uncertain { .. })
    ));

    let mut finished = StreamMapper::new(session(), turn_id(), "cathulu").unwrap();
    finished.map_line(init_ok()).unwrap();
    finished
        .map_line(r#"{"type":"result","subtype":"success","is_error":false,"uuid":"u-res"}"#)
        .unwrap();
    assert!(finished.finish(Some(0)).is_ok());
    // An unknown exit status cannot prove a success, even after a success result.
    assert!(matches!(
        finished.finish(None),
        Err(ProviderError::Uncertain { .. })
    ));
}

#[test]
fn adapter_capabilities_report_only_what_is_verified() {
    let capabilities = adapter_capabilities(None);
    capabilities.validate().unwrap();
    assert_eq!(capabilities.interrupt, FeatureSupport::Unsupported);
    assert_eq!(capabilities.credential_source, CredentialSource::Unknown);
    assert!(!capabilities.isolation.verified());
    assert_eq!(capabilities.isolation.plugins, FeatureSupport::Unverified);
}
