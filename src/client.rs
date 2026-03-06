use reqwest::{Client, IntoUrl};
use serde_json::json;
use std::time::Duration;
use url::Url;

use crate::error::{Error, Result};
use crate::models::{MemoryHit, RetainRequest};

/// Client for Hindsight HTTP API.
#[derive(Debug, Clone)]
pub struct HindsightClient {
    http: Client,
    base_url: Url,
}

impl HindsightClient {
    /// Create a new client pointing to a Hindsight server.
    pub fn new(base_url: impl IntoUrl) -> Result<Self> {
        let base_url = base_url.into_url()?;
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { http, base_url })
    }

    /// Store a memory in a bank.
    pub async fn retain(
        &self,
        bank_id: &str,
        content: &str,
        context: Option<&str>,
        timestamp: Option<&str>,
    ) -> Result<()> {
        let url = self.base_url
            .join(&format!("/api/v1/banks/{}/retain", bank_id))?;

        let req = RetainRequest {
            content: content.to_string(),
            context: context.map(String::from),
            timestamp: timestamp.map(String::from),
        };

        let resp = self.http.post(url).json(&req).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::ApiError(status, text));
        }
        Ok(())
    }

    /// Retrieve memories relevant to a query.
    pub async fn recall(
        &self,
        bank_id: &str,
        query: &str,
        top_k: Option<usize>,
    ) -> Result<Vec<MemoryHit>> {
        let url = self.base_url
            .join(&format!("/api/v1/banks/{}/recall", bank_id))?;

        let mut body = json!({ "query": query });
        if let Some(k) = top_k {
            body["top_k"] = json!(k);
        }

        let resp = self.http.post(url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::ApiError(status, text));
        }

        #[derive(serde::Deserialize)]
        struct RecallResponse {
            hits: Vec<MemoryHit>,
        }
        let res: RecallResponse = resp.json().await?;
        Ok(res.hits)
    }

    /// Perform a reflection (deep analysis) on a query.
    pub async fn reflect(
        &self,
        bank_id: &str,
        query: &str,
    ) -> Result<String> {
        let url = self.base_url
            .join(&format!("/api/v1/banks/{}/reflect", bank_id))?;

        let body = json!({ "query": query });
        let resp = self.http.post(url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::ApiError(status, text));
        }

        let text = resp.text().await?;
        Ok(text)
    }
}
