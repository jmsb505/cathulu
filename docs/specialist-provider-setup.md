# Optional specialist-provider setup

Jev and ScaleDown are optional remote optimizers. Codex remains responsible for reasoning and coding; deterministic Cathulu state and QA remain authoritative. Both connections are disabled unless a run explicitly configures them for shadow mode. Shadow predictions never change execution.

## Local API keys

Copy the repository-root .env.example to .env.local and fill only the provider keys you plan to use:

    CATHULU_DECISION_PROVIDER_API_KEY=your_jev_key
    CATHULU_CONTEXT_PROVIDER_API_KEY=your_scaledown_key

Cathulu reads process environment variables first, then a .env.local in its current directory or a parent directory. Set CATHULU_ENV_FILE to an absolute path when the app is launched outside the repository tree. The loader reads only the two named provider keys, does not execute shell syntax, and does not log or persist credentials. The .env.local file is ignored by Git.

## Explicit shadow configuration

Pass optimization_config to start_run to create an immutable treatment snapshot. A Jev example is:

    {
      "project_id": 1,
      "optimization_config": {
        "capabilities": {
          "workflow_routing": {
            "enabled": true,
            "mode": "shadow",
            "provider": "jev",
            "model": "jev-latest",
            "providerLocality": "remote",
            "remoteDataAllowed": true
          },
          "delegation_routing": {
            "enabled": true,
            "mode": "shadow",
            "provider": "jev",
            "model": "jev-latest",
            "providerLocality": "remote",
            "remoteDataAllowed": true
          }
        }
      }
    }

Then call run_decision_shadow_trial with the run ID, a unique invocation ID, and either workflow_routing or delegation_routing. Cathulu derives the request from the minimized SQLite state; the tool does not accept caller-provided state or task text. Current workflow state has no task-category field, so Jev may correctly return an escape outcome such as insufficient_context until that structured signal is available.

For ScaleDown extraction, enable context_extract in the same run treatment:

    "context_extract": {
      "enabled": true,
      "mode": "shadow",
      "provider": "scaledown",
      "model": "extract-1",
      "providerLocality": "remote",
      "remoteDataAllowed": true
    }

After registering a local test, compiler, CI, or tool-output artifact for that run, call run_context_extraction_shadow_trial with the run ID, invocation ID, and artifact IDs. Cathulu verifies the registered hash and sends only bounded error-adjacent excerpts. Credential-labeled values and known token formats are best-effort masked before transmission; any returned quote overlapping masked bytes is discarded. This is not a guarantee that every secret is detected, so review the selected evidence before explicitly allowing its remote processing. Exact returned spans must still match the unchanged local artifact before the result is recorded.

## Egress and fallback

Remote calls require all of the following: an enabled shadow capability in the immutable run treatment, providerLocality set to remote, remoteDataAllowed set to true, and a locally available API key. Missing credentials record a skipped shadow invocation. HTTP errors, timeouts, and malformed responses are recorded as provider failures; they do not block the run or bypass QA. Active mode remains rejected pending the M18 evidence and owner-approval gates.

Jev uses TypeSafe's official POST /v1/systemone API with bearer-key authentication and receives only the minimized routing object and fixed outcome choices. ScaleDown uses its official OpenAI-compatible POST /v1/chat/completions Extract API with x-api-key authentication and receives only selected sanitized excerpts; local paths, artifact IDs, source references, and full logs are omitted from the request. See the [TypeSafe OpenAPI](https://api.typesafe.ai/openapi.json), [ScaleDown Extract API](https://docs.scaledown.ai/api-reference/extract), and [ScaleDown quickstart](https://docs.scaledown.ai/quickstart).

The reviewed ScaleDown API pages specify the request and response formats but do not state the service's request-retention period. Treat provider calls as remote processing: use only data you authorize Cathulu to transmit. Cathulu keeps local run attribution, hashes, token/byte usage when provided, and validated extracted evidence; it does not persist raw provider response bodies.
