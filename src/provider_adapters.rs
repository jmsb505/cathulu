use crate::{
    context_extraction::{ContextExtractionCandidate, ContextExtractionCandidateKind},
    optimization::{
        ContextModel, ContextModelFuture, ContextRequest, ContextResult, ContextSourceSpan,
        DecisionModel, DecisionModelFuture, DecisionRequest, DecisionResult, ModelCallFailure,
        ModelResponse,
    },
    provider_credentials,
};
use reqwest::{redirect::Policy, Client, Response};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeSet, time::Duration};

const JEV_ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const SCALEDOWN_ENDPOINT: &str = "https://api.scaledown.xyz/v1/chat/completions";
const JEV_DEFAULT_MODEL: &str = "jev-latest";
const SCALEDOWN_DEFAULT_MODEL: &str = "extract-1";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_SCALEDOWN_INPUT_BYTES: usize = 128 * 1024;
const MAX_SCALEDOWN_CANDIDATES: usize = 64;

pub struct JevProvider {
    client: Client,
    api_key: String,
    model: String,
    endpoint: String,
}

impl JevProvider {
    pub fn from_environment(model: Option<&str>) -> Option<Self> {
        let api_key = provider_credentials::decision_api_key()?;
        Self::new(api_key, model.unwrap_or(JEV_DEFAULT_MODEL), JEV_ENDPOINT).ok()
    }

    fn new(api_key: String, model: &str, endpoint: &str) -> Result<Self, ModelCallFailure> {
        let api_key = clean_credential(api_key)?;
        let model = clean_model(model)?;
        let client = http_client()?;
        Ok(Self {
            client,
            api_key,
            model,
            endpoint: endpoint.to_string(),
        })
    }
}

impl DecisionModel for JevProvider {
    fn decide<'a>(&'a self, request: &'a DecisionRequest) -> DecisionModelFuture<'a> {
        Box::pin(async move {
            request
                .validate()
                .map_err(|_| ModelCallFailure::InvalidResponse)?;
            if !matches!(
                request.capability,
                crate::optimization::OptimizationCapability::WorkflowRouting
                    | crate::optimization::OptimizationCapability::DelegationRouting
            ) {
                return Err(ModelCallFailure::InvalidResponse);
            }

            let criteria = request
                .allowed_outcomes
                .iter()
                .map(|outcome| (outcome.clone(), Value::String(outcome_description(outcome))))
                .collect::<serde_json::Map<_, _>>();
            let instructions = match request.capability {
                crate::optimization::OptimizationCapability::WorkflowRouting => {
                    "Choose the workflow category supported by this compact operational state. If it contains no task-type evidence, choose insufficient_context or unknown. Never infer implementation correctness or QA success."
                }
                _ => {
                    "Choose the execution strategy supported by this compact operational state. Prefer an escape outcome when the state is insufficient. This is advisory only and must not perform or authorize an action."
                }
            };
            let body = json!({
                "model": self.model,
                "state": request.state,
                "questions": {
                    "cathulu_decision": {
                        "type": "choice",
                        "instructions": instructions,
                        "criteria": criteria
                    }
                }
            });
            let body_bytes =
                serde_json::to_vec(&body).map_err(|_| ModelCallFailure::InvalidResponse)?;
            let input_bytes = body_bytes.len() as u64;
            let response = self
                .client
                .post(&self.endpoint)
                .bearer_auth(&self.api_key)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body_bytes)
                .send()
                .await
                .map_err(map_http_error)?;
            let response_bytes = read_limited_response(response).await?;
            let wire_response: JevResponse = serde_json::from_slice(&response_bytes)
                .map_err(|_| ModelCallFailure::InvalidResponse)?;
            let answer = wire_response
                .answers
                .get("cathulu_decision")
                .ok_or(ModelCallFailure::InvalidResponse)?;
            if answer.answer_type != "choice" {
                return Err(ModelCallFailure::InvalidResponse);
            }
            let result = DecisionResult {
                schema_version: request.schema_version,
                outcome: answer.choice.clone(),
                model_confidence: Some(answer.confidence),
                probabilities: answer.probabilities.clone(),
            };
            Ok(Some(ModelResponse {
                value: result,
                provider_model: Some(wire_response.model),
                input_bytes: Some(input_bytes),
                output_bytes: Some(response_bytes.len() as u64),
                input_tokens: wire_response.usage.as_ref().map(|usage| usage.input_tokens),
                output_tokens: wire_response
                    .usage
                    .as_ref()
                    .map(|usage| usage.output_tokens),
            }))
        })
    }
}

pub struct ScaleDownContextProvider {
    client: Client,
    api_key: String,
    model: String,
    endpoint: String,
}

impl ScaleDownContextProvider {
    pub fn from_environment(model: Option<&str>) -> Option<Self> {
        let api_key = provider_credentials::context_api_key()?;
        Self::new(
            api_key,
            model.unwrap_or(SCALEDOWN_DEFAULT_MODEL),
            SCALEDOWN_ENDPOINT,
        )
        .ok()
    }

    fn new(api_key: String, model: &str, endpoint: &str) -> Result<Self, ModelCallFailure> {
        let api_key = clean_credential(api_key)?;
        let model = clean_model(model)?;
        let client = http_client()?;
        Ok(Self {
            client,
            api_key,
            model,
            endpoint: endpoint.to_string(),
        })
    }
}

impl ContextModel for ScaleDownContextProvider {
    fn process<'a>(&'a self, request: &'a ContextRequest) -> ContextModelFuture<'a> {
        Box::pin(async move {
            request
                .validate()
                .map_err(|_| ModelCallFailure::InvalidResponse)?;
            if request.capability != crate::optimization::ContextCapability::Extract
                || request.schema_version < 2
            {
                return Err(ModelCallFailure::InvalidResponse);
            }
            let (selected_text, source_map) = combine_selected_sources(request)?;
            if selected_text.is_empty() || selected_text.len() > MAX_SCALEDOWN_INPUT_BYTES {
                return Err(ModelCallFailure::InvalidResponse);
            }
            let body = scaledown_extract_request(&self.model, &selected_text);
            let body_bytes =
                serde_json::to_vec(&body).map_err(|_| ModelCallFailure::InvalidResponse)?;
            let input_bytes = body_bytes.len() as u64;
            let response = self
                .client
                .post(&self.endpoint)
                .header("x-api-key", &self.api_key)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body_bytes)
                .send()
                .await
                .map_err(map_http_error)?;
            let response_bytes = read_limited_response(response).await?;
            let wire_response: ScaleDownResponse = serde_json::from_slice(&response_bytes)
                .map_err(|_| ModelCallFailure::InvalidResponse)?;
            let content = wire_response
                .choices
                .first()
                .and_then(|choice| choice.message.content.as_deref())
                .ok_or(ModelCallFailure::InvalidResponse)?;
            let extracted: ScaleDownExtraction =
                serde_json::from_str(content).map_err(|_| ModelCallFailure::InvalidResponse)?;
            let candidates = map_extractions(request, &selected_text, &source_map, extracted)?;
            let spans = candidates
                .iter()
                .map(|candidate| candidate.source_span.clone())
                .collect::<Vec<_>>();
            let content = json!({ "candidates": candidates });
            let result = ContextResult {
                schema_version: request.schema_version,
                content,
                source_artifact_ids: request
                    .sources
                    .iter()
                    .map(|source| source.artifact_id)
                    .collect(),
                source_content_hashes: request
                    .sources
                    .iter()
                    .map(|source| source.content_hash.clone())
                    .collect(),
                source_spans: spans,
                model_confidence: None,
            };
            Ok(Some(ModelResponse {
                value: result,
                provider_model: Some(wire_response.model),
                input_bytes: Some(input_bytes),
                output_bytes: Some(response_bytes.len() as u64),
                input_tokens: wire_response
                    .usage
                    .as_ref()
                    .map(|usage| usage.prompt_tokens),
                output_tokens: wire_response
                    .usage
                    .as_ref()
                    .map(|usage| usage.completion_tokens),
            }))
        })
    }
}

fn http_client() -> Result<Client, ModelCallFailure> {
    Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .redirect(Policy::none())
        .build()
        .map_err(|_| ModelCallFailure::ProviderUnavailable)
}

fn clean_credential(value: String) -> Result<String, ModelCallFailure> {
    if value.is_empty()
        || value.len() > 8 * 1024
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(ModelCallFailure::ProviderUnavailable);
    }
    Ok(value)
}

fn clean_model(value: &str) -> Result<String, ModelCallFailure> {
    if value.trim().is_empty() || value.len() > 120 || value.chars().any(char::is_control) {
        return Err(ModelCallFailure::InvalidResponse);
    }
    Ok(value.to_string())
}

fn map_http_error(error: reqwest::Error) -> ModelCallFailure {
    if error.is_timeout() {
        ModelCallFailure::Timeout
    } else {
        ModelCallFailure::ProviderUnavailable
    }
}

async fn read_limited_response(mut response: Response) -> Result<Vec<u8>, ModelCallFailure> {
    if !response.status().is_success() {
        return Err(ModelCallFailure::ProviderUnavailable);
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(ModelCallFailure::InvalidResponse);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(map_http_error)? {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(ModelCallFailure::InvalidResponse);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn outcome_description(outcome: &str) -> String {
    match outcome {
        "bug_fix" => "The task is primarily correcting an existing defect.".to_string(),
        "feature" => "The task is primarily adding or changing product behavior.".to_string(),
        "dependency_fix" => "The task is primarily resolving a dependency issue.".to_string(),
        "architecture" => "The task is primarily an architecture or design change.".to_string(),
        "research" => "The task is primarily information gathering or analysis.".to_string(),
        "verification" => "The task is primarily checking or validating existing behavior.".to_string(),
        "main_only" => "Keep the task with the current Main Overseer; no separate scope or worker is justified.".to_string(),
        "sub_overseer" => "Use a dedicated Sub-Overseer for an independent project scope.".to_string(),
        "worker" => "Delegate a bounded implementation task to a worker.".to_string(),
        "sub_plus_worker" => "Use a dedicated Sub-Overseer and a bounded worker task.".to_string(),
        "unknown" | "insufficient_context" | "needs_codex" | "needs_more_evidence" | "not_applicable" => {
            "Use this escape outcome when the structured state does not support a safe substantive choice.".to_string()
        }
        _ => "A predefined outcome; select it only when the structured state supports it.".to_string(),
    }
}

fn scaledown_extract_request(model: &str, selected_text: &str) -> Value {
    json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "Extract only exact text spans from the supplied selected test, compiler, CI, or tool output that show a failed test, error, exception, panic, assertion, or related source location. Do not rewrite, infer, or summarize a span. Omit anything that cannot be tied to a literal source range."
            },
            { "role": "user", "content": selected_text }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "cathulu_log_evidence",
                "schema": {
                    "type": "object",
                    "properties": {
                        "entities": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "text": { "type": "string", "description": "The exact quoted text from the selected output." },
                                    "type": { "type": "string", "enum": ["test_failure", "error", "panic", "location"], "description": "The kind of failure evidence represented by this exact span." },
                                    "confidence": { "type": "number", "minimum": 0, "maximum": 1, "description": "Model confidence for selecting this span." },
                                    "start": { "type": "integer", "minimum": 0, "description": "Zero-based Unicode character offset of the first character in text within the selected input." },
                                    "end": { "type": "integer", "minimum": 0, "description": "Exclusive Unicode character offset immediately after text within the selected input." }
                                },
                                "required": ["text", "type", "confidence", "start", "end"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["entities"],
                    "additionalProperties": false
                }
            }
        },
        "top_n": 16,
        "threshold": 0.0,
        "context_chars": 0
    })
}

#[derive(Clone, Debug)]
struct SourceMapEntry {
    combined_start: usize,
    combined_end: usize,
    excerpt_start: usize,
    artifact_id: i64,
    artifact_start: u64,
}

fn combine_selected_sources(
    request: &ContextRequest,
) -> Result<(String, Vec<SourceMapEntry>), ModelCallFailure> {
    let mut combined = String::new();
    let mut mappings = Vec::new();
    for (source_index, source) in request.sources.iter().enumerate() {
        if source_index > 0 {
            combined.push_str("\n\n");
        }
        let source_start = combined.len();
        combined.push_str(&sanitize_remote_text(&source.content));
        let mut excerpt_start = 0usize;
        for span in &source.source_spans {
            let span_length = usize::try_from(span.end_byte - span.start_byte)
                .map_err(|_| ModelCallFailure::InvalidResponse)?;
            let combined_start = source_start.saturating_add(excerpt_start);
            let combined_end = combined_start.saturating_add(span_length);
            if combined_end > combined.len() {
                return Err(ModelCallFailure::InvalidResponse);
            }
            mappings.push(SourceMapEntry {
                combined_start,
                combined_end,
                excerpt_start,
                artifact_id: source.artifact_id,
                artifact_start: span.start_byte,
            });
            excerpt_start = excerpt_start.saturating_add(span_length);
        }
        if excerpt_start != source.content.len() {
            return Err(ModelCallFailure::InvalidResponse);
        }
    }
    Ok((combined, mappings))
}

fn map_extractions(
    request: &ContextRequest,
    combined: &str,
    mappings: &[SourceMapEntry],
    extracted: ScaleDownExtraction,
) -> Result<Vec<ContextExtractionCandidate>, ModelCallFailure> {
    let mut candidates = Vec::new();
    let mut seen_spans = BTreeSet::new();
    for entity in extracted.entities {
        if !entity.confidence.is_finite() || !(0.0..=1.0).contains(&entity.confidence) {
            return Err(ModelCallFailure::InvalidResponse);
        }
        if entity.text.is_empty() {
            return Err(ModelCallFailure::InvalidResponse);
        }
        let (combined_start, combined_end) =
            char_range_to_byte_range(combined, entity.start, entity.end)
                .ok_or(ModelCallFailure::InvalidResponse)?;
        if combined.get(combined_start..combined_end) != Some(entity.text.as_str()) {
            return Err(ModelCallFailure::InvalidResponse);
        }
        let mapping = mappings
            .iter()
            .find(|mapping| {
                mapping.combined_start <= combined_start && combined_end <= mapping.combined_end
            })
            .ok_or(ModelCallFailure::InvalidResponse)?;
        let source = request
            .sources
            .iter()
            .find(|source| source.artifact_id == mapping.artifact_id)
            .ok_or(ModelCallFailure::InvalidResponse)?;
        let source_quote_start = mapping
            .excerpt_start
            .saturating_add(combined_start - mapping.combined_start);
        let source_quote_end = source_quote_start.saturating_add(combined_end - combined_start);
        let source_quote = source
            .content
            .get(source_quote_start..source_quote_end)
            .ok_or(ModelCallFailure::InvalidResponse)?;
        // A model may return masked content, but that evidence must not be
        // persisted as a quote. Skip any candidate overlapping redacted bytes.
        if source_quote != entity.text {
            continue;
        }
        let artifact_start = mapping
            .artifact_start
            .saturating_add((combined_start - mapping.combined_start) as u64);
        let artifact_end = artifact_start.saturating_add((combined_end - combined_start) as u64);
        if !seen_spans.insert((mapping.artifact_id, artifact_start, artifact_end)) {
            continue;
        }
        let kind = match entity.entity_type.as_str() {
            "test_failure" => ContextExtractionCandidateKind::TestFailure,
            "error" => ContextExtractionCandidateKind::Error,
            "panic" => ContextExtractionCandidateKind::Panic,
            "location" => ContextExtractionCandidateKind::Location,
            _ => return Err(ModelCallFailure::InvalidResponse),
        };
        let quote = entity.text;
        if quote.len() > 4096 || candidates.len() >= MAX_SCALEDOWN_CANDIDATES {
            return Err(ModelCallFailure::InvalidResponse);
        }
        let summary = quote.split_whitespace().collect::<Vec<_>>().join(" ");
        if summary.is_empty() || summary.len() > 512 {
            return Err(ModelCallFailure::InvalidResponse);
        }
        candidates.push(ContextExtractionCandidate {
            kind,
            summary,
            quote,
            source_span: ContextSourceSpan {
                artifact_id: mapping.artifact_id,
                start_byte: artifact_start,
                end_byte: artifact_end,
            },
        });
    }
    let serialized_candidates = serde_json::to_vec(&json!({ "candidates": candidates }))
        .map_err(|_| ModelCallFailure::InvalidResponse)?;
    if serialized_candidates.len() > request.max_output_bytes {
        return Err(ModelCallFailure::InvalidResponse);
    }
    Ok(candidates)
}

fn sanitize_remote_text(text: &str) -> String {
    let mut redacted = vec![false; text.len()];
    let lowercase = text.to_ascii_lowercase();
    let bytes = text.as_bytes();
    let lower_bytes = lowercase.as_bytes();

    // Redact values whose left-hand label identifies credential material,
    // including compound environment names such as AWS_SECRET_ACCESS_KEY.
    let mut line_start = 0usize;
    while line_start < bytes.len() {
        let line_end = bytes[line_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| line_start + offset)
            .unwrap_or(bytes.len());
        let line = &bytes[line_start..line_end];
        if let Some(separator) = line.iter().position(|byte| matches!(byte, b'=' | b':')) {
            let key = String::from_utf8_lossy(&line[..separator]).to_ascii_lowercase();
            let has_sensitive_label = key
                .split(|character: char| !character.is_ascii_alphanumeric())
                .any(|label| {
                    matches!(
                        label,
                        "key"
                            | "token"
                            | "secret"
                            | "password"
                            | "passwd"
                            | "passphrase"
                            | "credential"
                            | "credentials"
                            | "authorization"
                            | "auth"
                    )
                });
            if has_sensitive_label {
                let mut value_start = line_start + separator + 1;
                while value_start < line_end && bytes[value_start].is_ascii_whitespace() {
                    value_start += 1;
                }
                let value_end = sensitive_value_end(bytes, value_start);
                mark_sensitive(&mut redacted, value_start, value_end);
            }
        }
        if line_end == bytes.len() {
            break;
        }
        line_start = line_end + 1;
    }
    // Also handle JSON-like lines with multiple credential properties.
    for label in [
        "api_key",
        "apikey",
        "key",
        "token",
        "secret",
        "password",
        "passwd",
        "passphrase",
        "credential",
        "credentials",
        "authorization",
    ] {
        let label_bytes = label.as_bytes();
        let mut search_from = 0usize;
        while let Some(relative) = lower_bytes[search_from..]
            .windows(label_bytes.len())
            .position(|window| window == label_bytes)
        {
            let label_start = search_from + relative;
            let label_end = label_start + label_bytes.len();
            let left_boundary =
                label_start == 0 || !lower_bytes[label_start - 1].is_ascii_alphanumeric();
            let right_boundary =
                label_end == lower_bytes.len() || !lower_bytes[label_end].is_ascii_alphanumeric();
            if left_boundary && right_boundary {
                let mut delimiter = label_end;
                while delimiter < lower_bytes.len()
                    && matches!(lower_bytes[delimiter], b' ' | b'\t' | b'"' | b'\'')
                {
                    delimiter += 1;
                }
                if delimiter < lower_bytes.len() && matches!(lower_bytes[delimiter], b'=' | b':') {
                    let mut value_start = delimiter + 1;
                    while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
                        value_start += 1;
                    }
                    let value_end = sensitive_value_end(bytes, value_start);
                    mark_sensitive(&mut redacted, value_start, value_end);
                }
            }
            search_from = label_end;
        }
    }
    for pattern in [
        "bearer ",
        "sk-",
        "sk_",
        "ghp_",
        "github_pat_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "npm_",
        "pypi-",
        "xoxa-",
        "xoxr-",
        "basic ",
        "eyj",
        "glpat-",
        "xoxb-",
        "xoxp-",
        "akia",
        "AIza",
    ] {
        let pattern_lower = pattern.to_ascii_lowercase();
        let pattern_bytes = pattern_lower.as_bytes();
        let mut search_from = 0usize;
        while let Some(relative) = lower_bytes[search_from..]
            .windows(pattern_bytes.len())
            .position(|window| window == pattern_bytes)
        {
            let start = search_from + relative;
            if matches!(pattern, "bearer " | "basic ") {
                let token_start = start + pattern.len();
                let token_end = sensitive_value_end(bytes, token_start);
                mark_sensitive(&mut redacted, token_start, token_end);
            } else {
                let end = sensitive_value_end(bytes, start);
                mark_sensitive(&mut redacted, start, end);
            }
            search_from = start + pattern_bytes.len();
        }
    }

    let begin_marker = b"-----begin";
    let end_marker = b"-----end";
    let mut search_from = 0usize;
    while let Some(relative) = lower_bytes[search_from..]
        .windows(begin_marker.len())
        .position(|window| window == begin_marker)
    {
        let start = search_from + relative;
        let header_end = (start + 128).min(lower_bytes.len());
        if lower_bytes[start..header_end]
            .windows(b"private key".len())
            .any(|window| window == b"private key")
        {
            let end = lower_bytes[start..]
                .windows(end_marker.len())
                .position(|window| window == end_marker)
                .map(|relative_end| start + relative_end + end_marker.len())
                .unwrap_or(bytes.len());
            mark_sensitive(&mut redacted, start, end);
        }
        search_from = start + begin_marker.len();
    }

    let mut output = bytes.to_vec();
    for (index, should_redact) in redacted.into_iter().enumerate() {
        if should_redact && !matches!(output[index], b'\n' | b'\r') {
            // Replacing each byte with one ASCII byte keeps all provider
            // offsets mappable to the original UTF-8 artifact.
            output[index] = b'x';
        }
    }
    String::from_utf8(output).unwrap_or_else(|_| "[redacted context]".to_string())
}

fn sensitive_value_end(bytes: &[u8], mut start: usize) -> usize {
    if start >= bytes.len() {
        return start;
    }
    if matches!(bytes[start], b'\'' | b'"') {
        let quote = bytes[start];
        start += 1;
        let mut end = start;
        while end < bytes.len() && bytes[end] != quote && !matches!(bytes[end], b'\n' | b'\r') {
            end += 1;
        }
        return if end < bytes.len() && bytes[end] == quote {
            end + 1
        } else {
            end
        };
    }
    let mut end = start;
    while end < bytes.len()
        && !bytes[end].is_ascii_whitespace()
        && !matches!(bytes[end], b',' | b';' | b'&' | b']' | b'}' | b')')
    {
        end += 1;
    }
    end
}

fn mark_sensitive(redacted: &mut [bool], start: usize, end: usize) {
    if start < redacted.len() {
        let bounded_end = end.min(redacted.len());
        redacted[start..bounded_end].fill(true);
    }
}

fn char_range_to_byte_range(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    if start >= end {
        return None;
    }
    let mut start_byte = None;
    let mut end_byte = None;
    let mut char_index = 0usize;
    for (byte_index, _) in text.char_indices() {
        if char_index == start {
            start_byte = Some(byte_index);
        }
        if char_index == end {
            end_byte = Some(byte_index);
            break;
        }
        char_index += 1;
    }
    let start_byte = start_byte?;
    let end_byte = end_byte.or_else(|| (char_index == end).then_some(text.len()))?;
    (end_byte > start_byte).then_some((start_byte, end_byte))
}

#[derive(Debug, Deserialize)]
struct JevResponse {
    model: String,
    answers: std::collections::BTreeMap<String, JevAnswer>,
    #[serde(default)]
    usage: Option<JevUsage>,
}

#[derive(Debug, Deserialize)]
struct JevAnswer {
    #[serde(rename = "type")]
    answer_type: String,
    choice: String,
    confidence: f64,
    probabilities: std::collections::BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize)]
struct JevUsage {
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct ScaleDownResponse {
    model: String,
    choices: Vec<ScaleDownChoice>,
    #[serde(default)]
    usage: Option<ScaleDownUsage>,
}

#[derive(Debug, Deserialize)]
struct ScaleDownChoice {
    message: ScaleDownMessage,
}

#[derive(Debug, Deserialize)]
struct ScaleDownMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScaleDownUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct ScaleDownExtraction {
    entities: Vec<ScaleDownEntity>,
}

#[derive(Debug, Deserialize)]
struct ScaleDownEntity {
    text: String,
    #[serde(rename = "type")]
    entity_type: String,
    confidence: f64,
    start: usize,
    end: usize,
}

#[cfg(test)]
mod tests {
    use super::{
        char_range_to_byte_range, combine_selected_sources, map_extractions,
        scaledown_extract_request, ScaleDownEntity, ScaleDownExtraction,
    };
    use crate::optimization::{
        ContextCapability, ContextRequest, ContextSource, ContextSourceSpan,
    };
    use serde_json::json;

    #[test]
    fn scaledown_request_schema_matches_the_response_shape_required_by_parser() {
        let body = scaledown_extract_request("extract-1", "error: failed test");
        let schema = &body["response_format"]["json_schema"]["schema"];
        let entities_schema = &schema["properties"]["entities"];
        let entity_schema = &schema["properties"]["entities"]["items"];

        assert_eq!(schema["required"], json!(["entities"]));
        assert_eq!(entities_schema["type"], json!("array"));
        assert_eq!(
            entity_schema["required"],
            json!(["text", "type", "confidence", "start", "end"])
        );
        let properties = entity_schema["properties"]
            .as_object()
            .expect("entity schema properties");
        for field in ["text", "type", "confidence", "start", "end"] {
            assert!(properties.contains_key(field), "schema must define {field}");
        }
        assert_eq!(
            entity_schema["properties"]["type"]["enum"],
            json!(["test_failure", "error", "panic", "location"])
        );
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(entity_schema["additionalProperties"], json!(false));
        assert!(serde_json::from_value::<ScaleDownExtraction>(json!({})).is_err());
        assert!(serde_json::from_value::<ScaleDownExtraction>(json!({ "entities": [] })).is_ok());
    }

    #[test]
    fn maps_multibyte_scale_down_offsets_back_to_exact_artifact_spans() {
        let request = ContextRequest {
            capability: ContextCapability::Extract,
            schema_version: 2,
            task: "extract_test_compiler_ci_tool_failures".to_string(),
            sources: vec![ContextSource {
                artifact_id: 9,
                content_hash: "a".repeat(64),
                source_ref: "cathulu://artifacts/9".to_string(),
                content: "routine output\nerror: café failed\n".to_string(),
                source_spans: vec![ContextSourceSpan {
                    artifact_id: 9,
                    start_byte: 30,
                    end_byte: 65,
                }],
            }],
            max_output_bytes: 8192,
        };
        let (combined, mappings) =
            combine_selected_sources(&request).expect("combine selected text");
        assert_eq!(combined, request.sources[0].content);
        let offset = combined.find("café").unwrap();
        let char_start = combined[..offset].chars().count();
        let text = "café";
        let candidates = map_extractions(
            &request,
            &combined,
            &mappings,
            ScaleDownExtraction {
                entities: vec![ScaleDownEntity {
                    text: text.to_string(),
                    entity_type: "error".to_string(),
                    confidence: 0.97,
                    start: char_start,
                    end: char_start + text.chars().count(),
                }],
            },
        )
        .expect("map exact extracted span");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].quote, text);
        assert_eq!(candidates[0].source_span.start_byte, 30 + offset as u64);
        assert_eq!(
            candidates[0].source_span.end_byte,
            30 + offset as u64 + text.len() as u64
        );
    }

    #[test]
    fn source_spans_and_entity_offsets_are_checked_before_mapping() {
        assert_eq!(char_range_to_byte_range("café", 0, 4), Some((0, 5)));
        assert_eq!(char_range_to_byte_range("café", 0, 5), None);
        assert!(map_extractions(
            &ContextRequest {
                capability: ContextCapability::Extract,
                schema_version: 2,
                task: "extract_test_compiler_ci_tool_failures".to_string(),
                sources: vec![ContextSource {
                    artifact_id: 1,
                    content_hash: "b".repeat(64),
                    source_ref: "cathulu://artifacts/1".to_string(),
                    content: "error: test".to_string(),
                    source_spans: vec![ContextSourceSpan {
                        artifact_id: 1,
                        start_byte: 0,
                        end_byte: 11
                    }],
                }],
                max_output_bytes: 8192,
            },
            "error: test",
            &[super::SourceMapEntry {
                combined_start: 0,
                combined_end: 11,
                excerpt_start: 0,
                artifact_id: 1,
                artifact_start: 0
            }],
            ScaleDownExtraction {
                entities: vec![ScaleDownEntity {
                    text: "invented".to_string(),
                    entity_type: "error".to_string(),
                    confidence: 0.9,
                    start: 0,
                    end: 5
                }]
            }
        )
        .is_err());
    }

    #[test]
    fn extraction_request_contains_only_selected_text_and_fixed_schema() {
        let body = super::scaledown_extract_request("extract-1", "error: failed test");
        let encoded = serde_json::to_string(&body).unwrap();
        assert!(encoded.contains("error: failed test"));
        assert!(!encoded.contains("source_ref"));
        assert!(!encoded.contains("artifact_id"));
        assert_eq!(body["context_chars"], json!(0));
        assert_eq!(body["model"], json!("extract-1"));
    }

    #[test]
    fn remote_sanitization_masks_common_credentials_without_changing_offsets() {
        let source =
            "Authorization: Bearer bearer-secret\napi_key=sk-secret-token\nprivate_key=abc123\nAWS_SECRET_ACCESS_KEY=aws-super-secret\nGITHUB_TOKEN=github-token-123\nAuthorization: Basic base64-secret\n{\"token\":\"json-secret\",\"api_key\":\"json-key\"}\n";
        let sanitized = super::sanitize_remote_text(source);
        assert!(!sanitized.contains("bearer-secret"));
        assert!(!sanitized.contains("sk-secret-token"));
        assert!(!sanitized.contains("abc123"));
        assert!(!sanitized.contains("aws-super-secret"));
        assert!(!sanitized.contains("github-token-123"));
        assert!(!sanitized.contains("base64-secret"));
        assert!(!sanitized.contains("json-secret"));
        assert!(!sanitized.contains("json-key"));
        assert_eq!(sanitized.len(), source.len());
        assert!(sanitized.contains("Authorization:"));
    }
}

#[cfg(test)]
mod provider_http_tests {
    use super::{JevProvider, ScaleDownContextProvider};
    use crate::optimization::{
        ContextCapability, ContextModel, ContextRequest, ContextSource, ContextSourceSpan,
        DecisionModel, DecisionRequest, OptimizationCapability,
    };
    use serde_json::{json, Value};
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread::{self, JoinHandle},
        time::Duration,
    };

    fn mock_http_server(response_body: String) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local mock server");
        let address = listener.local_addr().expect("mock server address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept provider request");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("set mock read timeout");
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .expect("set mock write timeout");
            let request = read_request(&mut stream);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write mock response");
            request
        });
        (format!("http://{address}/mock"), server)
    }

    fn read_request(stream: &mut TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 2048];
        loop {
            let bytes_read = stream.read(&mut buffer).expect("read request bytes");
            if bytes_read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..bytes_read]);
            if request_is_complete(&request) {
                break;
            }
        }
        String::from_utf8(request).expect("HTTP request is UTF-8")
    }

    fn request_is_complete(request: &[u8]) -> bool {
        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            return false;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        });
        if let Some(content_length) = content_length {
            request.len() >= header_end + 4 + content_length
        } else {
            headers.lines().any(|line| {
                line.to_ascii_lowercase()
                    .starts_with("transfer-encoding: chunked")
            }) && request.ends_with(b"0\r\n\r\n")
        }
    }

    fn request_body(request: &str) -> Value {
        let (_, body) = request.split_once("\r\n\r\n").expect("HTTP body separator");
        serde_json::from_str(body).expect("provider request JSON")
    }

    #[tokio::test]
    async fn jev_adapter_uses_bearer_auth_and_only_the_bounded_decision_state() {
        let response_body = json!({
            "model": "jev-fixture-v1",
            "answers": {
                "cathulu_decision": {
                    "type": "choice",
                    "choice": "bug_fix",
                    "confidence": 0.82,
                    "probabilities": { "bug_fix": 0.82, "insufficient_context": 0.18 }
                }
            },
            "usage": { "input_tokens": 31, "output_tokens": 8 }
        })
        .to_string();
        let (endpoint, server) = mock_http_server(response_body);
        let provider = JevProvider::new("jev-test-key".to_string(), "jev-fixture", &endpoint)
            .expect("construct Jev test provider");
        let request = DecisionRequest {
            capability: OptimizationCapability::WorkflowRouting,
            schema_version: 1,
            state: json!({ "runStatus": "running", "changedPathCount": 2 }),
            allowed_outcomes: vec!["bug_fix".to_string(), "insufficient_context".to_string()],
        };

        let response = provider
            .decide(&request)
            .await
            .expect("Jev request succeeds")
            .expect("Jev returns a prediction");
        let request_text = server.join().expect("mock server finishes");
        let headers = request_text
            .split_once("\r\n\r\n")
            .expect("HTTP headers")
            .0
            .to_ascii_lowercase();
        let body = request_body(&request_text);

        assert!(headers.contains("authorization: bearer jev-test-key"));
        assert_eq!(body["model"], json!("jev-fixture"));
        assert_eq!(body["state"]["changedPathCount"], json!(2));
        assert_eq!(
            body["questions"]["cathulu_decision"]["type"],
            json!("choice")
        );
        assert!(!request_text.contains("TASK_TEXT"));
        assert_eq!(response.value.outcome, "bug_fix");
        assert_eq!(response.provider_model.as_deref(), Some("jev-fixture-v1"));
        assert_eq!(response.input_tokens, Some(31));
        assert_eq!(response.output_tokens, Some(8));
    }

    #[tokio::test]
    async fn scaledown_adapter_sends_only_sanitized_selected_spans_and_maps_exact_evidence() {
        let quote = "error: café failed";
        let extraction = json!({
            "entities": [{
                "text": quote,
                "type": "error",
                "confidence": 0.96,
                "start": 0,
                "end": quote.chars().count()
            }]
        })
        .to_string();
        let response_body = json!({
            "model": "extract-fixture-v1",
            "choices": [{ "message": { "content": extraction } }],
            "usage": { "prompt_tokens": 50, "completion_tokens": 12 }
        })
        .to_string();
        let (endpoint, server) = mock_http_server(response_body);
        let provider = ScaleDownContextProvider::new(
            "scaledown-test-key".to_string(),
            "extract-fixture",
            &endpoint,
        )
        .expect("construct ScaleDown test provider");
        let selected_text = "error: café failed\nAPI_KEY=sk-sensitive-test-secret\n";
        let request = ContextRequest {
            capability: ContextCapability::Extract,
            schema_version: 2,
            task: "PRIVATE_TASK_TEXT".to_string(),
            sources: vec![ContextSource {
                artifact_id: 42,
                content_hash: "a".repeat(64),
                source_ref: "C:/private/path/private.log".to_string(),
                content: selected_text.to_string(),
                source_spans: vec![ContextSourceSpan {
                    artifact_id: 42,
                    start_byte: 100,
                    end_byte: 100 + selected_text.len() as u64,
                }],
            }],
            max_output_bytes: 8192,
        };

        let response = provider
            .process(&request)
            .await
            .expect("ScaleDown request succeeds")
            .expect("ScaleDown returns extraction candidates");
        let request_text = server.join().expect("mock server finishes");
        let headers = request_text
            .split_once("\r\n\r\n")
            .expect("HTTP headers")
            .0
            .to_ascii_lowercase();
        let body = request_body(&request_text);
        let sent_text = body["messages"][1]["content"]
            .as_str()
            .expect("selected context text");

        assert!(headers.contains("x-api-key: scaledown-test-key"));
        assert_eq!(body["model"], json!("extract-fixture"));
        assert!(sent_text.contains("error: café failed"));
        assert!(!request_text.contains("sensitive-test-secret"));
        assert!(!request_text.contains("PRIVATE_TASK_TEXT"));
        assert!(!request_text.contains("private/path"));
        assert!(!request_text.contains("artifact_id"));
        assert_eq!(response.value.source_artifact_ids, vec![42]);
        assert_eq!(
            response.value.content["candidates"][0]["quote"],
            json!(quote)
        );
        assert_eq!(response.value.source_spans[0].start_byte, 100);
        assert_eq!(
            response.value.source_spans[0].end_byte,
            100 + quote.len() as u64
        );
        assert_eq!(
            response.provider_model.as_deref(),
            Some("extract-fixture-v1")
        );
        assert_eq!(response.input_tokens, Some(50));
        assert_eq!(response.output_tokens, Some(12));
    }
}
