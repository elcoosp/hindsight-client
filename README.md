<div align="center">
  <img src="docs/logo.png" alt="hindsight-client Logo" width="300"/>
  <p>
    <strong>hindsight-client v2</strong><br/>
    The fast, typed, batteries-included async Rust client for the
    <a href="https://hindsight.vectorize.io/">Hindsight</a> agent memory system.
  </p>
  <p>
    <img src="https://img.shields.io/badge/Rust-1.85%2B-000000?style=flat-square&logo=rust" alt="Rust"/>
    <img src="https://img.shields.io/badge/License-MIT-blue?style=flat-square" alt="License MIT"/>
    <img src="https://img.shields.io/badge/API-96%20methods-6C47FF?style=flat-square" alt="API surface"/>
  </p>
</div>

---

`hindsight-client` gives your agents long-term memory over
[Hindsight](https://hindsight.vectorize.io) — **retain** facts, **recall** them with
4-way search (semantic + keyword + graph + temporal), and **reflect** with an agentic
loop grounded in the bank. Works against a self-hosted server (`http://localhost:8888`)
or [Hindsight Cloud](https://api.hindsight.vectorize.io).

Why v2 beats a raw HTTP layer (and the generated clients):

| | v0.1 / raw HTTP | **v2** |
|---|---|---|
| API surface | 3 endpoints (wrong paths) | **96 methods, full v1.1.0 spec** |
| Auth | none | `Bearer hsk_…` via builder or env |
| Recall model | `hits[].content/score` (not the real schema) | typed `RecallResult` with scores, entities, temporal fields |
| Retain | flat body the server 422s | multimodal `MemoryItem`s, async mode, idempotency keys |
| Errors | one opaque variant | `Auth` / `NotFound` / `RateLimited` / `Api`(+parsed validation) / `Timeout` / `StructuredOutput` |
| Retries | none | jittered backoff, `Retry-After`, writes opt-in |
| Pagination | none | `Pager` with `collect_all()` |
| Reflect | raw string | citations, usage, trace, **typed structured output** |
| Timeouts | fixed 30 s (reflect dies) | 120 s default, reflect ≥ 300 s, per-call override |
| Bank ids | broken on spaces/slashes | percent-encoded via `Url` |

## Installation

```toml
[dependencies]
hindsight-client = "0.2"
tokio = { version = "1", features = ["full"] }
```

Optional feature: `schemars` — enables `reflect_structured::<T>()` with
schema-derived JSON contracts.

## Quick start

```rust
use hindsight_client::Client;

#[tokio::main]
async fn main() -> hindsight_client::Result<()> {
    // From env: HINDSIGHT_API_URL + HINDSIGHT_API_KEY
    let client = Client::from_env()?;
    // or explicitly:
    // let client = Client::builder().base_url("http://localhost:8888").api_key("hsk_…").build()?;

    let bank = client.bank("my-bank"); // one handle, ids auto-encoded

    bank.retain(["Alice works at Google on the AI team"]).await?;
    let hits = bank.recall("Where does Alice work?").await?;
    println!("{}", hits.to_context_string()); // LLM-ready context block

    let answer = bank.reflect_text("Summarize what you know about Alice.").await?;
    println!("{answer}");
    Ok(())
}
```

## The agent memory loop (2 lines per turn)

```rust
let context = bank.recall_text("code review preferences").await?;   // before the call
let submitted = bank.retain(["User asked to shorten PRs"]).await?;  // after the call
```

Need the write off the hot path? `bank.retain_async(...)` returns an operation id;
`bank.wait_for_operation(id, OperationWait::new())` polls to a terminal state.

## Structured reflect (feature `schemars`)

Turn reflect into a type-safe, memory-grounded extraction engine: the client derives
the JSON schema from your type, the server validates against it, and you get `T` back.

```rust
#[derive(serde::Deserialize, schemars::JsonSchema)]
struct Answer { summary: String, key_points: Vec<String> }

let answer: Answer = bank
    .reflect_structured(ReflectRequest::new("what did we decide about budgets?"))
    .await?;
```

Failures surface as `Error::StructuredOutput { message, value }` — never a silent `None`.

## Full API map

Every bank-scoped operation hangs off `client.bank("id")`:

- **Memory**: `retain`, `retain_async`, `retain_with`, `retain_files` (multipart PDFs/images),
  `dry_run_extract`, `recall`, `recall_with`, `recall_text`, `reflect`, `reflect_text`,
  `reflect_structured`
- **Memory management**: `list_memories`(+`_paged`), `get_memory`, `curate_memory`,
  `memory_history`, `clear_memory_observations`, `clear_memories`, `graph`, `list_tags`,
  `list_observation_scopes`, `clear_all_observations`, `preview_consolidation_strategies`
- **Banks**: `create_or_update_bank`, `update_bank`, `delete_bank`, `list_banks`(+`_paged`),
  `get_bank_config`, `update_bank_config`, `reset_bank_config`, `get_bank_stats`,
  `memories_timeseries`, `bank_llm_health`, `get_bank_profile`, `set_bank_background`,
  aliases (`list/add/set_primary/remove`), `consolidate`, `recover_consolidation`
- **Mental models**: CRUD + `refresh`, `dry_run_refresh`, `clear`, `history`
- **Knowledge base**: `knowledge_tree`, `create_knowledge_folder`, `create_knowledge_page`,
  `get_knowledge_page`, `search_knowledge_base`, `export_knowledge_base`,
  `update_knowledge_node`, `delete_knowledge_node`
- **Directives**: CRUD
- **Documents**: `list_documents`(+`_paged`), `get_document`, `update_document`,
  `delete_document`, `document_history`, `list_document_chunks`, `reprocess_document`,
  `get_attachment`
- **Operations**: `list_operations`, `get_operation`, `cancel_operation`,
  `retry_operation`, `delete_operation`, `wait_for_operation`
- **Transfer**: `export_bank`, `import_bank`, `clone_bank`, `export_documents`,
  `import_documents`, `list_document_transfers`, bank templates (`import/export/schema`)
- **Webhooks**: `register_webhook`, `list_webhooks`, `update_webhook`,
  `delete_webhook`, `list_webhook_deliveries`
- **Observability**: audit logs (`list/stats/get`), LLM request traces (`list/stats`),
  `preview_prompts` (render prompts without an LLM call)
- **Root**: `health`, `readiness`, `liveness`, `version` (feature flags!),
  `metrics`, `get_chunk`, `download_file`

## Errors you can match

```rust
use hindsight_client::{Client, Error};

match client.bank("nope").recall("anything").await {
    Ok(r) => println!("{} hits", r.results.len()),
    Err(Error::NotFound { message }) => eprintln!("bank missing: {message}"),
    Err(Error::RateLimited { retry_after }) => eprintln!("back off: {retry_after:?}"),
    Err(Error::Timeout) => eprintln!("reflect is thinking too long"),
    Err(e) => eprintln!("other: {e}"),
}
```

FastAPI `422` bodies are parsed: `err.validation_errors()` returns
`Vec<FieldError { loc, msg, kind }>` for form-level error rendering.

## Resilience

- `RetryPolicy::new()` — 2 retries, full-jitter exponential backoff (100 ms → 5 s),
  on `429/500/502/503/504` and connection errors, **reads only** by default.
- `ClientBuilder::retry_writes(true)` — opt in to retrying writes (safe for reflect,
  or retains that deduplicate by content hash).
- `Retry-After` is honored verbatim on `429`/`503`.
- `ClientBuilder::timeout(...)` sets the global budget; reflect calls automatically get
  ≥ 300 s (the agentic loop needs it).

## Pagination

```rust
let all = client.bank("b")
    .list_memories_paged(hindsight_client::ListMemoriesQuery::new(), 200)
    .collect_all()
    .await?;
```

`Pager` also exposes `next_page()` for streaming UIs and `total()` after the first page.

## Performance notes

- One pooled `reqwest::Client` per `Client` (HTTP/2, keepalive, `tcp_nodelay`,
  tuned idle pool) shared through `Arc` — cloning the client is free.
- Typed structs with `skip_serializing_if` everywhere: no `null` noise on the wire,
  no `serde_json::Value` on hot paths.
- gzip/brotli response decompression enabled.
- Async retain + operation polling instead of holding connections through LLM extraction.
- `hindsight_client::util::map_concurrent` for bounded multi-bank fan-out that keeps
  input order.

## Migration from 0.1

v2 is intentionally breaking (see [DESIGN.md](DESIGN.md) for the full audit):

| 0.1 | 0.2 |
|---|---|
| `HindsightClient::new(url)` | `Client::builder().base_url(url).build()?` / `Client::from_env()?` |
| `retain(bank, content, ctx, ts)` | `client.bank(bank).retain([MemoryItem::new(content).context(ctx)])` |
| `recall(bank, query, top_k)` | `client.bank(bank).recall(RecallRequest::new(query).max_tokens(k))` |
| `reflect(bank, query) -> String` | `client.bank(bank).reflect_text(query)` |
| `MemoryHit { content, score }` | `RecallResult` (id/text/type/entities/scores/…) |
| `Error::ApiError(status, text)` | `Error::Api { status, message, detail }` + specific variants |

## Development

```bash
cargo test --all-features   # unit + wiremock integration + structured + doc tests
cargo clippy --all-features --all-targets  # pedantic, clean
cargo fmt --check
```

> [!NOTE]
> **Server requirement** — this crate talks to a running Hindsight server
> ([Docker quickstart](https://hindsight.vectorize.io/developer/installation))
> or Hindsight Cloud with an `hsk_…` key.

---

*MIT licensed. Design rationale and the 0.1 API audit live in [DESIGN.md](DESIGN.md).*
