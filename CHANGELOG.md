# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-11-03

A ground-up rewrite against the real Hindsight HTTP API (spec 1.1.0). **Breaking: no
backward compatibility with 0.1** — 0.1's endpoints, models and error type were
incorrect against current servers (it posted to retired paths and expected response
shapes the API never produced).

### Added

- Full v1.1.0 API surface: 96 methods across monitoring, retain/recall/reflect,
  memory management, banks (config/aliases/stats/timeseries/consolidation), entities,
  tags, observation scopes, mental models, knowledge base, directives, documents and
  chunks, async operations, file upload/download, bank & document transfer, bank
  templates, webhooks, audit logs and LLM traces.
- `Client` / `ClientBuilder` / `Client::from_env()` (`HINDSIGHT_API_URL`,
  `HINDSIGHT_API_KEY`) with pooled HTTP/2, keepalive, and `hsk_…` bearer auth.
- Bank-scoped `BankClient` via `client.bank("id")` — ids percent-encoded automatically.
- Typed, matchable `Error` (`Auth`, `NotFound`, `RateLimited` with `Retry-After`,
  `Api` with parsed FastAPI validation details, `Timeout`, `StructuredOutput`,
  `RetriesExhausted`) plus `Error::validation_errors()`.
- `RetryPolicy`: full-jitter exponential backoff on 429/5xx/transport errors,
  `Retry-After` honored, reads-only by default, writes opt-in.
- Builder-pattern request types (`MemoryItem`, `RecallRequest`, `ReflectRequest`,
  query structs for every list endpoint) — no positional `Option<&str>` arguments.
- `recall_text()` — memories formatted as an LLM-ready context string.
- `reflect_structured::<T>()` behind the `schemars` feature: schema-derived structured
  output with typed deserialization.
- Async pipelines: `retain_async`, `retain_files` (multipart), `wait_for_operation`
  with configurable polling.
- `Pager` auto-pagination for every `{items,total,limit,offset}` list endpoint.
- `util::map_concurrent` — bounded-concurrency fan-out preserving input order.
- Content blocks for multimodal retain (text + base64 images/files), tag groups with
  fuzzy resolution, temporal windows, per-stage `min_scores`, observation scopes.
- 5 runnable examples and a `DESIGN.md` documenting the 0.1 audit and v2 rationale.

### Changed

- Endpoints moved to the current API paths (`/v1/default/banks/{bank_id}/memories`,
  `/memories/recall`, `/reflect`) — 0.1's `/api/v1/banks/{bank_id}/…` paths 404 on
  current servers.
- `MemoryHit` replaced by the real `RecallResult` schema (id/text/type/entities/
  scores/temporal fields); `reflect` now returns the full `ReflectResponse`
  (text, citations, usage, trace) instead of a raw string.
- Default timeout raised 30 s → 120 s (reflect ≥ 300 s), per-request overrides.

### Removed

- The incorrect 0.1 API (`HindsightClient::retain/recall/reflect` positional API,
  `MemoryHit`, `Error::ApiError`).

[0.2.0]: https://github.com/elcoosp/hindsight-client/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/elcoosp/hindsight-client/releases/tag/v0.1.0
