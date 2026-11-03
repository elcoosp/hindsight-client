//! End-to-end tests against a wiremock server, asserting exact paths, methods,
//! bodies, query strings, auth, retries, pagination and error mapping.

use std::time::Duration;

use hindsight_client::{
    BankSettings, Client, CreateDirectiveRequest, CreateMentalModelRequest, CreateWebhookRequest,
    Error, FactType, ListBanksQuery, ListMemoriesQuery, ListTagsQuery, RecallRequest,
    ReflectRequest, RetainRequest, TagsMatch, TimeField, UploadedFile,
};
use serde_json::json;
use wiremock::matchers::{body_json, body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn server() -> MockServer {
    MockServer::start().await
}

fn client(url: String) -> Client {
    Client::builder().base_url(url).build().unwrap()
}

fn keyed_client(url: String) -> Client {
    Client::builder()
        .base_url(url)
        .api_key("hsk_test")
        .build()
        .unwrap()
}

// ----------------------------------------------------------------------
// Plumbing: auth, user agent, encoding, prefix
// ----------------------------------------------------------------------

#[tokio::test]
async fn auth_header_is_sent_when_key_configured() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks"))
        .and(header("authorization", "Bearer hsk_test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "banks": [], "total": 0, "limit": 100, "offset": 0
        })))
        .mount(&s)
        .await;
    keyed_client(s.uri())
        .list_banks(ListBanksQuery::new())
        .await
        .unwrap();
}

#[tokio::test]
async fn bank_id_is_percent_encoded() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/my%20bank%2Fx/memories"))
        .and(body_json(json!({"items": [{"content": "hi"}]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "bank_id": "my bank/x", "items_count": 1, "async": false
        })))
        .mount(&s)
        .await;
    let c = client(s.uri());
    c.bank("my bank/x").retain(["hi"]).await.unwrap();
}

#[tokio::test]
async fn custom_path_prefix_is_respected() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/acme/banks/b/memories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "bank_id": "b", "items_count": 1, "async": false
        })))
        .mount(&s)
        .await;
    let c = Client::builder()
        .base_url(s.uri())
        .path_prefix("/v1/acme")
        .build()
        .unwrap();
    c.bank("b").retain(["hi"]).await.unwrap();
}

#[tokio::test]
async fn empty_bank_id_is_rejected_locally() {
    let s = server().await;
    let c = client(s.uri());
    let err = c.bank("").retain(["x"]).await.unwrap_err();
    assert!(err.is_invalid_input());
}

// ----------------------------------------------------------------------
// Retain
// ----------------------------------------------------------------------

#[tokio::test]
async fn retain_sends_items_and_parses_response() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/memories"))
        .and(body_json(json!({
            "items": [
                {"content": "Alice works at Google", "context": "work",
                 "metadata": {"src": "chat"}, "tags": ["u1"]},
                {"content": "Bob likes hiking", "timestamp": "2026-01-02T03:04:05Z"}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "bank_id": "b", "items_count": 2, "async": false,
            "usage": {"input_tokens": 10, "output_tokens": 2, "total_tokens": 12}
        })))
        .mount(&s)
        .await;
    let c = keyed_client(s.uri());
    let resp = c
        .bank("b")
        .retain([
            hindsight_client::MemoryItem::new("Alice works at Google")
                .context("work")
                .meta("src", "chat")
                .tags(["u1"]),
            hindsight_client::MemoryItem::new("Bob likes hiking").timestamp("2026-01-02T03:04:05Z"),
        ])
        .await
        .unwrap();
    assert!(resp.success);
    assert_eq!(resp.items_count, 2);
    assert_eq!(resp.usage.total_tokens, 12);
    assert!(!resp.processed_async);
}

#[tokio::test]
async fn retain_async_sets_wire_async_true() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/memories"))
        .and(body_json(
            json!({"items": [{"content": "x"}], "async": true, "operation_id": "op-1"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "bank_id": "b", "items_count": 1, "async": true, "operation_id": "op-1"
        })))
        .mount(&s)
        .await;
    let c = client(s.uri());
    let resp = c
        .bank("b")
        .retain_with(RetainRequest::new(["x"]).as_async().operation_id("op-1"))
        .await
        .unwrap();
    assert_eq!(resp.operation_id.as_deref(), Some("op-1"));
}

#[tokio::test]
async fn retain_rejects_empty_items_without_http_call() {
    let s = server().await;
    let c = client(s.uri());
    let err = c.bank("b").retain(Vec::<String>::new()).await.unwrap_err();
    assert!(err.is_invalid_input());
}

#[tokio::test]
async fn retain_supports_multimodal_content_blocks() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/memories"))
        .and(body_json(json!({
            "items": [{
                "content": [
                    {"type": "text", "text": "See attached"},
                    {"type": "image", "source": {"media_type": "image/png", "data": "aGk="}}
                ]
            }]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "bank_id": "b", "items_count": 1, "async": false
        })))
        .mount(&s)
        .await;
    let c = client(s.uri());
    c.bank("b")
        .retain([hindsight_client::MemoryItem::new(vec![
            hindsight_client::ContentBlock::Text {
                text: "See attached".into(),
            },
            hindsight_client::ContentBlock::Image {
                source: hindsight_client::AttachmentSource::from_bytes("image/png", b"hi"),
            },
        ])])
        .await
        .unwrap();
}

// ----------------------------------------------------------------------
// Recall & reflect
// ----------------------------------------------------------------------

#[tokio::test]
async fn recall_parses_results_scores_and_includes() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/memories/recall"))
        .and(body_json(json!({
            "query": "who is alice?",
            "types": ["world", "observation"],
            "budget": "high",
            "max_tokens": 2048,
            "tags": ["u1", "u2"],
            "tags_match": "all_strict",
            "include": {"chunks": {"max_tokens": 4096}}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {"id": "1", "text": "Alice works at Google", "type": "world",
                 "entities": ["Alice", "Google"], "context": "work",
                 "scores": {"final": 0.9, "semantic": 0.8}},
                {"id": "2", "text": "Alice leads the AI team", "type": "observation",
                 "source_fact_ids": ["1"]}
            ],
            "chunks": {"c1": {"id": "c1", "chunk_index": 0, "text": "chunk text"}}
        })))
        .mount(&s)
        .await;
    let c = client(s.uri());
    let resp = c
        .bank("b")
        .recall(
            RecallRequest::new("who is alice?")
                .types([FactType::World, FactType::Observation])
                .budget(hindsight_client::Budget::High)
                .max_tokens(2048)
                .tags(["u1", "u2"])
                .tags_match(TagsMatch::AllStrict)
                .include_chunks(4096),
        )
        .await
        .unwrap();
    assert_eq!(resp.results.len(), 2);
    let first = &resp.results[0];
    assert_eq!(first.text, "Alice works at Google");
    let scores = first.scores.as_ref().unwrap();
    assert!((scores.final_score - 0.9).abs() < 1e-9);
    assert_eq!(scores.semantic, Some(0.8));
    assert!(scores.reranker.is_none());
    assert_eq!(resp.chunks.as_ref().unwrap()["c1"].text, "chunk text");

    let ctx = resp.to_context_string();
    assert!(ctx.contains("# world\n- Alice works at Google"));
    assert!(ctx.contains("[entities: Alice, Google]"));
    assert!(ctx.contains("# observation"));
}

#[tokio::test]
async fn recall_plain_string_query_works() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/memories/recall"))
        .and(body_json(json!({"query": "hello"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results": []})))
        .mount(&s)
        .await;
    let c = client(s.uri());
    let resp = c.bank("b").recall("hello").await.unwrap();
    assert!(resp.results.is_empty());
    assert_eq!(resp.to_context_string(), "");
}

#[tokio::test]
async fn reflect_parses_full_response() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/reflect"))
        .and(body_json(json!({
            "query": "what do you know about alice?",
            "budget": "low",
            "apply_all_directives": true,
            "exclude_mental_models": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "Alice works at Google.",
            "based_on": {
                "memories": [{"id": "1", "text": "Alice works at Google", "type": "world"}],
                "directives": [{"id": "d1", "name": "tone", "content": "be brief"}]
            },
            "usage": {"input_tokens": 100, "output_tokens": 10, "total_tokens": 110},
            "trace": {"tool_calls": [{"tool": "recall", "input": {"query": "alice"},
                                      "duration_ms": 5, "iteration": 1}],
                      "llm_calls": [{"scope": "agent_1", "duration_ms": 100}]}
        })))
        .mount(&s)
        .await;
    let c = client(s.uri());
    let resp = c
        .bank("b")
        .reflect(
            ReflectRequest::new("what do you know about alice?")
                .budget(hindsight_client::Budget::Low)
                .apply_all_directives(true)
                .exclude_mental_models(None),
        )
        .await
        .unwrap();
    assert_eq!(resp.text, "Alice works at Google.");
    assert_eq!(resp.based_on.memories.len(), 1);
    assert_eq!(resp.based_on.directives.len(), 1);
    assert_eq!(resp.usage.total_tokens, 110);
    assert_eq!(resp.trace.unwrap().tool_calls.len(), 1);
}

#[tokio::test]
async fn reflect_text_returns_only_the_answer() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/reflect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"text": "42"})))
        .mount(&s)
        .await;
    let c = client(s.uri());
    assert_eq!(c.bank("b").reflect_text("meaning?").await.unwrap(), "42");
}

// ----------------------------------------------------------------------
// Error mapping & retries
// ----------------------------------------------------------------------

#[tokio::test]
async fn error_status_mapping() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/gone/memories/recall"))
        .respond_with(ResponseTemplate::new(404).set_body_string("bank not found"))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/locked/memories/recall"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"detail": "bad key"})))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/bad/memories/recall"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "detail": [
                {"loc": ["body", "query"], "msg": "field required", "type": "missing"},
                {"loc": ["body", "items", 0, "content"], "msg": "empty", "type": "value_error"}
            ]
        })))
        .mount(&s)
        .await;
    let c = client(s.uri());

    match c.bank("gone").recall("q").await {
        Err(Error::NotFound { message }) => assert!(message.contains("bank not found")),
        other => panic!("expected NotFound, got {other:?}"),
    }
    match c.bank("locked").recall("q").await {
        Err(Error::Auth { status, message }) => {
            assert_eq!(status.as_u16(), 401);
            assert_eq!(message, "bad key");
        }
        other => panic!("expected Auth, got {other:?}"),
    }
    match c.bank("bad").recall("q").await {
        Err(e @ Error::Api { status, .. }) => {
            assert_eq!(status.as_u16(), 422);
            let fields = e.validation_errors();
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].loc, vec!["body".to_string(), "query".to_string()]);
            assert!(e.to_string().contains("body.query: field required"));
        }
        other => panic!("expected Api, got {other:?}"),
    }
}

#[tokio::test]
async fn reads_are_retried_on_500_then_succeed() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/stats"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .up_to_n_times(1)
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/stats"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "bank_id": "b", "total_nodes": 3, "total_links": 0, "total_documents": 0,
            "pending_operations": 0, "failed_operations": 0
        })))
        .mount(&s)
        .await;
    let c = client(s.uri());
    let stats = c.bank("b").get_bank_stats().await.unwrap();
    assert_eq!(stats.total_nodes, 3);
}

#[tokio::test]
async fn writes_are_not_retried_by_default() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/memories"))
        .respond_with(ResponseTemplate::new(503).set_body_string("overloaded"))
        .expect(1)
        .mount(&s)
        .await;
    let c = client(s.uri());
    let err = c.bank("b").retain(["x"]).await.unwrap_err();
    assert!(matches!(
        err,
        Error::Api { .. } | Error::RetriesExhausted { .. }
    ));
    // wiremock verifies the expectation count on drop.
}

#[tokio::test]
async fn writes_are_retried_when_enabled() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/memories"))
        .respond_with(ResponseTemplate::new(503).set_body_string("overloaded"))
        .up_to_n_times(1)
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/memories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "bank_id": "b", "items_count": 1, "async": false
        })))
        .mount(&s)
        .await;
    let c = Client::builder()
        .base_url(s.uri())
        .retry_policy(hindsight_client::RetryPolicy::new().base_delay(Duration::from_millis(1)))
        .retry_writes(true)
        .build()
        .unwrap();
    c.bank("b").retain(["x"]).await.unwrap();
}

#[tokio::test]
async fn rate_limit_is_retried_then_surfaced() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/tags"))
        .and(query_param("q", "user"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_string("slow")
                .append_header("Retry-After", "1"),
        )
        .expect(3)
        .mount(&s)
        .await;
    let c = Client::builder()
        .base_url(s.uri())
        .retry_policy(
            hindsight_client::RetryPolicy::new()
                .retries(2)
                .base_delay(Duration::from_millis(1))
                .max_delay(Duration::from_millis(2)),
        )
        .build()
        .unwrap();
    let err = c
        .bank("b")
        .list_tags(ListTagsQuery::new().search("user"))
        .await
        .unwrap_err();
    assert!(err.is_rate_limited());
}

#[tokio::test]
async fn timeout_maps_to_timeout_error() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/version"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(300)))
        .mount(&s)
        .await;
    let c = Client::builder()
        .base_url(s.uri())
        .timeout(Duration::from_millis(50))
        .build()
        .unwrap();
    assert!(matches!(c.version().await, Err(Error::Timeout)));
}

// ----------------------------------------------------------------------
// Pagination
// ----------------------------------------------------------------------

#[tokio::test]
async fn list_memories_pages_through_everything() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/memories/list"))
        .and(query_param("limit", "2"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                {"id": "1", "text": "one", "state": "valid", "proof_count": 1},
                {"id": "2", "text": "two", "state": "valid", "proof_count": 1}
            ],
            "total": 3, "limit": 2, "offset": 0
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/memories/list"))
        .and(query_param("offset", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"id": "3", "text": "three", "state": "valid", "proof_count": 1}],
            "total": 3, "limit": 2, "offset": 2
        })))
        .mount(&s)
        .await;
    let c = client(s.uri());
    let all = c
        .bank("b")
        .list_memories_paged(ListMemoriesQuery::new(), 2)
        .collect_all()
        .await
        .unwrap();
    assert_eq!(
        all.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["1", "2", "3"]
    );
}

#[tokio::test]
async fn list_memories_query_serializes_time_window_and_type() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/memories/list"))
        .and(query_param("type", "world"))
        .and(query_param("time_field", "mentioned_at"))
        .and(query_param("start_date", "2026-01-01T00:00:00Z"))
        .and(query_param("tags", "u1"))
        .and(query_param("tags_match", "all"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [], "total": 0, "limit": 50, "offset": 0
        })))
        .mount(&s)
        .await;
    let c = client(s.uri());
    c.bank("b")
        .list_memories(
            ListMemoriesQuery::new()
                .fact_type(FactType::World)
                .tags(["u1"])
                .tags_match(TagsMatch::All)
                .time_window(
                    TimeField::MentionedAt,
                    Some("2026-01-01T00:00:00Z".into()),
                    None,
                )
                .limit(50),
        )
        .await
        .unwrap();
}

// ----------------------------------------------------------------------
// Memories management
// ----------------------------------------------------------------------

#[tokio::test]
async fn memory_curate_and_history_and_clear() {
    let s = server().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/default/banks/b/memories/m1"))
        .and(body_json(
            json!({"state": "invalidated", "reason": "stale", "resolve_entities": false}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "m1", "state": "invalidated"})),
        )
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/memories/m1/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"changed_at": "2026-01-01", "source_facts": ["f1"]}
        ])))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/b/memories/m1/observations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"deleted_count": 3})))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/b/memories"))
        .and(query_param("type", "experience"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
        .mount(&s)
        .await;

    let c = client(s.uri());
    let bank = c.bank("b");
    bank.curate_memory(
        "m1",
        hindsight_client::UpdateMemoryRequest::new()
            .invalidate(Some("stale".into()))
            .literal_entities(),
    )
    .await
    .unwrap();
    let history = bank.memory_history("m1").await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].fields["source_facts"], json!(["f1"]));
    let cleared = bank.clear_memory_observations("m1").await.unwrap();
    assert_eq!(cleared.deleted_count, 3);
    bank.clear_memories(Some(FactType::Experience))
        .await
        .unwrap();
}

#[tokio::test]
async fn graph_and_dry_run_extract() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/graph"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "nodes": [{"data": {"id": "1", "label": "Alice works", "text": "Alice works"}}],
            "edges": [{"data": {"id": "1-2-semantic", "linkType": "semantic", "source": "1", "target": "2", "weight": 0.5}}],
            "table_rows": [],
            "total_units": 2,
            "limit": 10
        })))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/memories/dry-run-extract"))
        .and(body_json(
            json!({"content": "Alice works at Google", "context": "meeting"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "facts": [{"text": "Alice works at Google", "fact_type": "world",
                        "entities": ["Alice", "Google"], "chunk_index": 0}],
            "chunks": [{"text": "Alice works at Google", "fact_count": 1}],
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        })))
        .mount(&s)
        .await;
    let c = client(s.uri());
    let graph = c
        .bank("b")
        .graph(hindsight_client::GraphQuery::new().limit(10))
        .await
        .unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.edges[0].data.link_type, "semantic");
    let dry = c
        .bank("b")
        .dry_run_extract(
            hindsight_client::DryRunExtractRequest::new("Alice works at Google").context("meeting"),
        )
        .await
        .unwrap();
    assert_eq!(dry.facts.len(), 1);
    assert_eq!(
        dry.facts[0].entities.as_deref(),
        Some(&["Alice".to_string(), "Google".to_string()][..])
    );
    assert_eq!(dry.usage.total_tokens, 15);
}

// ----------------------------------------------------------------------
// Banks
// ----------------------------------------------------------------------

#[tokio::test]
async fn bank_lifecycle_and_config() {
    let s = server().await;
    Mock::given(method("PUT"))
        .and(path("/v1/default/banks/new-bank"))
        .and(body_json(json!({
            "name": "Test Bank", "mission": "be helpful",
            "disposition": {"skepticism": 4, "literalism": 2, "empathy": 5},
            "enable_observations": true
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"bank_id": "new-bank", "name": "Test Bank"})),
        )
        .mount(&s)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/default/banks/new-bank"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"bank_id": "new-bank"})))
        .mount(&s)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/default/banks/new-bank/config"))
        .and(body_json(json!({"updates": {"retain_chunk_size": 1024}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "bank_id": "new-bank", "config": {"retain_chunk_size": 1024}, "overrides": {"retain_chunk_size": 1024}
        })))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/new-bank/config"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "bank_id": "new-bank", "config": {}, "overrides": {}
        })))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/new-bank"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
        .mount(&s)
        .await;

    let c = keyed_client(s.uri());
    let settings = BankSettings::new()
        .name("Test Bank")
        .mission("be helpful")
        .traits(4, 2, 5)
        .enable_observations(true);
    let created = c.create_or_update_bank("new-bank", settings).await.unwrap();
    assert_eq!(created.bank_id, "new-bank");
    c.update_bank("new-bank", BankSettings::new())
        .await
        .unwrap();
    let cfg = c
        .bank("new-bank")
        .update_bank_config(
            hindsight_client::BankConfigUpdate::from_json(json!({"retain_chunk_size": 1024}))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cfg.config["retain_chunk_size"], 1024);
    c.bank("new-bank").reset_bank_config().await.unwrap();
    c.delete_bank("new-bank").await.unwrap();
}

#[tokio::test]
async fn aliases_and_stats_and_background() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/aliases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "bank_id": "b",
            "aliases": [{"alias": "user-123", "primary": true}]
        })))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/aliases"))
        .and(body_json(json!({"alias": "user-123", "primary": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&s)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/default/banks/b/aliases/user-123"))
        .and(body_json(json!({"primary": false})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/b/aliases/user-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/background"))
        .and(body_json(json!({"content": "We are a fintech team"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "mission": "m", "background": "We are a fintech team",
            "disposition": {"skepticism": 3, "literalism": 3, "empathy": 3}
        })))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/consolidate"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"operation_id": "op-c", "deduplicated": false})),
        )
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/consolidation/recover"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"retried_count": 2})))
        .mount(&s)
        .await;

    let c = client(s.uri());
    let bank = c.bank("b");
    let aliases = bank.list_aliases().await.unwrap();
    assert_eq!(aliases.aliases[0].alias, "user-123");
    bank.add_alias("user-123", true).await.unwrap();
    bank.set_primary_alias("user-123", false).await.unwrap();
    bank.remove_alias("user-123").await.unwrap();
    let bg = bank
        .set_bank_background("We are a fintech team")
        .await
        .unwrap();
    assert_eq!(bg.background.as_deref(), Some("We are a fintech team"));
    let con = bank
        .consolidate(hindsight_client::ConsolidationRequest::default())
        .await
        .unwrap();
    assert_eq!(con.operation_id, "op-c");
    let rec = bank.recover_consolidation().await.unwrap();
    assert_eq!(rec.retried_count, 2);
}

// ----------------------------------------------------------------------
// Mental models & directives
// ----------------------------------------------------------------------

#[tokio::test]
async fn mental_model_crud_and_refresh() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/mental-models"))
        .and(body_json(json!({
            "name": "User profile", "source_query": "What do I know about the user?",
            "tags": ["core"], "max_tokens": 1024
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "mm-1", "bank_id": "b", "name": "User profile",
            "source_query": "What do I know about the user?", "content": "",
            "tags": ["core"], "max_tokens": 1024, "is_stale": true
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/mental-models/mm-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "mm-1", "bank_id": "b", "name": "User profile", "content": "They use Rust."
        })))
        .mount(&s)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/default/banks/b/mental-models/mm-1"))
        .and(body_json(json!({"name": "Profile v2"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "mm-1", "bank_id": "b", "name": "Profile v2"
        })))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/mental-models/mm-1/refresh"))
        .and(body_json(json!({"mode": "delta"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "mm-1", "bank_id": "b", "name": "Profile v2", "content": "updated"
        })))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path(
            "/v1/default/banks/b/mental-models/mm-1/dry-run-refresh",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "mental_model_id": "mm-1", "name": "Profile v2",
            "requested_mode": "delta", "effective_mode": "full",
            "outcome": "content_written", "would_persist": true,
            "scope": {}, "window": {}, "facts": {},
            "current_content": "a", "candidate_content": "b", "preview_content": "b",
            "diff": "", "delta_operations": {}, "retraction": {}, "trace": {},
            "usage": {}, "duration_ms": 12, "warnings": ["first refresh"]
        })))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/mental-models/mm-1/clear"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "mm-1", "bank_id": "b", "name": "Profile v2", "content": ""
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/mental-models/mm-1/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"content": "old"}])))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/b/mental-models/mm-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
        .mount(&s)
        .await;

    let c = client(s.uri());
    let bank = c.bank("b");
    let mm = bank
        .create_mental_model(
            CreateMentalModelRequest::new("User profile", "What do I know about the user?")
                .tags(["core"])
                .max_tokens(1024),
        )
        .await
        .unwrap();
    assert_eq!(mm.id, "mm-1");
    bank.get_mental_model("mm-1").await.unwrap();
    bank.update_mental_model(
        "mm-1",
        hindsight_client::UpdateMentalModelRequest::new().name("Profile v2"),
    )
    .await
    .unwrap();
    bank.refresh_mental_model(
        "mm-1",
        hindsight_client::RefreshMentalModelRequest::new().mode("delta"),
    )
    .await
    .unwrap();
    let dry = bank
        .dry_run_refresh_mental_model("mm-1", hindsight_client::RefreshMentalModelRequest::new())
        .await
        .unwrap();
    assert!(dry.would_persist);
    assert_eq!(dry.warnings, Some(vec!["first refresh".to_string()]));
    bank.clear_mental_model("mm-1").await.unwrap();
    let hist = bank.mental_model_history("mm-1").await.unwrap();
    assert_eq!(hist[0].content.as_deref(), Some("old"));
    bank.delete_mental_model("mm-1").await.unwrap();
}

#[tokio::test]
async fn directive_crud() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/directives"))
        .and(body_json(json!({
            "name": "no stocks", "content": "Never recommend specific stocks.",
            "priority": 10, "is_active": true, "tags": ["finance"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "d1", "bank_id": "b", "name": "no stocks",
            "content": "Never recommend specific stocks.", "priority": 10,
            "is_active": true, "tags": ["finance"]
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/directives/d1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "d1", "bank_id": "b", "name": "no stocks",
            "content": "Never recommend specific stocks.", "priority": 10, "is_active": true
        })))
        .mount(&s)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/default/banks/b/directives/d1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "d1", "bank_id": "b", "name": "no stocks",
            "content": "Never recommend specific stocks.", "priority": 10, "is_active": false
        })))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/b/directives/d1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
        .mount(&s)
        .await;
    let c = client(s.uri());
    let bank = c.bank("b");
    let d = bank
        .create_directive(
            CreateDirectiveRequest::new("no stocks", "Never recommend specific stocks.")
                .priority(10)
                .tags(["finance"]),
        )
        .await
        .unwrap();
    assert_eq!(d.id, "d1");
    bank.get_directive("d1").await.unwrap();
    bank.update_directive(
        "d1",
        hindsight_client::UpdateDirectiveRequest::new().active(false),
    )
    .await
    .unwrap();
    bank.delete_directive("d1").await.unwrap();
}

// ----------------------------------------------------------------------
// Documents, entities, tags, observations
// ----------------------------------------------------------------------

#[tokio::test]
async fn documents_flow() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/documents"))
        .and(query_param("q", "meeting"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"id": "doc-1", "text_length": 100, "memory_unit_count": 3}],
            "total": 1, "limit": 100, "offset": 0
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/documents/doc-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "doc-1", "bank_id": "b", "original_text": "meeting notes",
            "content_hash": "abc", "created_at": "2026-01-01", "updated_at": "2026-01-01",
            "memory_unit_count": 3, "tags": ["work"]
        })))
        .mount(&s)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/default/banks/b/documents/doc-1"))
        .and(body_json(
            json!({"tags": ["work", "q1"], "document_metadata": {"team": "core"}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "doc-1", "bank_id": "b", "created_at": "2026-01-01", "updated_at": "2026-01-02",
            "memory_unit_count": 3, "tags": ["work", "q1"]
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/documents/doc-1/chunks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"id": "c1", "document_id": "doc-1", "chunk_index": 0, "text": "meeting notes"}],
            "total": 1, "limit": 100, "offset": 0
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/documents/doc-1/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "document_id": "doc-1", "items": [{"document_id": "doc-1", "kind": "retain"}],
            "total": 1, "limit": 100, "offset": 0
        })))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/documents/doc-1/reprocess"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "operation_id": "op-r", "items_count": 1
        })))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/b/documents/doc-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/chunks/c1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "c1", "document_id": "doc-1", "chunk_index": 0, "text": "meeting notes"
        })))
        .mount(&s)
        .await;

    let c = client(s.uri());
    let bank = c.bank("b");
    let docs = bank
        .list_documents(hindsight_client::ListDocumentsQuery::new().search("meeting"))
        .await
        .unwrap();
    assert_eq!(docs.items[0].id, "doc-1");
    bank.get_document("doc-1").await.unwrap();
    let updated = bank
        .update_document(
            "doc-1",
            hindsight_client::UpdateDocumentRequest::new()
                .tags(["work", "q1"])
                .metadata([("team", "core")]),
        )
        .await
        .unwrap();
    assert_eq!(updated.tags, Some(vec!["work".into(), "q1".into()]));
    let chunks = bank
        .list_document_chunks("doc-1", None, None)
        .await
        .unwrap();
    assert_eq!(chunks.items[0].text, "meeting notes");
    let hist = bank
        .document_history("doc-1", hindsight_client::DocumentHistoryQuery::new())
        .await
        .unwrap();
    assert_eq!(
        hist.items[0].kind,
        hindsight_client::DocumentEventKind::Retain
    );
    let rep = bank.reprocess_document("doc-1").await.unwrap();
    assert_eq!(rep.operation_id, "op-r");
    bank.delete_document("doc-1").await.unwrap();
    let chunk = c.get_chunk("c1").await.unwrap();
    assert_eq!(chunk.text, "meeting notes");
}

#[tokio::test]
async fn entities_tags_and_observation_scopes() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/entities"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"id": "e1", "canonical_name": "Alice", "mention_count": 5}],
            "total": 1, "limit": 10, "offset": 0
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/entities/graph"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "nodes": [{"data": {"id": "e1", "label": "Alice"}}],
            "edges": [{"data": {"id": "x", "source": "e1", "target": "e2", "weight": 0.4}}]
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/entities/e1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "e1", "canonical_name": "Alice", "mention_count": 5,
            "observations": [{"text": "Works at Google", "mentioned_at": "2026-01-01"}]
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"tag": "user:alice", "count": 12}], "total": 1, "limit": 100, "offset": 0
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/observations/scopes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "scopes": [{"tags": [], "count": 3}], "total": 1, "limit": 100, "offset": 0
        })))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/b/observations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"deleted_count": 9})))
        .mount(&s)
        .await;

    let c = client(s.uri());
    let bank = c.bank("b");
    let entities = bank
        .list_entities(hindsight_client::ListEntitiesQuery::new().limit(10))
        .await
        .unwrap();
    assert_eq!(entities.items[0].canonical_name, "Alice");
    let graph = bank.entity_graph().await.unwrap();
    assert_eq!(graph.edges[0].data.weight, Some(0.4));
    let detail = bank.get_entity("e1").await.unwrap();
    assert_eq!(detail.observations[0].text, "Works at Google");
    let tags = bank.list_tags(ListTagsQuery::new()).await.unwrap();
    assert_eq!(tags.items[0].count, 12);
    let scopes = bank.list_observation_scopes(None, None).await.unwrap();
    assert_eq!(scopes.scopes[0].count, 3);
    bank.clear_all_observations().await.unwrap();
}

// ----------------------------------------------------------------------
// Operations
// ----------------------------------------------------------------------

#[tokio::test]
async fn operations_lifecycle_and_wait() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/operations/op-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "operation_id": "op-1", "status": "processing", "operation_type": "retain"
        })))
        .up_to_n_times(1)
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/operations/op-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "operation_id": "op-1", "status": "completed", "operation_type": "retain",
            "completed_at": "2026-01-01T00:00:00Z"
        })))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/b/operations/op-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "message": "cancelled", "operation_id": "op-1"
        })))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/operations/op-1/retry"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "message": "requeued", "operation_id": "op-1"
        })))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/operations/op-1/delete"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "message": "deleted", "operation_id": "op-1"
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/operations"))
        .and(query_param("status", "pending"))
        .and(query_param("type", "retain"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "bank_id": "b", "operations": [], "total": 0, "limit": 100, "offset": 0
        })))
        .mount(&s)
        .await;

    let c = client(s.uri());
    let bank = c.bank("b");
    let done = bank
        .wait_for_operation(
            "op-1",
            hindsight_client::OperationWait::new().poll_interval(Duration::from_millis(20)),
        )
        .await
        .unwrap();
    assert!(done.is_completed());
    let cancelled = bank.cancel_operation("op-1").await.unwrap();
    assert!(cancelled.success);
    bank.retry_operation("op-1").await.unwrap();
    bank.delete_operation("op-1").await.unwrap();
    bank.list_operations(
        hindsight_client::ListOperationsQuery::new()
            .status("pending")
            .task_type("retain"),
    )
    .await
    .unwrap();
}

// ----------------------------------------------------------------------
// Files & transfer
// ----------------------------------------------------------------------

#[tokio::test]
async fn retain_files_uploads_multipart() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/files/retain"))
        .and(body_string_contains(
            "name=\"files\"; filename=\"notes.pdf\"",
        ))
        .and(body_string_contains("name=\"request\""))
        .and(body_string_contains("files_metadata"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "operation_ids": ["op-f1", "op-f2"]
        })))
        .mount(&s)
        .await;
    let c = keyed_client(s.uri());
    let resp = c
        .bank("b")
        .retain_files(
            [
                UploadedFile::new(b"%PDF-1.4".to_vec(), "notes.pdf", "application/pdf"),
                UploadedFile::new(b"txt".to_vec(), "a.txt", "text/plain"),
            ],
            hindsight_client::FilesRetainRequest {
                files_metadata: Some(vec![
                    hindsight_client::FilesRetainMetadata {
                        context: Some("pdf".into()),
                        document_id: Some("d1".into()),
                        tags: Some(vec!["docs".into()]),
                        metadata: None,
                        observation_scopes: None,
                    },
                    hindsight_client::FilesRetainMetadata::default(),
                ]),
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.operation_ids.len(), 2);
}

#[tokio::test]
async fn retain_files_rejects_empty_upload() {
    let s = server().await;
    let c = client(s.uri());
    let err = c
        .bank("b")
        .retain_files([], Default::default())
        .await
        .unwrap_err();
    assert!(err.is_invalid_input());
}

#[tokio::test]
async fn transfer_export_import_clone_and_download() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/transfer/export"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"operation_id": "op-x", "status": "pending"})),
        )
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/transfer/import"))
        .and(query_param("mode", "merge"))
        .and(body_string_contains("name=\"file\""))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"operation_id": "op-i", "status": "pending"})),
        )
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/clone"))
        .and(body_json(
            json!({"target_bank_id": "b-clone", "name": "Copy"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"operation_id": "op-cl", "status": "pending"})),
        )
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/document-transfer/export"))
        .and(query_param("include_observations", "true"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"operation_id": "op-dx", "status": "pending"})),
        )
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/document-transfer"))
        .and(body_string_contains("name=\"file\""))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"operation_id": "op-di", "status": "pending"})),
        )
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/document-transfer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"transfers": []})))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/files/download/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ZIPDATA".to_vec()))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/attachments/att-1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"BYTES".to_vec()))
        .mount(&s)
        .await;

    let c = client(s.uri());
    let bank = c.bank("b");
    assert_eq!(bank.export_bank().await.unwrap().operation_id, "op-x");
    let imported = bank
        .import_bank(
            b"zip-bytes".to_vec(),
            hindsight_client::ImportBankOptions::new().mode("merge"),
        )
        .await
        .unwrap();
    assert_eq!(imported.operation_id, "op-i");
    assert_eq!(
        bank.clone_bank(hindsight_client::CloneBankRequest::new("b-clone").name("Copy"))
            .await
            .unwrap()
            .operation_id,
        "op-cl"
    );
    assert_eq!(
        bank.export_documents(
            hindsight_client::ExportDocumentsOptions::new().include_observations(true)
        )
        .await
        .unwrap()
        .operation_id,
        "op-dx"
    );
    assert_eq!(
        bank.import_documents(b"zip".to_vec(), Some("skip".into()))
            .await
            .unwrap()
            .operation_id,
        "op-di"
    );
    bank.list_document_transfers().await.unwrap();
    assert_eq!(
        c.download_file("abc123").await.unwrap(),
        b"ZIPDATA".to_vec()
    );
    assert_eq!(
        bank.get_attachment("att-1").await.unwrap(),
        b"BYTES".to_vec()
    );
}

#[tokio::test]
async fn templates_import_export_and_schema() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/import"))
        .and(query_param("dry_run", "true"))
        .and(body_json(json!({
            "version": "1",
            "bank": {"retain_mission": "capture facts"},
            "directives": [{"name": "tone", "content": "be brief", "priority": 0, "is_active": true}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "bank_id": "b", "config_applied": true,
            "directives_created": ["tone"], "dry_run": true
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/export"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "version": "1",
            "bank": {"retain_mission": "capture facts"},
            "mental_models": [],
            "directives": [{"name": "tone", "content": "be brief"}]
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/bank-template-schema"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"type": "object"})))
        .mount(&s)
        .await;

    let c = client(s.uri());
    let bank = c.bank("b");
    let manifest = hindsight_client::BankTemplateManifest {
        version: "1".into(),
        bank: Some(hindsight_client::BankTemplateConfig {
            retain_mission: Some("capture facts".into()),
            ..Default::default()
        }),
        mental_models: None,
        directives: Some(vec![hindsight_client::BankTemplateDirective {
            name: "tone".into(),
            content: "be brief".into(),
            priority: 0,
            is_active: true,
            tags: vec![],
        }]),
    };
    let imported = bank.import_bank_template(manifest, true).await.unwrap();
    assert!(imported.dry_run);
    assert_eq!(imported.directives_created, ["tone".to_string()]);
    let exported = bank.export_bank_template().await.unwrap();
    assert_eq!(exported.version, "1");
    c.bank_template_schema().await.unwrap();
}

// ----------------------------------------------------------------------
// Webhooks, audit, llm traces, prompts preview
// ----------------------------------------------------------------------

#[tokio::test]
async fn webhooks_flow() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/webhooks"))
        .and(body_json(json!({
            "url": "https://hooks.example/x", "secret": "s3cret",
            "event_types": ["consolidation.completed"], "enabled": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "wh-1", "bank_id": "b", "url": "https://hooks.example/x",
            "event_types": ["consolidation.completed"], "enabled": true
        })))
        .mount(&s)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/default/banks/b/webhooks/wh-1"))
        .and(body_json(json!({"enabled": false})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "wh-1", "bank_id": "b", "url": "https://hooks.example/x",
            "event_types": ["consolidation.completed"], "enabled": false
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/webhooks/wh-1/deliveries"))
        .and(query_param("cursor", "cur1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"id": "dl-1", "webhook_id": "wh-1", "url": "https://hooks.example/x",
                        "event_type": "consolidation.completed", "status": "delivered", "attempts": 1}],
            "total": 1, "limit": 50, "offset": 0
        })))
        .mount(&s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/default/banks/b/webhooks/wh-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
        .mount(&s)
        .await;

    let c = client(s.uri());
    let bank = c.bank("b");
    let wh = bank
        .register_webhook(
            CreateWebhookRequest::new("https://hooks.example/x")
                .secret("s3cret")
                .event_types(["consolidation.completed"]),
        )
        .await
        .unwrap();
    assert_eq!(wh.id, "wh-1");
    bank.update_webhook(
        "wh-1",
        hindsight_client::UpdateWebhookRequest::new().enabled(false),
    )
    .await
    .unwrap();
    let deliveries = bank
        .list_webhook_deliveries(
            "wh-1",
            hindsight_client::WebhookDeliveriesQuery::new().cursor("cur1"),
        )
        .await
        .unwrap();
    assert_eq!(deliveries.items[0].status, "delivered");
    bank.delete_webhook("wh-1").await.unwrap();
}

#[tokio::test]
async fn audit_logs_and_llm_traces() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/audit-logs"))
        .and(query_param("action", "recall"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"id": "a1", "action": "recall", "transport": "http", "metadata": {}}],
            "total": 1, "limit": 100, "offset": 0
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/audit-logs/stats"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"buckets": [], "total": 0})))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/audit-logs/a1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "a1", "action": "recall", "transport": "http", "bank_id": "b",
            "started_at": "2026-01-01", "ended_at": "2026-01-01",
            "metadata": {}, "request": null, "response": null
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/llm-requests"))
        .and(query_param("operation", "reflect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"id": "l1", "bank_id": "b", "operation": "reflect", "scope": "agent_1",
                        "provider": "openai", "model": "gpt-5-mini", "status": "ok",
                        "started_at": "2026-01-01", "ended_at": "2026-01-01",
                        "duration_ms": 100, "input_tokens": 10, "output_tokens": 5,
                        "total_tokens": 15, "cached_tokens": 0, "thoughts_tokens": 0}],
            "total": 1, "limit": 100, "offset": 0
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/default/banks/b/llm-requests/stats"))
        .and(query_param("period", "day"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "bank_id": "b", "period": "day", "buckets": []
        })))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/prompts/preview"))
        .and(body_json(json!({"operation": "retain"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "messages": [{"role": "system", "blocks": [{"text": "You are a memory extractor.",
                                                        "source": "builtin", "active": true, "editable": false}]}],
            "strategies": ["default"]
        })))
        .mount(&s)
        .await;

    let c = client(s.uri());
    let bank = c.bank("b");
    let logs = bank
        .list_audit_logs(hindsight_client::ListAuditLogsQuery::new().action("recall"))
        .await
        .unwrap();
    assert_eq!(logs.items[0].action, "recall");
    bank.audit_log_stats().await.unwrap();
    let entry = bank.get_audit_log("a1").await.unwrap();
    assert_eq!(entry.bank_id, "b");
    let traces = bank
        .list_llm_requests(hindsight_client::ListLlmRequestsQuery::new().operation("reflect"))
        .await
        .unwrap();
    assert_eq!(traces.items[0].total_tokens, 15);
    bank.llm_request_stats(hindsight_client::LlmStatsQuery::new().period("day"))
        .await
        .unwrap();
    let preview = bank
        .preview_prompts(hindsight_client::PromptPreviewRequest::retain())
        .await
        .unwrap();
    assert_eq!(
        preview.messages[0].blocks[0].text,
        "You are a memory extractor."
    );
}

// ----------------------------------------------------------------------
// Monitoring
// ----------------------------------------------------------------------

#[tokio::test]
async fn monitoring_endpoints() {
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "ok"})))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/health/ready"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/health/live"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "alive", "version": "0.9.0", "uptime_seconds": 42.5
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "api_version": "0.9.0",
            "features": {"observations": true, "mcp": true, "worker": false}
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/metrics"))
        .respond_with(ResponseTemplate::new(200).set_body_string("# HELP hindsight_up 1\n"))
        .mount(&s)
        .await;

    let c = client(s.uri());
    c.health().await.unwrap();
    c.readiness().await.unwrap();
    let live = c.liveness().await.unwrap();
    assert_eq!(live.version, "0.9.0");
    let v = c.version().await.unwrap();
    assert_eq!(v.api_version, "0.9.0");
    assert_eq!(v.features["observations"], json!(true));
    let m = c.metrics().await.unwrap();
    assert!(m.starts_with("# HELP"));
}
