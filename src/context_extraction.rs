use crate::{
    events::append_external_event,
    hash::content_hash,
    optimization::{
        ContextCapability, ContextModel, ContextRequest, ContextResult, ContextSource,
        ContextSourceSpan, EffectiveOptimizationMode, InvocationStatus, ModelCallFailure,
        OptimizationCapability, OptimizationInvocation, ProviderLocality, ShadowTrialReceipt,
    },
    workflow::{sqlite_error, EventRecord, EvidenceArtifact},
    Store,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeSet, fs, path::PathBuf, time::Instant};

pub const CONTEXT_EXTRACTION_SCHEMA_VERSION: u16 = 2;
pub const MAX_CONTEXT_SHADOW_OUTPUT_BYTES: usize = 8 * 1024;
const CONTEXT_EXTRACTION_TASK: &str = "extract_test_compiler_ci_tool_failures";
const MAX_CONTEXT_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
const MAX_CONTEXT_SELECTED_BYTES: usize = 128 * 1024;
const MAX_CONTEXT_SELECTED_SPANS: usize = 128;
const MAX_CONTEXT_CANDIDATES: usize = 64;
const MAX_CONTEXT_BASELINE_CANDIDATES: usize = 32;
const MAX_CONTEXT_EVENT_BYTES: usize = 30 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextExtractionCandidateKind {
    TestFailure,
    Error,
    Panic,
    Location,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextExtractionCandidate {
    pub kind: ContextExtractionCandidateKind,
    pub summary: String,
    /// Exact text copied from the source range. This makes each claim auditable.
    pub quote: String,
    pub source_span: ContextSourceSpan,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextExtractionContent {
    candidates: Vec<ContextExtractionCandidate>,
}

struct LoadedContextSource {
    source: ContextSource,
    raw_content: String,
    raw_bytes: u64,
}

struct ContextExtractionShadowEvidence<'a> {
    run_id: i64,
    invocation_id: &'a str,
    request: &'a ContextRequest,
    loaded_sources: &'a [LoadedContextSource],
    baseline_candidates: &'a [ContextExtractionCandidate],
    result: &'a ContextResult,
    input_bytes: u64,
}

impl Store {
    /// Builds a versioned extraction request from registered, local test,
    /// compiler, CI, or tool output artifacts. Only selected error-adjacent
    /// source spans are included in the request; the original file is read-only.
    pub fn prepare_context_extraction_request(
        &self,
        artifact_ids: &[i64],
    ) -> Result<ContextRequest, String> {
        let sources = self.load_context_sources(artifact_ids)?;
        let request = ContextRequest {
            capability: ContextCapability::Extract,
            schema_version: CONTEXT_EXTRACTION_SCHEMA_VERSION,
            task: CONTEXT_EXTRACTION_TASK.to_string(),
            sources: sources.into_iter().map(|source| source.source).collect(),
            max_output_bytes: MAX_CONTEXT_SHADOW_OUTPUT_BYTES,
        };
        request.validate()?;
        Ok(request)
    }

    /// Returns the deterministic comparison candidates for a request without
    /// calling a model or changing run state.
    pub fn deterministic_context_extraction(
        &self,
        request: &ContextRequest,
    ) -> Result<Vec<ContextExtractionCandidate>, String> {
        let sources = self.verify_context_extraction_request(request)?;
        Ok(deterministic_candidates(&sources))
    }

    /// Runs a local-only ContextModel shadow trial. It returns an audit receipt,
    /// never the candidate output, and writes no workflow, QA, or memory state.
    pub fn run_context_extraction_shadow_trial(
        &self,
        run_id: i64,
        invocation_id: &str,
        request: &ContextRequest,
        model: &dyn ContextModel,
    ) -> Result<Option<ShadowTrialReceipt>, String> {
        request.validate()?;
        if request.capability != ContextCapability::Extract
            || request.schema_version != CONTEXT_EXTRACTION_SCHEMA_VERSION
            || request.task != CONTEXT_EXTRACTION_TASK
            || request.max_output_bytes != MAX_CONTEXT_SHADOW_OUTPUT_BYTES
        {
            return Err("M17 accepts only its bounded context-extraction schema".to_string());
        }

        let treatment = self
            .get_run_optimization_treatment(run_id)?
            .ok_or_else(|| format!("run {run_id} has no optimization treatment snapshot"))?;
        let settings = treatment
            .configuration
            .settings(OptimizationCapability::ContextExtract);
        match settings.effective_mode() {
            EffectiveOptimizationMode::Disabled => return Ok(None),
            EffectiveOptimizationMode::Active => {
                return Err("M17 context extraction is shadow-only until M18 approval".to_string())
            }
            EffectiveOptimizationMode::Shadow => {}
        }
        let run = self
            .get_run(run_id)?
            .ok_or_else(|| format!("run {run_id} was not found"))?;
        if run.finished_at.is_some() || run.status != "running" {
            return Err("context shadow trials require a currently running task".to_string());
        }

        let loaded_sources = self.verify_context_extraction_request(request)?;
        let baseline_candidates = deterministic_candidates(&loaded_sources);
        let input_bytes = serde_json::to_vec(request)
            .map_err(|error| format!("cannot size context request: {error}"))?
            .len() as u64;
        let provider = settings
            .provider
            .clone()
            .ok_or_else(|| "shadow context capability has no configured provider".to_string())?;
        let selected_input_bytes = request
            .sources
            .iter()
            .map(|source| source.content.len() as u64)
            .sum::<u64>();

        let locality = settings.provider_locality.ok_or_else(|| {
            "shadow context capability has no configured provider locality".to_string()
        })?;
        let mut output_result = None;
        let mut output_bytes = 0u64;
        let mut model_confidence = None;
        let mut status = InvocationStatus::Completed;
        let mut latency_ms = 0u64;

        if locality == ProviderLocality::Remote {
            // M17 has no reviewed remote extraction adapter or credential path.
            // A local artifact reference is not sufficient permission to send it.
            status = InvocationStatus::Skipped;
        } else if selected_input_bytes == 0 {
            status = InvocationStatus::Skipped;
        } else {
            let started = Instant::now();
            let response = model.process(request);
            latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            match response {
                Ok(Some(result)) => {
                    let response_bytes = serde_json::to_vec(&result)
                        .map(|bytes| bytes.len() as u64)
                        .unwrap_or(0);
                    let still_current = self.verify_context_extraction_request(request).is_ok();
                    if !still_current {
                        status = InvocationStatus::InvalidResponse;
                        output_bytes = response_bytes;
                    } else {
                        match validate_extraction_result(request, &loaded_sources, &result) {
                            Ok(_) => {
                                output_bytes = response_bytes;
                                model_confidence = result.model_confidence;
                                output_result = Some(result);
                            }
                            Err(_) => {
                                status = InvocationStatus::InvalidResponse;
                                output_bytes = response_bytes;
                            }
                        }
                    }
                }
                Ok(None) => status = InvocationStatus::Skipped,
                Err(ModelCallFailure::ProviderUnavailable) => {
                    status = InvocationStatus::ProviderError
                }
                Err(ModelCallFailure::Timeout) => status = InvocationStatus::Timeout,
                Err(ModelCallFailure::InvalidResponse) => {
                    status = InvocationStatus::InvalidResponse
                }
            }
        }

        let invocation = OptimizationInvocation {
            invocation_id: invocation_id.to_string(),
            capability: OptimizationCapability::ContextExtract,
            provider,
            model: settings.model,
            model_version: None,
            provider_locality: locality,
            remote_data_allowed: settings.remote_data_allowed,
            schema_version: request.schema_version,
            mode: EffectiveOptimizationMode::Shadow,
            input_bytes,
            output_bytes,
            input_tokens: None,
            output_tokens: None,
            latency_ms,
            status,
            prediction: output_result
                .as_ref()
                .map(|_| "extracted_candidates".to_string()),
            model_confidence,
            probabilities: Default::default(),
            fallback_used: status != InvocationStatus::Completed,
            influenced_execution: false,
        };
        let invocation_event = self.record_optimization_invocation(run_id, &invocation)?;
        if let Some(result) = output_result {
            self.record_context_extraction_shadow_output(ContextExtractionShadowEvidence {
                run_id,
                invocation_id,
                request,
                loaded_sources: &loaded_sources,
                baseline_candidates: &baseline_candidates,
                result: &result,
                input_bytes,
            })?;
        }
        Ok(Some(ShadowTrialReceipt {
            event_id: invocation_event.id,
            status,
        }))
    }

    fn verify_context_extraction_request(
        &self,
        request: &ContextRequest,
    ) -> Result<Vec<LoadedContextSource>, String> {
        request.validate()?;
        let ids = request
            .sources
            .iter()
            .map(|source| source.artifact_id)
            .collect::<Vec<_>>();
        let loaded = self.load_context_sources(&ids)?;
        let expected = ContextRequest {
            capability: ContextCapability::Extract,
            schema_version: CONTEXT_EXTRACTION_SCHEMA_VERSION,
            task: CONTEXT_EXTRACTION_TASK.to_string(),
            sources: loaded.iter().map(|source| source.source.clone()).collect(),
            max_output_bytes: MAX_CONTEXT_SHADOW_OUTPUT_BYTES,
        };
        if &expected != request {
            return Err(
                "context request does not match the current minimized artifact projection"
                    .to_string(),
            );
        }
        Ok(loaded)
    }

    fn load_context_sources(
        &self,
        artifact_ids: &[i64],
    ) -> Result<Vec<LoadedContextSource>, String> {
        if artifact_ids.is_empty() || artifact_ids.len() > 32 {
            return Err("context extraction needs between 1 and 32 artifact IDs".to_string());
        }
        let mut sorted_ids = artifact_ids.to_vec();
        sorted_ids.sort_unstable();
        if sorted_ids.iter().any(|id| *id <= 0)
            || sorted_ids.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(
                "context extraction artifact IDs must be unique positive integers".to_string(),
            );
        }

        sorted_ids
            .into_iter()
            .map(|artifact_id| {
                let artifact = self
                    .get_artifact(artifact_id)?
                    .ok_or_else(|| format!("context artifact {artifact_id} was not found"))?;
                load_local_context_source(artifact)
            })
            .collect()
    }

    fn record_context_extraction_shadow_output(
        &self,
        evidence: ContextExtractionShadowEvidence<'_>,
    ) -> Result<EventRecord, String> {
        let ContextExtractionShadowEvidence {
            run_id,
            invocation_id,
            request,
            loaded_sources,
            baseline_candidates,
            result,
            input_bytes,
        } = evidence;
        let result_json = serde_json::to_vec(result)
            .map_err(|error| format!("cannot serialize extraction result: {error}"))?;
        let baseline_json = serde_json::to_vec(baseline_candidates)
            .map_err(|error| format!("cannot serialize deterministic candidates: {error}"))?;
        let selected_input_json = serde_json::to_vec(&request.sources)
            .map_err(|error| format!("cannot hash selected extraction input: {error}"))?;
        let model_content: ContextExtractionContent =
            serde_json::from_value(result.content.clone())
                .map_err(|error| format!("cannot read validated extraction result: {error}"))?;
        let baseline_spans = baseline_candidates
            .iter()
            .map(|candidate| {
                (
                    candidate.source_span.artifact_id,
                    candidate.source_span.start_byte,
                    candidate.source_span.end_byte,
                )
            })
            .collect::<BTreeSet<_>>();
        let matched_baseline_candidates = model_content
            .candidates
            .iter()
            .filter(|candidate| {
                baseline_spans.contains(&(
                    candidate.source_span.artifact_id,
                    candidate.source_span.start_byte,
                    candidate.source_span.end_byte,
                ))
            })
            .count();
        let payload = json!({
            "extractionEventSchemaVersion": 1,
            "contextSchemaVersion": result.schema_version,
            "invocationId": invocation_id,
            "sourceArtifacts": request.sources.iter().map(|source| json!({
                "artifactId": source.artifact_id,
                "contentHash": source.content_hash,
                "sourceSpanCount": source.source_spans.len()
            })).collect::<Vec<_>>(),
            "rawSourceBytes": loaded_sources.iter().map(|source| source.raw_bytes).sum::<u64>(),
            "selectedInputBytes": request.sources.iter().map(|source| source.content.len() as u64).sum::<u64>(),
            "selectedInputHash": content_hash(std::str::from_utf8(&selected_input_json).unwrap_or("")),
            "inputBytes": input_bytes,
            "baselineCandidateCount": baseline_candidates.len(),
            "baselineCandidateHash": content_hash(std::str::from_utf8(&baseline_json).unwrap_or("")),
            "candidateCount": model_content.candidates.len(),
            "candidateSpanMatches": matched_baseline_candidates,
            "baselineCandidatesMissingFromModel": baseline_candidates.len().saturating_sub(matched_baseline_candidates),
            "modelCandidatesOutsideBaseline": model_content.candidates.len().saturating_sub(matched_baseline_candidates),
            "resultHash": content_hash(std::str::from_utf8(&result_json).unwrap_or("")),
            "result": result,
            "influencedExecution": false,
            "rawSourceOverwritten": false
        });
        let payload_bytes = serde_json::to_vec(&payload)
            .map_err(|error| format!("cannot serialize context shadow output event: {error}"))?;
        if payload_bytes.len() > MAX_CONTEXT_EVENT_BYTES {
            return Err("context shadow output event exceeds the 30 KiB limit".to_string());
        }
        let external_event_id = format!(
            "context-extraction:{}",
            content_hash(&format!("{run_id}:{invocation_id}"))
        );
        let now = crate::workflow::now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let inserted_id = append_external_event(
            &transaction,
            "run",
            run_id,
            "context_extraction_shadow_recorded",
            &external_event_id,
            &payload,
            now,
        )?;
        let event_id = if let Some(event_id) = inserted_id {
            event_id
        } else {
            let (event_id, existing_payload): (i64, String) = transaction
                .query_row(
                    "SELECT id, payload_json FROM events WHERE external_event_id = ?1",
                    params![external_event_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(sqlite_error)?;
            let requested_payload = serde_json::to_string(&payload)
                .map_err(|error| format!("cannot serialize context shadow event: {error}"))?;
            if requested_payload != existing_payload {
                return Err(
                    "context extraction invocation ID was reused with different output".to_string(),
                );
            }
            event_id
        };
        let event = transaction
            .query_row(
                "SELECT id, aggregate_type, aggregate_id, event_type, payload_json, created_at
                 FROM events WHERE id = ?1",
                params![event_id],
                crate::workflow::event_from_row,
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(event)
    }
}

fn load_local_context_source(artifact: EvidenceArtifact) -> Result<LoadedContextSource, String> {
    if artifact.archived_at.is_some() {
        return Err(format!("context artifact {} is archived", artifact.id));
    }
    let kind = artifact
        .artifact_kind
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    if !matches!(
        kind.as_str(),
        "test_log" | "compiler_log" | "ci_log" | "tool_output" | "tool_result"
    ) {
        return Err(format!(
            "artifact {} is outside the M17 test/compiler/CI/tool-output corpus",
            artifact.id
        ));
    }
    let path = local_artifact_path(&artifact.uri)?;
    let metadata = fs::metadata(&path)
        .map_err(|error| format!("cannot inspect context artifact {}: {error}", artifact.id))?;
    if !metadata.is_file() || metadata.len() > MAX_CONTEXT_ARTIFACT_BYTES as u64 {
        return Err(format!(
            "context artifact {} must be a local regular file no larger than 16 MiB",
            artifact.id
        ));
    }
    let canonical_path = fs::canonicalize(&path)
        .map_err(|error| format!("cannot resolve context artifact {}: {error}", artifact.id))?;
    let bytes = fs::read(&canonical_path)
        .map_err(|error| format!("cannot read context artifact {}: {error}", artifact.id))?;
    if bytes.len() > MAX_CONTEXT_ARTIFACT_BYTES {
        return Err(format!(
            "context artifact {} exceeds the 16 MiB limit",
            artifact.id
        ));
    }
    let raw_content = String::from_utf8(bytes)
        .map_err(|_| format!("context artifact {} is not valid UTF-8", artifact.id))?;
    let hash = content_hash(&raw_content);
    if let Some(recorded_hash) = artifact.content_hash.as_deref() {
        let recorded_hash = recorded_hash
            .strip_prefix("sha256:")
            .unwrap_or(recorded_hash);
        if recorded_hash.len() != 64
            || !recorded_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !recorded_hash.eq_ignore_ascii_case(&hash)
        {
            return Err(format!(
                "context artifact {} no longer matches its recorded SHA-256 hash",
                artifact.id
            ));
        }
    }
    let source_spans = select_error_adjacent_spans(&raw_content, artifact.id);
    let excerpt = source_spans
        .iter()
        .map(|span| {
            raw_content
                .as_bytes()
                .get(span.start_byte as usize..span.end_byte as usize)
                .ok_or_else(|| format!("context artifact {} has an invalid span", artifact.id))
                .and_then(|bytes| {
                    std::str::from_utf8(bytes)
                        .map(str::to_string)
                        .map_err(|_| "selected context span is not valid UTF-8".to_string())
                })
        })
        .collect::<Result<Vec<_>, _>>()?
        .concat();

    Ok(LoadedContextSource {
        source: ContextSource {
            artifact_id: artifact.id,
            content_hash: hash,
            // Keep filesystem paths inside the local store boundary. The
            // stable Cathulu resource ID is sufficient for provenance.
            source_ref: format!("cathulu://artifacts/{}", artifact.id),
            content: excerpt,
            source_spans,
        },
        raw_content: raw_content.clone(),
        raw_bytes: raw_content.len() as u64,
    })
}

fn local_artifact_path(uri: &str) -> Result<PathBuf, String> {
    let path_text = if let Some(rest) = uri.strip_prefix("file://") {
        let local_path = if let Some(path) = rest.strip_prefix("localhost") {
            path
        } else if rest.starts_with('/') {
            rest
        } else {
            return Err("remote file hosts are not valid context sources".to_string());
        };
        percent_decode_path(local_path)?
    } else {
        if uri.starts_with("\\\\")
            || uri.starts_with("//")
            || uri.contains("://")
            || uri.starts_with("artifact:")
        {
            return Err("context extraction requires a local file artifact".to_string());
        }
        uri.to_string()
    };
    let path_text = if cfg!(windows)
        && path_text.len() >= 3
        && path_text.as_bytes()[0] == b'/'
        && path_text.as_bytes()[2] == b':'
    {
        path_text[1..].to_string()
    } else {
        path_text
    };
    if path_text.contains('\0') {
        return Err("context artifact path contains a NUL byte".to_string());
    }
    let path = PathBuf::from(path_text);
    if !path.is_absolute() {
        return Err("context artifact path must be absolute".to_string());
    }
    Ok(path)
}

fn percent_decode_path(path: &str) -> Result<String, String> {
    let input = path.as_bytes();
    let mut decoded = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' {
            if index + 2 >= input.len() {
                return Err("context file URI has an invalid percent escape".to_string());
            }
            let high = hex_value(input[index + 1])?;
            let low = hex_value(input[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(input[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| "context file URI is not valid UTF-8".to_string())
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("context file URI has an invalid percent escape".to_string()),
    }
}

fn select_error_adjacent_spans(content: &str, artifact_id: i64) -> Vec<ContextSourceSpan> {
    let bytes = content.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            lines.push((start, index + 1));
            start = index + 1;
        }
    }
    if start < bytes.len() {
        lines.push((start, bytes.len()));
    }
    if lines.is_empty() {
        return Vec::new();
    }

    let mut selected = vec![false; lines.len()];
    for (index, (start, end)) in lines.iter().enumerate() {
        let Some(line) = content.get(*start..*end) else {
            continue;
        };
        if is_relevant_line(line) {
            let first = index.saturating_sub(1);
            let last = (index + 1).min(lines.len() - 1);
            selected[first..=last].fill(true);
        }
    }

    let mut merged = Vec::<(usize, usize)>::new();
    for (index, is_selected) in selected.into_iter().enumerate() {
        if !is_selected {
            continue;
        }
        let range = lines[index];
        if let Some(last) = merged.last_mut() {
            if range.0 <= last.1 {
                last.1 = range.1;
                continue;
            }
        }
        merged.push(range);
    }

    let mut spans = Vec::new();
    let mut selected_bytes = 0usize;
    for (start, end) in merged {
        let length = end.saturating_sub(start);
        if spans.len() >= MAX_CONTEXT_SELECTED_SPANS
            || selected_bytes.saturating_add(length) > MAX_CONTEXT_SELECTED_BYTES
        {
            continue;
        }
        spans.push(ContextSourceSpan {
            artifact_id,
            start_byte: start as u64,
            end_byte: end as u64,
        });
        selected_bytes += length;
    }
    spans
}

fn is_relevant_line(line: &str) -> bool {
    let line = line.to_ascii_lowercase();
    [
        "error",
        "failed",
        "failure",
        "panic",
        "exception",
        "assertion",
    ]
    .iter()
    .any(|needle| line.contains(needle))
}

fn deterministic_candidates(sources: &[LoadedContextSource]) -> Vec<ContextExtractionCandidate> {
    let mut candidates = Vec::new();
    for source in sources {
        for source_span in &source.source.source_spans {
            let Some(span_content) = source
                .raw_content
                .get(source_span.start_byte as usize..source_span.end_byte as usize)
            else {
                continue;
            };
            let span_bytes = span_content.as_bytes();
            let mut line_start = 0usize;
            for (index, byte) in span_bytes.iter().enumerate() {
                if *byte != b'\n' {
                    continue;
                }
                push_candidate_for_line(
                    &mut candidates,
                    source,
                    source_span.start_byte as usize + line_start,
                    source_span.start_byte as usize + index,
                );
                line_start = index + 1;
            }
            if line_start < span_bytes.len() {
                push_candidate_for_line(
                    &mut candidates,
                    source,
                    source_span.start_byte as usize + line_start,
                    source_span.end_byte as usize,
                );
            }
            if candidates.len() >= MAX_CONTEXT_BASELINE_CANDIDATES {
                return candidates;
            }
        }
    }
    candidates
}

fn push_candidate_for_line(
    candidates: &mut Vec<ContextExtractionCandidate>,
    source: &LoadedContextSource,
    start: usize,
    end: usize,
) {
    if candidates.len() >= MAX_CONTEXT_BASELINE_CANDIDATES || start >= end {
        return;
    }
    let Some(line) = source.raw_content.get(start..end) else {
        return;
    };
    let Some(kind) = classify_line(line) else {
        return;
    };
    let summary = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if summary.is_empty() || summary.len() > 512 || line.len() > 4096 {
        return;
    }
    candidates.push(ContextExtractionCandidate {
        kind,
        summary,
        quote: line.to_string(),
        source_span: ContextSourceSpan {
            artifact_id: source.source.artifact_id,
            start_byte: start as u64,
            end_byte: end as u64,
        },
    });
}

fn classify_line(line: &str) -> Option<ContextExtractionCandidateKind> {
    let normalized = line.to_ascii_lowercase();
    if (normalized.contains("test ") || normalized.contains("test result"))
        && (normalized.contains("failed") || normalized.contains("failure"))
    {
        Some(ContextExtractionCandidateKind::TestFailure)
    } else if normalized.contains("panic") {
        Some(ContextExtractionCandidateKind::Panic)
    } else if normalized.contains("error")
        || normalized.contains("exception")
        || normalized.contains("assertion")
    {
        Some(ContextExtractionCandidateKind::Error)
    } else if looks_like_source_location(line) {
        Some(ContextExtractionCandidateKind::Location)
    } else {
        None
    }
}

fn looks_like_source_location(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b':' || index == 0 {
            continue;
        }
        let mut cursor = index + 1;
        let digits_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor > digits_start && cursor < bytes.len() && bytes[cursor] == b':' {
            return true;
        }
    }
    false
}

fn validate_extraction_result(
    request: &ContextRequest,
    sources: &[LoadedContextSource],
    result: &ContextResult,
) -> Result<ContextExtractionContent, String> {
    if result.schema_version != CONTEXT_EXTRACTION_SCHEMA_VERSION {
        return Err("context extraction result schema version does not match".to_string());
    }
    if result.source_artifact_ids
        != request
            .sources
            .iter()
            .map(|source| source.artifact_id)
            .collect::<Vec<_>>()
        || result.source_content_hashes
            != request
                .sources
                .iter()
                .map(|source| source.content_hash.clone())
                .collect::<Vec<_>>()
    {
        return Err("context extraction provenance does not match its request".to_string());
    }
    if result
        .model_confidence
        .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
    {
        return Err("context model confidence is outside the 0 to 1 range".to_string());
    }
    let content: ContextExtractionContent = serde_json::from_value(result.content.clone())
        .map_err(|error| format!("context extraction content is invalid: {error}"))?;
    if content.candidates.len() > MAX_CONTEXT_CANDIDATES {
        return Err("context extraction exceeds the 64 candidate limit".to_string());
    }
    let candidate_spans = content
        .candidates
        .iter()
        .map(|candidate| candidate.source_span.clone())
        .collect::<Vec<_>>();
    if candidate_spans != result.source_spans {
        return Err("context candidate spans do not match the provenance span list".to_string());
    }

    let mut seen_spans = BTreeSet::new();
    for candidate in &content.candidates {
        if candidate.summary.trim().is_empty()
            || candidate.summary.len() > 512
            || candidate.summary.chars().any(char::is_control)
            || candidate.quote.is_empty()
            || candidate.quote.len() > 4096
            || candidate.quote.contains('\0')
        {
            return Err("context candidate text is empty, too long, or malformed".to_string());
        }
        let span = &candidate.source_span;
        if span.start_byte >= span.end_byte
            || !seen_spans.insert((span.artifact_id, span.start_byte, span.end_byte))
        {
            return Err("context candidate spans must be unique non-empty ranges".to_string());
        }
        let request_source = request
            .sources
            .iter()
            .find(|source| source.artifact_id == span.artifact_id)
            .ok_or_else(|| "context candidate refers to an unrequested artifact".to_string())?;
        if !request_source.source_spans.iter().any(|selected| {
            selected.start_byte <= span.start_byte && span.end_byte <= selected.end_byte
        }) {
            return Err(
                "context candidate span falls outside the selected source excerpts".to_string(),
            );
        }
        let loaded_source = sources
            .iter()
            .find(|source| source.source.artifact_id == span.artifact_id)
            .ok_or_else(|| "context source artifact could not be reopened".to_string())?;
        let quoted_source = loaded_source
            .raw_content
            .as_bytes()
            .get(span.start_byte as usize..span.end_byte as usize)
            .ok_or_else(|| "context candidate span exceeds the source artifact".to_string())?;
        if quoted_source != candidate.quote.as_bytes() {
            return Err("context candidate quote does not match its source byte span".to_string());
        }
    }
    let result_bytes = serde_json::to_vec(result)
        .map_err(|error| format!("cannot size context extraction result: {error}"))?;
    if result_bytes.len() > request.max_output_bytes
        || result_bytes.len() > MAX_CONTEXT_SHADOW_OUTPUT_BYTES
    {
        return Err("context extraction result exceeds its bounded output size".to_string());
    }
    Ok(content)
}
