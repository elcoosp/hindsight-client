use hindsight_client::HindsightClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_retain() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/banks/test-bank/retain"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = HindsightClient::new(mock_server.uri()).unwrap();
    client.retain("test-bank", "content", None, None).await.unwrap();
}

#[tokio::test]
async fn test_recall() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "hits": [
            { "content": "memory1", "score": 0.9 },
            { "content": "memory2", "score": 0.8 }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/api/v1/banks/test-bank/recall"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = HindsightClient::new(mock_server.uri()).unwrap();
    let hits = client.recall("test-bank", "query", Some(5)).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].content, "memory1");
    assert_eq!(hits[0].score, 0.9);
}

#[tokio::test]
async fn test_reflect() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/banks/test-bank/reflect"))
        .respond_with(ResponseTemplate::new(200).set_body_string("reflection result"))
        .mount(&mock_server)
        .await;

    let client = HindsightClient::new(mock_server.uri()).unwrap();
    let result = client.reflect("test-bank", "query").await.unwrap();
    assert_eq!(result, "reflection result");
}
