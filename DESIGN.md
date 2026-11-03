# hindsight-client v2 — Design Brief

This document is the brainstorm + design rationale behind the v2 rewrite. The v2 API
is **intentionally breaking**; there is no backward compatibility with 0.1.

---

## 1. What 0.1 got wrong (audited against the real API, spec version 1.1.0)

The real Hindsight HTTP API (self-hosted `http://localhost:8888`, cloud
`https://api.hindsight.vectorize.io`) is documented by an OpenAPI spec with **79 paths**.
0.1 covered **3 endpoints, incorrectly**:

| # | Problem | Evidence |
|---|---------|----------|
| 1 | **Wrong paths.** 0.1 posts to `/api/v1/banks/{id}/retain|recall|reflect`. The real API is `/v1/default/banks/{id}/memories` (retain), `/v1/default/banks/{id}/memories/recall`, `/v1/default/banks/{id}/reflect`. Every 0.1 call 404s against a current server. | OpenAPI spec, generated clients |
| 2 | **No authentication.** Cloud and secured deployments require `Authorization: Bearer hsk_…`. 0.1 cannot send it. | API reference, all official clients |
| 3 | **Wrong recall response model.** 0.1 expects `{hits: [{content, score}]}`. The real response is `{results: [RecallResult]}` where each result has `id`, `text`, `type`, `entities`, `context`, `occurred_start/end`, `mentioned_at`, `document_id`, `chunk_id`, `tags`, `metadata`, `scores{semantic,keyword,reranker,final}`, `attachments`. There is no `content`/`score` field at all. | `RecallResult` schema |
| 4 | **Wrong retain request.** Real retain takes `{items: [MemoryItem], async, document_tags, operation_id}`; a `MemoryItem` supports multimodal content blocks, `metadata`, `entities`, `tags`, `observation_scopes`, `strategy`, `update_mode`. 0.1 sends a flat `{content, context, timestamp}` body that the server rejects with 422. | `RetainRequest`/`MemoryItem` schemas |
| 5 | **reflect loses everything.** Real response carries `text`, `based_on` (cited memories/facts/directives/mental models), `structured_output`, `usage`, `trace`. 0.1 returns the raw body string. | `ReflectResponse` schema |
| 6 | **~93% of the API missing.** No banks management, mental models, directives, knowledge base, documents, entities, operations, files, webhooks, transfer, audit, LLM traces, monitoring, tags, graph. | OpenAPI path list |
| 7 | **No retries.** A transient 429/503 kills the call. | — |
| 8 | **No pagination** even though every list endpoint is `{items,total,limit,offset}`. | list schemas |
| 9 | **Unencoded path segments.** `format!("/banks/{}/…", bank_id)` breaks on ids with spaces/slashes — both legal in bank ids. | — |
| 10 | **DX friction.** Positional `Option<&str>` arguments (`retain(bank, content, None, None)`) are unreadable and unextensible; a fixed 30s timeout is fatal for `reflect` (agentic loop, up to 10 rounds); no way to reuse config. | — |

## 2. Competitive position

The official `hindsight-client` Rust crate is **progenitor-generated** from the OpenAPI
spec: exhaustive but raw (`Client::retain_memories(&bank_id, None, &types::RetainRequest{
…, async_: false, …})`, `ResponseValue<T>` envelopes, 100+ flat `types::`, no retries, no
pagination helpers, no bank-scoped handles, no structured-output support, no
User-Agent-aware builder). The Python/TS wrappers are hand-written but carry the same
"config dict" style.

v2's bet: a **hand-designed, opinionated client** beats a generated one on every axis a
Rust engineer touches daily: error type, retries, pagination, timeouts, cost
(`schemars`-powered structured reflect), bank-scoped ergonomics, and request builders
with `Default`.

## 3. v2 design decisions

### 3.1 Core shape

- `Client` (cheap `Clone`, `Arc` inside) built via `Client::builder()` or `Client::from_env()`
  (`HINDSIGHT_API_URL`, `HINDSIGHT_API_KEY`).
- `client.bank("id")` → `BankClient<'_>`: every bank-scoped operation without re-threading
  `bank_id` through 70 methods. This mirrors how agents actually work (one bank per
  user/session) and is the single biggest ergonomic win over both 0.1 and the official crate.
- Root-level ops (health/version/metrics, list banks, chunk fetch, template schema) live on `Client`.
- All paths built through `Url::path_segments_mut().push(...)` — **automatic percent-encoding**,
  no `format!` into URLs.
- Path prefix `/v1/default` is a `ClientBuilder::path_prefix` option (forward-compat when
  multi-tenant prefixes land).

### 3.2 Error type (typed, matchable)

```
Error::Transport(reqwest) | Timeout | Url
Error::Auth        401/403 with server message
Error::NotFound    404 (bank/memory/… does not exist)
Error::RateLimited 429 after retries exhausted (carries Retry-After)
Error::Api         any other 4xx/5xx: status + message + parsed `detail` (FastAPI validation shape)
Error::Json        response decode failure
Error::InvalidInput client-side validation (empty bank id, empty files, …)
Error::RetriesExhausted attempts + last error
Error::StructuredOutput reflect structured_output was null/error (message + raw value)
```

### 3.3 Retries & resilience

`RetryPolicy { max_retries, base_delay, max_delay, jitter }` (default: 2 retries, full-jitter
exponential backoff). GET/HEAD retry on `429/500/502/503/504` and transport errors; write
methods retry only when `ClientBuilder::retry_writes(true)` (default off — retain is not
idempotent). `Retry-After` on 429/503 is honored verbatim. Backoff sleeps are tokio-cancellable.

### 3.4 Requests as builders, not positional args

Every operation takes a params struct with `Default`/`new()` and chainable setters
(`RecallParams::new("q").budget(Budget::High).types([FactType::World]).tags(["u1"])`).
`impl Into<RecallParams>` lets `bank.recall("who is alice?")` stay one-liner simple while
every knob is reachable. Same for retain (`MemoryInput`), reflect (`ReflectParams`), and all
list/filter operations.

### 3.5 The recall→prompt bridge (new usage)

`bank.recall_text(params)` returns memories pre-formatted as an LLM-ready context string
(sections by fact type, entities/timestamps inline) — the "2-line agent memory" use case
the official clients solve with wrappers, solved in 0 extra deps.

### 3.6 Structured reflect (killer feature)

Behind the optional `schemars` feature: `bank.reflect_structured::<T>(params)` derives the
JSON schema from `T: JsonSchema`, sends it as `response_schema`, and returns typed
`T` (mapping `structured_output_error`/null to a typed error). Turn "reflect" into a
type-safe, memory-grounded extraction/answer engine.

### 3.7 Async pipelines (new usage)

- `bank.retain_async(...)` → `OperationId`, `client.wait_for_operation(bank, id, policy)`
  polls with backoff until terminal state, returns the final `OperationStatusResponse`.
- `retain_files` uploads PDFs/images (multipart) and hands back operation ids for tracking.
- Bounded-concurrency helper `util::map_concurrent` for multi-bank fan-out without `futures` boilerplate in user code.

### 3.8 Pagination

Every list endpoint returns `Page<T> { items, total, limit, offset }`; `Pager<T>` drives
`next_page()` / `collect_all()` with the same filters, transparently advancing offsets.

### 3.9 Performance

1. One pooled `reqwest::Client` (HTTP/2, keepalive, `tcp_nodelay`, tuned idle pool) shared via `Arc`.
2. Zero `serde_json::Value` on hot paths — typed structs, `skip_serializing_if` everywhere
   (no `null` payloads, smaller bodies).
3. gzip/brotli/zstd response decompression enabled.
4. Percent-encoding via `Url` (no re-parse per segment).
5. Async retain + operation polling instead of holding connections during LLM extraction.
6. `&str`/`AsRef<str>` APIs, no `String` traffic requirements; `Content` serializes as a
   plain string in the common case (no block wrapping).
7. Per-request timeout override (reflect) so a global short timeout never forces reconnect churn.

### 3.10 Coverage

All 79 documented paths minus the two the server itself marks "removed" (profile GET,
disposition PUT, background merge PUT legacy variants): **96 methods** across monitoring,
retain/recall/reflect, memories, banks (+aliases, config, stats, timeseries, LLM health,
consolidation), entities, tags, observations, mental models, knowledge base, directives,
documents (+chunks), operations, files, transfer, webhooks, audit, LLM traces, templates.

### 3.11 What is deliberately *not* here

- No blocking client (agents are async; a footgun `block_on` wrapper costs more than it buys).
- No SSE streaming (the API returns JSON; nothing to stream).
- No webhook receiving server (webhooks are registered/managed here, not served).

## 4. Testing strategy

wiremock-based integration tests per endpoint group (paths, methods, bodies, query strings),
serialization unit tests for every wire-relevant model, retry/pagination/error-mapping
behavior tests, plus `cargo clippy -D warnings`, `cargo fmt --check`, `cargo doc` clean.
