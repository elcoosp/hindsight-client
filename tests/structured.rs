//! Structured-output reflect tests (feature `schemars`).
#![cfg(feature = "schemars")]

use hindsight_client::{Client, ReflectRequest};
use serde::{Deserialize, Serialize};
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[allow(dead_code)]
struct Answer {
    /// One-line summary.
    summary: String,
    /// Key points extracted from memory.
    key_points: Vec<String>,
    /// Confidence between 0 and 1.
    confidence: f64,
}

#[tokio::test]
async fn reflect_structured_sends_schema_and_deserializes() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/reflect"))
        .and(body_string_contains("response_schema"))
        .and(body_string_contains("summary"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "Alice works at Google.",
            "structured_output": {
                "summary": "Alice works at Google",
                "key_points": ["Alice is a software engineer"],
                "confidence": 0.92
            },
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        })))
        .mount(&s)
        .await;
    let c = Client::builder().base_url(s.uri()).build().unwrap();
    let answer: Answer = c
        .bank("b")
        .reflect_structured(ReflectRequest::new("where does alice work?"))
        .await
        .unwrap();
    assert_eq!(answer.summary, "Alice works at Google");
    assert_eq!(answer.key_points, ["Alice is a software engineer"]);
    assert!((answer.confidence - 0.92).abs() < 1e-9);
}

#[tokio::test]
async fn reflect_structured_maps_server_error() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/reflect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "attempted",
            "structured_output": null,
            "structured_output_error": "model refused schema"
        })))
        .mount(&s)
        .await;
    let c = Client::builder().base_url(s.uri()).build().unwrap();
    let err = c
        .bank("b")
        .reflect_structured::<Answer>(ReflectRequest::new("q"))
        .await
        .unwrap_err();
    match err {
        hindsight_client::Error::StructuredOutput { message, value } => {
            assert_eq!(message, "model refused schema");
            assert_eq!(value, None);
        }
        other => panic!("expected StructuredOutput, got {other:?}"),
    }
}

#[tokio::test]
async fn reflect_structured_maps_deserialize_failure() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/default/banks/b/reflect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "attempted",
            "structured_output": {"summary": 42}
        })))
        .mount(&s)
        .await;
    let c = Client::builder().base_url(s.uri()).build().unwrap();
    let err = c
        .bank("b")
        .reflect_structured::<Answer>(ReflectRequest::new("q"))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        hindsight_client::Error::StructuredOutput { .. }
    ));
}
