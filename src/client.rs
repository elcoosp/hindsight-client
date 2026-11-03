//! The HTTP client: [`Client`], [`ClientBuilder`] and the bank-scoped [`BankClient`].

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};
use crate::models::*;
use crate::pagination::Pager;
use crate::retry::{RetryPolicy, Scope};

pub(crate) const DEFAULT_USER_AGENT: &str =
    concat!("hindsight-client-rs/", env!("CARGO_PKG_VERSION"));
pub(crate) const DEFAULT_PATH_PREFIX: &str = "v1/default";

/// Environment variable holding the Hindsight server URL (self-hosted or cloud).
pub const ENV_API_URL: &str = "HINDSIGHT_API_URL";
/// Environment variable holding the Hindsight API key (`hsk_…`).
pub const ENV_API_KEY: &str = "HINDSIGHT_API_KEY";

#[derive(Debug)]
struct Inner {
    http: reqwest::Client,
    /// Origin URL (scheme + host + port).
    origin: Url,
    /// Origin + path prefix (e.g. `https://host/v1/default`), no trailing slash.
    base: Url,
    api_key: Option<String>,
    retry: RetryPolicy,
    timeout: Duration,
}

/// Decode a successful response body as JSON.
pub(crate) async fn decode<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
    let bytes = resp.bytes().await.map_err(Error::Transport)?;
    serde_json::from_slice(&bytes).map_err(|e| {
        Error::Json(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} (body: {:?})", e, String::from_utf8_lossy(&bytes)),
        )))
    })
}

/// An async client for the Hindsight agent memory API.
///
/// Cheap to clone (internally `Arc`-shared); one pooled HTTP/2 connection
/// pool is reused for every request.
///
/// ```no_run
/// use hindsight_client::Client;
///
/// # async fn demo() -> hindsight_client::Result<()> {
/// let client = Client::builder()
///     .base_url("http://localhost:8888")
///     .api_key("hsk_…")          // or Client::from_env()
///     .build()?;
///
/// let bank = client.bank("my-bank");
/// bank.retain(["Alice works at Google"]).await?;
/// let hits = bank.recall("Where does Alice work?").await?;
/// println!("{}", hits.to_context_string());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl std::fmt::Display for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hindsight client → {}", self.inner.base)
    }
}

/// Builder for [`Client`].
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    base_url: Option<Url>,
    path_prefix: String,
    api_key: Option<String>,
    timeout: Duration,
    user_agent: String,
    retry: RetryPolicy,
    pool_max_idle_per_host: usize,
    connect_timeout: Option<Duration>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            base_url: None,
            path_prefix: DEFAULT_PATH_PREFIX.to_string(),
            api_key: None,
            timeout: Duration::from_secs(120),
            user_agent: DEFAULT_USER_AGENT.to_string(),
            retry: RetryPolicy::new(),
            pool_max_idle_per_host: 32,
            connect_timeout: Some(Duration::from_secs(10)),
        }
    }
}

impl Client {
    /// Start building a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Build a client from `HINDSIGHT_API_URL` and `HINDSIGHT_API_KEY`.
    ///
    /// # Errors
    /// Returns [`Error::InvalidInput`] when the variables are unset or the
    /// URL is unparsable.
    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var(ENV_API_URL).map_err(|_| {
            Error::InvalidInput(format!(
                "missing environment variable {ENV_API_URL} (e.g. http://localhost:8888)"
            ))
        })?;
        let mut b = Self::builder().base_url(base_url);
        if let Ok(key) = std::env::var(ENV_API_KEY) {
            b = b.api_key(key);
        }
        b.build()
    }

    /// Scope this client to one bank — the primary way to use it.
    ///
    /// Bank ids are percent-encoded automatically.
    pub fn bank(&self, bank_id: impl Into<String>) -> BankClient<'_> {
        BankClient {
            client: self,
            bank_id: bank_id.into(),
        }
    }

    // ------------------------------------------------------------------
    // Plumbing
    // ------------------------------------------------------------------

    pub(crate) fn url_under_prefix(&self, segments: &[&str]) -> Result<Url> {
        push_segments(&self.inner.base, segments)
    }

    pub(crate) fn url_root(&self, segments: &[&str]) -> Result<Url> {
        push_segments(&self.inner.origin, segments)
    }

    fn request(&self, method: reqwest::Method, url: Url) -> reqwest::RequestBuilder {
        let mut rb = self
            .inner
            .http
            .request(method, url)
            .timeout(self.inner.timeout);
        if let Some(key) = &self.inner.api_key {
            rb = rb.bearer_auth(key);
        }
        rb
    }

    fn retryable(&self, method: &reqwest::Method) -> bool {
        self.inner.retry.applies_to(method)
    }

    async fn execute(
        &self,
        method: reqwest::Method,
        url: Url,
        body: Option<serde_json::Value>,
        timeout_override: Option<Duration>,
    ) -> Result<reqwest::Response> {
        let max_retries = if self.retryable(&method) {
            self.inner.retry.max_retries
        } else {
            0
        };
        let mut attempts = 0u32;
        loop {
            let mut rb = self.request(method.clone(), url.clone());
            if let Some(t) = timeout_override {
                rb = rb.timeout(t);
            }
            if let Some(json) = &body {
                rb = rb.json(json);
            }
            let result = rb.send().await;
            attempts += 1;

            let response = match result {
                Ok(r) => r,
                Err(e) if attempts <= max_retries && is_retryable_transport(&e) => {
                    let delay = self.inner.retry.delay_for(attempts - 1, None);
                    tracing::debug!(attempt = attempts, ?delay, error = %e, "retrying transport error");
                    tokio::time::sleep(delay).await;
                    continue;
                }
                Err(e) if e.is_timeout() => return Err(Error::Timeout),
                Err(e) => return Err(Error::Transport(e)),
            };

            let status = response.status();
            if status.is_success() {
                return Ok(response);
            }

            let retry_after = parse_retry_after(response.headers());
            let body_text = response.text().await.unwrap_or_default();

            if attempts <= max_retries && crate::retry::RetryPolicy::is_retryable_status(status) {
                let delay = self.inner.retry.delay_for(attempts - 1, retry_after);
                tracing::debug!(attempt = attempts, ?delay, %status, "retrying after status");
                tokio::time::sleep(delay).await;
                continue;
            }

            let mut err = Error::from_response(status, &body_text);
            if let Error::RateLimited { retry_after: ra } = &mut err {
                *ra = retry_after;
            }
            return Err(err);
        }
    }

    async fn execute_json<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        url: Url,
        body: Option<serde_json::Value>,
        timeout_override: Option<Duration>,
    ) -> Result<T> {
        let resp = self.execute(method, url, body, timeout_override).await?;
        decode(resp).await
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        url: Url,
        query: Option<&impl Serialize>,
    ) -> Result<T> {
        let url = attach_query(url, query)?;
        self.execute_json(reqwest::Method::GET, url, None, None)
            .await
    }

    pub(crate) async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        url: Url,
        body: &B,
        timeout: Option<Duration>,
    ) -> Result<T> {
        let json = serde_json::to_value(body).map_err(Error::Json)?;
        self.execute_json(reqwest::Method::POST, url, Some(json), timeout)
            .await
    }

    pub(crate) async fn put_json<B: Serialize, T: DeserializeOwned>(
        &self,
        url: Url,
        body: &B,
    ) -> Result<T> {
        let json = serde_json::to_value(body).map_err(Error::Json)?;
        self.execute_json(reqwest::Method::PUT, url, Some(json), None)
            .await
    }

    pub(crate) async fn patch_json<B: Serialize, T: DeserializeOwned>(
        &self,
        url: Url,
        body: &B,
    ) -> Result<T> {
        let json = serde_json::to_value(body).map_err(Error::Json)?;
        self.execute_json(reqwest::Method::PATCH, url, Some(json), None)
            .await
    }

    // ------------------------------------------------------------------
    // Monitoring
    // ------------------------------------------------------------------

    /// Readiness check — verifies the API can reach the database.
    /// Fails with [`Error::Api`] (503) when traffic must not be routed here.
    pub async fn health(&self) -> Result<serde_json::Value> {
        self.execute_json(
            reqwest::Method::GET,
            self.url_root(&["health"])?,
            None,
            None,
        )
        .await
    }

    /// Readiness probe (`/health/ready`): 200 when the database is reachable.
    /// Unlike [`Client::health`] this ignores the response body.
    pub async fn readiness(&self) -> Result<()> {
        self.execute(
            reqwest::Method::GET,
            self.url_root(&["health", "ready"])?,
            None,
            None,
        )
        .await?
        .error_for_status()
        .map_err(|e| Error::from_response(e.status().unwrap_or_default(), &e.to_string()))?;
        Ok(())
    }

    /// Liveness probe (`/health/live`): process up, no database access.
    pub async fn liveness(&self) -> Result<LivenessResponse> {
        self.execute_json(
            reqwest::Method::GET,
            self.url_root(&["health", "live"])?,
            None,
            None,
        )
        .await
    }

    /// API version + enabled feature flags — gate new capabilities on it.
    pub async fn version(&self) -> Result<VersionResponse> {
        self.execute_json(
            reqwest::Method::GET,
            self.url_root(&["version"])?,
            None,
            None,
        )
        .await
    }

    /// Prometheus metrics (text exposition format).
    pub async fn metrics(&self) -> Result<String> {
        let resp = self
            .execute(
                reqwest::Method::GET,
                self.url_root(&["metrics"])?,
                None,
                None,
            )
            .await?;
        Ok(resp.text().await?)
    }

    // ------------------------------------------------------------------
    // Banks
    // ------------------------------------------------------------------

    /// List banks, optionally searching ids/aliases/names with `q`.
    pub async fn list_banks(&self, query: ListBanksQuery) -> Result<BankListResponse> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            banks: Vec<BankListItem>,
            #[serde(default)]
            total: i64,
            #[serde(default)]
            limit: i64,
            #[serde(default)]
            offset: i64,
        }
        let raw: Raw = self
            .get_json(self.url_under_prefix(&["banks"])?, Some(&query))
            .await?;
        Ok(Page {
            items: raw.banks,
            total: raw.total,
            limit: raw.limit,
            offset: raw.offset,
        })
    }

    /// Auto-paging variant of [`Client::list_banks`].
    pub fn list_banks_paged(&self, page_size: i64) -> Pager<BankListItem> {
        let client = self.clone();
        Pager::new(move |offset| {
            let client = client.clone();
            async move {
                client
                    .list_banks(ListBanksQuery::new().limit(page_size).offset(offset))
                    .await
            }
        })
    }

    /// Create the bank if missing, or fully update it (PUT semantics).
    pub async fn create_or_update_bank(
        &self,
        bank_id: impl Into<String>,
        settings: BankSettings,
    ) -> Result<BankWriteResponse> {
        let id = validate_bank_id(bank_id)?;
        self.put_json(self.url_under_prefix(&["banks", &id])?, &settings)
            .await
    }

    /// Partially update a bank (PATCH semantics).
    pub async fn update_bank(
        &self,
        bank_id: impl Into<String>,
        settings: BankSettings,
    ) -> Result<BankWriteResponse> {
        let id = validate_bank_id(bank_id)?;
        self.patch_json(self.url_under_prefix(&["banks", &id])?, &settings)
            .await
    }

    /// Delete a bank and all its memories. Irreversible.
    pub async fn delete_bank(&self, bank_id: impl Into<String>) -> Result<serde_json::Value> {
        let id = validate_bank_id(bank_id)?;
        self.execute_json(
            reqwest::Method::DELETE,
            self.url_under_prefix(&["banks", &id])?,
            None,
            None,
        )
        .await
    }

    /// Fetch a stored chunk by id (`GET /v1/default/chunks/{chunk_id}`).
    pub async fn get_chunk(&self, chunk_id: impl Into<String>) -> Result<ChunkResponse> {
        let id = validate_segment("chunk_id", &chunk_id.into())?;
        self.get_json(
            self.url_under_prefix(&["chunks", &id])?,
            Option::<&()>::None,
        )
        .await
    }

    /// Download a stored file (upload attachment or export archive) by key.
    pub async fn download_file(&self, key: impl Into<String>) -> Result<Vec<u8>> {
        let key = validate_segment("key", &key.into())?;
        let resp = self
            .execute(
                reqwest::Method::GET,
                self.url_under_prefix(&["files", "download", &key])?,
                None,
                None,
            )
            .await?;
        Ok(resp.bytes().await?.to_vec())
    }

    /// JSON Schema for bank template manifests (`GET /v1/bank-template-schema`).
    pub async fn bank_template_schema(&self) -> Result<serde_json::Value> {
        self.get_json(
            self.url_root(&["v1", "bank-template-schema"])?,
            Option::<&()>::None,
        )
        .await
    }

    // ------------------------------------------------------------------
    // Operations (bank-scoped via Client for convenience)
    // ------------------------------------------------------------------

    /// Poll an async operation until it reaches a terminal state.
    pub async fn wait_for_operation(
        &self,
        bank_id: impl Into<String>,
        operation_id: impl Into<String>,
        wait: OperationWait,
    ) -> Result<OperationStatusResponse> {
        self.bank(bank_id)
            .wait_for_operation(operation_id, wait)
            .await
    }
}

/// Tuning for [`Client::wait_for_operation`].
#[derive(Debug, Clone, Copy)]
pub struct OperationWait {
    /// Delay between polls (default 1 s).
    pub poll_interval: Duration,
    /// Give up after this long (default 10 min).
    pub timeout: Duration,
}

impl Default for OperationWait {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(1),
            timeout: Duration::from_secs(600),
        }
    }
}

impl OperationWait {
    /// Default polling policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the poll interval.
    #[must_use]
    pub fn poll_interval(mut self, d: Duration) -> Self {
        self.poll_interval = d;
        self
    }

    /// Set the overall timeout.
    #[must_use]
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }
}

// ----------------------------------------------------------------------
// BankClient
// ----------------------------------------------------------------------

/// All bank-scoped operations, with `bank_id` bound once.
///
/// Obtain via [`Client::bank`].
#[derive(Debug, Clone)]
pub struct BankClient<'a> {
    client: &'a Client,
    bank_id: String,
}

impl<'a> BankClient<'a> {
    /// The bank id this handle is scoped to.
    pub fn bank_id(&self) -> &str {
        &self.bank_id
    }

    /// The shared [`Client`] behind this handle.
    pub fn client(&self) -> &'a Client {
        self.client
    }

    fn url(&self, segments: &[&str]) -> Result<Url> {
        validate_segment("bank_id", &self.bank_id)?;
        let mut all: Vec<&str> = Vec::with_capacity(segments.len() + 2);
        all.push("banks");
        all.push(&self.bank_id);
        all.extend_from_slice(segments);
        self.client.url_under_prefix(&all)
    }

    // ------------------------------------------------------------------
    // Retain
    // ------------------------------------------------------------------

    /// Retain memories (synchronous — waits for extraction and storage).
    ///
    /// ```no_run
    /// # use hindsight_client::{Client, MemoryItem};
    /// # async fn demo(client: &Client) -> hindsight_client::Result<()> {
    /// let bank = client.bank("user-123");
    /// bank.retain(["Alice works at Google"]).await?;
    /// bank.retain([
    ///     MemoryItem::new("Prefers dark mode").context("ui settings"),
    /// ]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn retain(
        &self,
        items: impl IntoIterator<Item: Into<MemoryItem>>,
    ) -> Result<RetainResponse> {
        self.retain_with(RetainRequest::new(items)).await
    }

    /// Retain with full control over the request (async mode, document tags, …).
    pub async fn retain_with(&self, request: RetainRequest) -> Result<RetainResponse> {
        if request.items.is_empty() {
            return Err(Error::InvalidInput(
                "retain requires at least one item".into(),
            ));
        }
        self.client
            .post_json(self.url(&["memories"])?, &request, None)
            .await
    }

    /// Retain asynchronously: returns immediately with an operation id to poll.
    pub async fn retain_async(
        &self,
        items: impl IntoIterator<Item: Into<MemoryItem>>,
    ) -> Result<RetainResponse> {
        self.retain_with(RetainRequest::new(items).as_async()).await
    }

    /// Retain files (PDF, images, …): multipart upload, always async.
    pub async fn retain_files(
        &self,
        files: impl IntoIterator<Item = UploadedFile>,
        metadata: FilesRetainRequest,
    ) -> Result<FileRetainResponse> {
        let files: Vec<UploadedFile> = files.into_iter().collect();
        if files.is_empty() {
            return Err(Error::InvalidInput(
                "retain_files requires at least one file".into(),
            ));
        }
        let mut form = reqwest::multipart::Form::new();
        for f in files {
            let part = reqwest::multipart::Part::bytes(f.bytes)
                .file_name(f.filename.clone())
                .mime_str(&f.mime_type)
                .map_err(|e| Error::InvalidInput(format!("invalid mime type: {e}")))?;
            form = form.part("files", part);
        }
        let request_json = serde_json::to_string(&metadata).map_err(Error::Json)?;
        form = form.text("request", request_json);

        let url = self.url(&["files", "retain"])?;
        let resp = self
            .client
            .request(reqwest::Method::POST, url)
            .multipart(form)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::from_response(status, &body));
        }
        decode(resp).await
    }

    /// Preview fact extraction without persisting anything.
    pub async fn dry_run_extract(
        &self,
        request: DryRunExtractRequest,
    ) -> Result<DryRunExtractionResult> {
        self.client
            .post_json(self.url(&["memories", "dry-run-extract"])?, &request, None)
            .await
    }

    // ------------------------------------------------------------------
    // Recall & Reflect
    // ------------------------------------------------------------------

    /// Recall memories relevant to the query.
    ///
    /// Accepts a plain query string or a [`RecallRequest`] for full control.
    pub async fn recall(&self, query: impl Into<RecallRequest>) -> Result<RecallResponse> {
        self.recall_with(query.into()).await
    }

    /// Recall with a pre-built request.
    pub async fn recall_with(&self, request: RecallRequest) -> Result<RecallResponse> {
        self.client
            .post_json(self.url(&["memories", "recall"])?, &request, None)
            .await
    }

    /// Recall and format the results as an LLM-ready context string
    /// (grouped by fact type, entities and timestamps inline).
    pub async fn recall_text(&self, query: impl Into<RecallRequest>) -> Result<String> {
        Ok(self.recall(query).await?.to_context_string())
    }

    /// Reflect: agentic multi-round reasoning grounded in the bank.
    pub async fn reflect(&self, query: impl Into<ReflectRequest>) -> Result<ReflectResponse> {
        self.reflect_with(query.into()).await
    }

    /// Reflect with a pre-built request.
    pub async fn reflect_with(&self, request: ReflectRequest) -> Result<ReflectResponse> {
        // The agentic loop can take minutes; give it head-room over the
        // default timeout unless the caller chose one.
        self.client
            .post_json(
                self.url(&["reflect"])?,
                &request,
                Some(self.client.inner.timeout.max(Duration::from_secs(300))),
            )
            .await
    }

    /// The answer text only.
    pub async fn reflect_text(&self, query: impl Into<ReflectRequest>) -> Result<String> {
        Ok(self.reflect(query).await?.text)
    }

    /// Reflect with typed structured output.
    ///
    /// The JSON schema of `T` is derived via `schemars` and sent as
    /// `response_schema`; the server-validated `structured_output` is
    /// deserialized into `T`.
    #[cfg(feature = "schemars")]
    pub async fn reflect_structured<T>(&self, query: impl Into<ReflectRequest>) -> Result<T>
    where
        T: schemars::JsonSchema + DeserializeOwned,
    {
        let mut request = query.into();
        let schema = schemars::schema_for!(T);
        request.response_schema = Some(serde_json::to_value(schema).map_err(Error::Json)?);
        let resp = self.reflect_with(request).await?;
        if let Some(msg) = resp.structured_output_error {
            return Err(Error::StructuredOutput {
                message: msg,
                value: resp.structured_output,
            });
        }
        match resp.structured_output {
            Some(v) => serde_json::from_value(v).map_err(|e| Error::StructuredOutput {
                message: format!(
                    "structured_output did not deserialize into {}: {e}",
                    std::any::type_name::<T>()
                ),
                value: None,
            }),
            None => Err(Error::StructuredOutput {
                message: "server returned no structured_output".into(),
                value: None,
            }),
        }
    }

    // ------------------------------------------------------------------
    // Memories: list / curate / history / clear / graph / tags
    // ------------------------------------------------------------------

    /// List memory units with filters and pagination.
    pub async fn list_memories(&self, query: ListMemoriesQuery) -> Result<MemoryUnitPage> {
        self.client
            .get_json(self.url(&["memories", "list"])?, Some(&query))
            .await
    }

    /// Auto-paging variant of [`BankClient::list_memories`].
    pub fn list_memories_paged(
        &self,
        query: ListMemoriesQuery,
        page_size: i64,
    ) -> Pager<MemoryUnitListItem> {
        let client = self.client.clone();
        let bank_id = self.bank_id.clone();
        Pager::new(move |offset| {
            let client = client.clone();
            let bank_id = bank_id.clone();
            let mut q = query.clone();
            q.limit = Some(page_size);
            q.offset = Some(offset);
            async move { client.bank(&bank_id).list_memories(q).await }
        })
    }

    /// Get one memory unit (typed view; unknown fields preserved in `extra`-style maps).
    pub async fn get_memory(&self, memory_id: impl Into<String>) -> Result<MemoryUnitDetail> {
        let id = validate_segment("memory_id", &memory_id.into())?;
        self.client
            .get_json(self.url(&["memories", &id])?, Option::<&()>::None)
            .await
    }

    /// Edit or invalidate a memory (curation).
    pub async fn curate_memory(
        &self,
        memory_id: impl Into<String>,
        update: UpdateMemoryRequest,
    ) -> Result<serde_json::Value> {
        let id = validate_segment("memory_id", &memory_id.into())?;
        self.client
            .patch_json(self.url(&["memories", &id])?, &update)
            .await
    }

    /// Full change history of an observation, sources resolved to text.
    pub async fn memory_history(
        &self,
        memory_id: impl Into<String>,
    ) -> Result<Vec<ObservationHistoryEntry>> {
        let id = validate_segment("memory_id", &memory_id.into())?;
        self.client
            .get_json(
                self.url(&["memories", &id, "history"])?,
                Option::<&()>::None,
            )
            .await
    }

    /// Clear every observation derived from one memory.
    pub async fn clear_memory_observations(
        &self,
        memory_id: impl Into<String>,
    ) -> Result<ClearMemoryObservationsResponse> {
        let id = validate_segment("memory_id", &memory_id.into())?;
        self.client
            .execute_json(
                reqwest::Method::DELETE,
                self.url(&["memories", &id, "observations"])?,
                None,
                None,
            )
            .await
    }

    /// Delete all memories in the bank (optionally only one fact type).
    pub async fn clear_memories(&self, fact_type: Option<FactType>) -> Result<DeleteResponse> {
        let mut url = self.url(&["memories"])?;
        if let Some(ft) = fact_type {
            url.query_pairs_mut().append_pair("type", &ft.to_string());
        }
        self.client
            .execute_json(reqwest::Method::DELETE, url, None, None)
            .await
    }

    /// Memory graph for visualization.
    pub async fn graph(&self, query: GraphQuery) -> Result<GraphDataResponse> {
        self.client
            .get_json(self.url(&["graph"])?, Some(&query))
            .await
    }

    /// List tags used in the bank.
    pub async fn list_tags(&self, query: ListTagsQuery) -> Result<TagPage> {
        self.client
            .get_json(self.url(&["tags"])?, Some(&query))
            .await
    }

    /// Observation scopes present in the bank.
    pub async fn list_observation_scopes(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<ObservationScopesResponse> {
        #[derive(Serialize)]
        struct Q {
            #[serde(skip_serializing_if = "Option::is_none")]
            limit: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            offset: Option<i64>,
        }
        self.client
            .get_json(
                self.url(&["observations", "scopes"])?,
                Some(&Q { limit, offset }),
            )
            .await
    }

    /// Clear all observations in the bank (they will be re-consolidated).
    pub async fn clear_all_observations(&self) -> Result<serde_json::Value> {
        self.client
            .execute_json(
                reqwest::Method::DELETE,
                self.url(&["observations"])?,
                None,
                None,
            )
            .await
    }

    /// Preview what consolidation strategies would claim, without running.
    pub async fn preview_consolidation_strategies(
        &self,
        strategies: Vec<serde_json::Value>,
    ) -> Result<ConsolidationStrategiesPreview> {
        #[derive(Serialize)]
        struct Req {
            strategies: Vec<serde_json::Value>,
        }
        self.client
            .post_json(
                self.url(&["consolidation-strategies", "preview"])?,
                &Req { strategies },
                None,
            )
            .await
    }

    // ------------------------------------------------------------------
    // Bank management (config, aliases, stats, background, consolidation)
    // ------------------------------------------------------------------

    /// Fully-resolved bank configuration plus the bank-specific overrides.
    pub async fn get_bank_config(&self) -> Result<BankConfigResponse> {
        self.client
            .get_json(self.url(&["config"])?, Option::<&()>::None)
            .await
    }

    /// Update bank configuration overrides (Python-style field names accepted).
    pub async fn update_bank_config(&self, update: BankConfigUpdate) -> Result<BankConfigResponse> {
        self.client
            .patch_json(self.url(&["config"])?, &update)
            .await
    }

    /// Reset bank configuration to defaults.
    pub async fn reset_bank_config(&self) -> Result<BankConfigResponse> {
        self.client
            .execute_json(reqwest::Method::DELETE, self.url(&["config"])?, None, None)
            .await
    }

    /// Bank statistics (counts by type, pending operations, consolidation state).
    pub async fn get_bank_stats(&self) -> Result<BankStatsResponse> {
        self.client
            .get_json(self.url(&["stats"])?, Option::<&()>::None)
            .await
    }

    /// Memory ingestion timeseries.
    pub async fn memories_timeseries(
        &self,
        query: MemoriesTimeseriesQuery,
    ) -> Result<MemoriesTimeseriesResponse> {
        self.client
            .get_json(self.url(&["stats", "memories-timeseries"])?, Some(&query))
            .await
    }

    /// LLM connectivity health per operation.
    pub async fn bank_llm_health(&self) -> Result<BankLlmHealthResponse> {
        self.client
            .get_json(self.url(&["health", "llm"])?, Option::<&()>::None)
            .await
    }

    /// Bank profile (mission/background/disposition).
    pub async fn get_bank_profile(&self) -> Result<BankProfileResponse> {
        self.client
            .get_json(self.url(&["profile"])?, Option::<&()>::None)
            .await
    }

    /// Set/replace the bank's background knowledge.
    pub async fn set_bank_background(
        &self,
        content: impl Into<String>,
    ) -> Result<BackgroundResponse> {
        #[derive(Serialize)]
        struct Req {
            content: String,
        }
        self.client
            .post_json(
                self.url(&["background"])?,
                &Req {
                    content: content.into(),
                },
                None,
            )
            .await
    }

    /// Trigger consolidation; returns the async operation id.
    pub async fn consolidate(
        &self,
        request: ConsolidationRequest,
    ) -> Result<ConsolidationResponse> {
        self.client
            .post_json(self.url(&["consolidate"])?, &request, None)
            .await
    }

    /// Requeue permanently failed consolidations.
    pub async fn recover_consolidation(&self) -> Result<RecoverConsolidationResponse> {
        self.client
            .post_json(
                self.url(&["consolidation", "recover"])?,
                &serde_json::Value::Null,
                None,
            )
            .await
    }

    /// List the bank's aliases.
    pub async fn list_aliases(&self) -> Result<BankAliasesResponse> {
        self.client
            .get_json(self.url(&["aliases"])?, Option::<&()>::None)
            .await
    }

    /// Add an alias (optionally making it the displayed one).
    pub async fn add_alias(
        &self,
        alias: impl Into<String>,
        primary: bool,
    ) -> Result<serde_json::Value> {
        #[derive(Serialize)]
        struct Req {
            alias: String,
            #[serde(skip_serializing_if = "std::ops::Not::not")]
            primary: bool,
        }
        self.client
            .post_json(
                self.url(&["aliases"])?,
                &Req {
                    alias: alias.into(),
                    primary,
                },
                None,
            )
            .await
    }

    /// Show (or stop showing) this alias in place of the bank id.
    pub async fn set_primary_alias(
        &self,
        alias: impl Into<String>,
        primary: bool,
    ) -> Result<serde_json::Value> {
        #[derive(Serialize)]
        struct Req {
            primary: bool,
        }
        let a = validate_segment("alias", &alias.into())?;
        self.client
            .patch_json(self.url(&["aliases", &a])?, &Req { primary })
            .await
    }

    /// Remove an alias.
    pub async fn remove_alias(&self, alias: impl Into<String>) -> Result<serde_json::Value> {
        let a = validate_segment("alias", &alias.into())?;
        self.client
            .execute_json(
                reqwest::Method::DELETE,
                self.url(&["aliases", &a])?,
                None,
                None,
            )
            .await
    }

    // ------------------------------------------------------------------
    // Entities
    // ------------------------------------------------------------------

    /// List entities.
    pub async fn list_entities(&self, query: ListEntitiesQuery) -> Result<EntityListPage> {
        self.client
            .get_json(self.url(&["entities"])?, Some(&query))
            .await
    }

    /// Auto-paging variant of [`BankClient::list_entities`].
    pub fn list_entities_paged(&self, page_size: i64) -> Pager<EntityListItem> {
        let client = self.client.clone();
        let bank_id = self.bank_id.clone();
        Pager::new(move |offset| {
            let client = client.clone();
            let bank_id = bank_id.clone();
            async move {
                client
                    .bank(&bank_id)
                    .list_entities(ListEntitiesQuery::new().limit(page_size).offset(offset))
                    .await
            }
        })
    }

    /// Entity co-occurrence graph.
    pub async fn entity_graph(&self) -> Result<EntityGraphResponse> {
        self.client
            .get_json(self.url(&["entities", "graph"])?, Option::<&()>::None)
            .await
    }

    /// Entity details with observations.
    pub async fn get_entity(&self, entity_id: impl Into<String>) -> Result<EntityDetailResponse> {
        let id = validate_segment("entity_id", &entity_id.into())?;
        self.client
            .get_json(self.url(&["entities", &id])?, Option::<&()>::None)
            .await
    }

    /// Regenerate an entity's observations (deprecated upstream; async operation).
    pub async fn regenerate_entity_observations(
        &self,
        entity_id: impl Into<String>,
    ) -> Result<serde_json::Value> {
        let id = validate_segment("entity_id", &entity_id.into())?;
        self.client
            .post_json(
                self.url(&["entities", &id, "regenerate"])?,
                &serde_json::Value::Null,
                None,
            )
            .await
    }

    // ------------------------------------------------------------------
    // Mental models
    // ------------------------------------------------------------------

    /// List mental models.
    pub async fn list_mental_models(
        &self,
        query: ListMentalModelsQuery,
    ) -> Result<MentalModelPage> {
        self.client
            .get_json(self.url(&["mental-models"])?, Some(&query))
            .await
    }

    /// Create a mental model.
    pub async fn create_mental_model(
        &self,
        request: CreateMentalModelRequest,
    ) -> Result<MentalModelResponse> {
        self.client
            .post_json(self.url(&["mental-models"])?, &request, None)
            .await
    }

    /// Get a mental model.
    pub async fn get_mental_model(
        &self,
        mental_model_id: impl Into<String>,
    ) -> Result<MentalModelResponse> {
        let id = validate_segment("mental_model_id", &mental_model_id.into())?;
        self.client
            .get_json(self.url(&["mental-models", &id])?, Option::<&()>::None)
            .await
    }

    /// Update a mental model.
    pub async fn update_mental_model(
        &self,
        mental_model_id: impl Into<String>,
        update: UpdateMentalModelRequest,
    ) -> Result<MentalModelResponse> {
        let id = validate_segment("mental_model_id", &mental_model_id.into())?;
        self.client
            .patch_json(self.url(&["mental-models", &id])?, &update)
            .await
    }

    /// Delete a mental model.
    pub async fn delete_mental_model(
        &self,
        mental_model_id: impl Into<String>,
    ) -> Result<serde_json::Value> {
        let id = validate_segment("mental_model_id", &mental_model_id.into())?;
        self.client
            .execute_json(
                reqwest::Method::DELETE,
                self.url(&["mental-models", &id])?,
                None,
                None,
            )
            .await
    }

    /// Mental model refresh history.
    pub async fn mental_model_history(
        &self,
        mental_model_id: impl Into<String>,
    ) -> Result<Vec<MentalModelHistoryEntry>> {
        let id = validate_segment("mental_model_id", &mental_model_id.into())?;
        self.client
            .get_json(
                self.url(&["mental-models", &id, "history"])?,
                Option::<&()>::None,
            )
            .await
    }

    /// Refresh a mental model now (runs reflect; may take a while).
    pub async fn refresh_mental_model(
        &self,
        mental_model_id: impl Into<String>,
        request: RefreshMentalModelRequest,
    ) -> Result<MentalModelResponse> {
        let id = validate_segment("mental_model_id", &mental_model_id.into())?;
        self.client
            .post_json(
                self.url(&["mental-models", &id, "refresh"])?,
                &request,
                Some(self.client.inner.timeout.max(Duration::from_secs(300))),
            )
            .await
    }

    /// Dry-run a refresh: no persistence, full preview + diff.
    pub async fn dry_run_refresh_mental_model(
        &self,
        mental_model_id: impl Into<String>,
        request: RefreshMentalModelRequest,
    ) -> Result<MentalModelDryRunRefreshResult> {
        let id = validate_segment("mental_model_id", &mental_model_id.into())?;
        self.client
            .post_json(
                self.url(&["mental-models", &id, "dry-run-refresh"])?,
                &request,
                Some(self.client.inner.timeout.max(Duration::from_secs(300))),
            )
            .await
    }

    /// Clear a mental model's content (keeps the model itself).
    pub async fn clear_mental_model(
        &self,
        mental_model_id: impl Into<String>,
    ) -> Result<MentalModelResponse> {
        let id = validate_segment("mental_model_id", &mental_model_id.into())?;
        self.client
            .post_json(
                self.url(&["mental-models", &id, "clear"])?,
                &serde_json::Value::Null,
                None,
            )
            .await
    }

    // ------------------------------------------------------------------
    // Knowledge base
    // ------------------------------------------------------------------

    /// Knowledge-base tree.
    pub async fn knowledge_tree(&self, query: KnowledgeTreeQuery) -> Result<KnowledgeTreeResponse> {
        self.client
            .get_json(self.url(&["knowledge-base", "tree"])?, Some(&query))
            .await
    }

    /// Create a folder.
    pub async fn create_knowledge_folder(
        &self,
        request: CreateFolderRequest,
    ) -> Result<KnowledgeNode> {
        self.client
            .post_json(self.url(&["knowledge-base", "folders"])?, &request, None)
            .await
    }

    /// Create a page (backed by a mental model, content generated async).
    pub async fn create_knowledge_page(
        &self,
        request: CreatePageRequest,
    ) -> Result<CreateKnowledgePageResponse> {
        self.client
            .post_json(self.url(&["knowledge-base", "pages"])?, &request, None)
            .await
    }

    /// Get a page with its markdown.
    pub async fn get_knowledge_page(
        &self,
        page_id: impl Into<String>,
    ) -> Result<KnowledgePageResponse> {
        let id = validate_segment("page_id", &page_id.into())?;
        self.client
            .get_json(
                self.url(&["knowledge-base", "pages", &id])?,
                Option::<&()>::None,
            )
            .await
    }

    /// Hybrid (BM25 + vector) search over knowledge pages.
    pub async fn search_knowledge_base(
        &self,
        query: KnowledgeSearchQuery,
    ) -> Result<KnowledgePageSearchResponse> {
        self.client
            .get_json(self.url(&["knowledge-base", "search"])?, Some(&query))
            .await
    }

    /// Export the knowledge base as a markdown bundle.
    pub async fn export_knowledge_base(&self) -> Result<KnowledgePageBundleResponse> {
        self.client
            .get_json(
                self.url(&["knowledge-base", "export"])?,
                Option::<&()>::None,
            )
            .await
    }

    /// Rename/move a node or update a page's options.
    pub async fn update_knowledge_node(
        &self,
        node_id: impl Into<String>,
        update: UpdateNodeRequest,
    ) -> Result<KnowledgeNode> {
        let id = validate_segment("node_id", &node_id.into())?;
        self.client
            .patch_json(self.url(&["knowledge-base", "nodes", &id])?, &update)
            .await
    }

    /// Delete a node (folder or page).
    pub async fn delete_knowledge_node(
        &self,
        node_id: impl Into<String>,
    ) -> Result<serde_json::Value> {
        let id = validate_segment("node_id", &node_id.into())?;
        self.client
            .execute_json(
                reqwest::Method::DELETE,
                self.url(&["knowledge-base", "nodes", &id])?,
                None,
                None,
            )
            .await
    }

    // ------------------------------------------------------------------
    // Directives
    // ------------------------------------------------------------------

    /// List directives.
    pub async fn list_directives(&self, query: ListDirectivesQuery) -> Result<DirectivePage> {
        self.client
            .get_json(self.url(&["directives"])?, Some(&query))
            .await
    }

    /// Create a directive.
    pub async fn create_directive(
        &self,
        request: CreateDirectiveRequest,
    ) -> Result<DirectiveResponse> {
        self.client
            .post_json(self.url(&["directives"])?, &request, None)
            .await
    }

    /// Get a directive.
    pub async fn get_directive(
        &self,
        directive_id: impl Into<String>,
    ) -> Result<DirectiveResponse> {
        let id = validate_segment("directive_id", &directive_id.into())?;
        self.client
            .get_json(self.url(&["directives", &id])?, Option::<&()>::None)
            .await
    }

    /// Update a directive.
    pub async fn update_directive(
        &self,
        directive_id: impl Into<String>,
        update: UpdateDirectiveRequest,
    ) -> Result<DirectiveResponse> {
        let id = validate_segment("directive_id", &directive_id.into())?;
        self.client
            .patch_json(self.url(&["directives", &id])?, &update)
            .await
    }

    /// Delete a directive.
    pub async fn delete_directive(
        &self,
        directive_id: impl Into<String>,
    ) -> Result<serde_json::Value> {
        let id = validate_segment("directive_id", &directive_id.into())?;
        self.client
            .execute_json(
                reqwest::Method::DELETE,
                self.url(&["directives", &id])?,
                None,
                None,
            )
            .await
    }

    // ------------------------------------------------------------------
    // Documents
    // ------------------------------------------------------------------

    /// List documents.
    pub async fn list_documents(&self, query: ListDocumentsQuery) -> Result<DocumentPage> {
        self.client
            .get_json(self.url(&["documents"])?, Some(&query))
            .await
    }

    /// Auto-paging variant of [`BankClient::list_documents`].
    pub fn list_documents_paged(&self, page_size: i64) -> Pager<DocumentListItem> {
        let client = self.client.clone();
        let bank_id = self.bank_id.clone();
        Pager::new(move |offset| {
            let client = client.clone();
            let bank_id = bank_id.clone();
            async move {
                client
                    .bank(&bank_id)
                    .list_documents(ListDocumentsQuery::new().limit(page_size).offset(offset))
                    .await
            }
        })
    }

    /// Get a document with full text.
    pub async fn get_document(&self, document_id: impl Into<String>) -> Result<DocumentResponse> {
        let id = validate_segment("document_id", &document_id.into())?;
        self.client
            .get_json(self.url(&["documents", &id])?, Option::<&()>::None)
            .await
    }

    /// Update document metadata/tags/observation scopes.
    pub async fn update_document(
        &self,
        document_id: impl Into<String>,
        update: UpdateDocumentRequest,
    ) -> Result<DocumentResponse> {
        let id = validate_segment("document_id", &document_id.into())?;
        self.client
            .patch_json(self.url(&["documents", &id])?, &update)
            .await
    }

    /// Delete a document and its memories.
    pub async fn delete_document(
        &self,
        document_id: impl Into<String>,
    ) -> Result<DeleteDocumentResponse> {
        let id = validate_segment("document_id", &document_id.into())?;
        self.client
            .execute_json(
                reqwest::Method::DELETE,
                self.url(&["documents", &id])?,
                None,
                None,
            )
            .await
    }

    /// Document change history.
    pub async fn document_history(
        &self,
        document_id: impl Into<String>,
        query: DocumentHistoryQuery,
    ) -> Result<DocumentHistoryResponse> {
        let id = validate_segment("document_id", &document_id.into())?;
        self.client
            .get_json(self.url(&["documents", &id, "history"])?, Some(&query))
            .await
    }

    /// List a document's chunks.
    pub async fn list_document_chunks(
        &self,
        document_id: impl Into<String>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<ChunkPage> {
        #[derive(Serialize)]
        struct Q {
            #[serde(skip_serializing_if = "Option::is_none")]
            limit: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            offset: Option<i64>,
        }
        let id = validate_segment("document_id", &document_id.into())?;
        self.client
            .get_json(
                self.url(&["documents", &id, "chunks"])?,
                Some(&Q { limit, offset }),
            )
            .await
    }

    /// Re-extract facts from a stored document (async).
    pub async fn reprocess_document(
        &self,
        document_id: impl Into<String>,
    ) -> Result<ReprocessDocumentResponse> {
        let id = validate_segment("document_id", &document_id.into())?;
        self.client
            .post_json(
                self.url(&["documents", &id, "reprocess"])?,
                &serde_json::Value::Null,
                None,
            )
            .await
    }

    /// Fetch an attachment retained inline with a document.
    pub async fn get_attachment(&self, attachment_id: impl Into<String>) -> Result<Vec<u8>> {
        let id = validate_segment("attachment_id", &attachment_id.into())?;
        let resp = self
            .client
            .execute(
                reqwest::Method::GET,
                self.url(&["attachments", &id])?,
                None,
                None,
            )
            .await?;
        Ok(resp.bytes().await?.to_vec())
    }

    // ------------------------------------------------------------------
    // Operations
    // ------------------------------------------------------------------

    /// List async operations.
    pub async fn list_operations(&self, query: ListOperationsQuery) -> Result<OperationsPage> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            operations: Vec<OperationStatusResponse>,
            #[serde(default)]
            total: i64,
            #[serde(default)]
            limit: i64,
            #[serde(default)]
            offset: i64,
        }
        let raw: Raw = self
            .client
            .get_json(self.url(&["operations"])?, Some(&query))
            .await?;
        Ok(Page {
            items: raw.operations,
            total: raw.total,
            limit: raw.limit,
            offset: raw.offset,
        })
    }

    /// Get one operation's status.
    pub async fn get_operation(
        &self,
        operation_id: impl Into<String>,
        include_payload: bool,
    ) -> Result<OperationStatusResponse> {
        #[derive(Serialize)]
        struct Q {
            #[serde(skip_serializing_if = "std::ops::Not::not")]
            include_payload: bool,
        }
        let id = validate_segment("operation_id", &operation_id.into())?;
        self.client
            .get_json(
                self.url(&["operations", &id])?,
                Some(&Q { include_payload }),
            )
            .await
    }

    /// Cancel a pending/in-flight operation.
    pub async fn cancel_operation(
        &self,
        operation_id: impl Into<String>,
    ) -> Result<OperationActionResponse> {
        let id = validate_segment("operation_id", &operation_id.into())?;
        self.client
            .execute_json(
                reqwest::Method::DELETE,
                self.url(&["operations", &id])?,
                None,
                None,
            )
            .await
    }

    /// Retry a failed operation.
    pub async fn retry_operation(
        &self,
        operation_id: impl Into<String>,
    ) -> Result<OperationActionResponse> {
        let id = validate_segment("operation_id", &operation_id.into())?;
        self.client
            .post_json(
                self.url(&["operations", &id, "retry"])?,
                &serde_json::Value::Null,
                None,
            )
            .await
    }

    /// Delete a terminal operation's record.
    pub async fn delete_operation(
        &self,
        operation_id: impl Into<String>,
    ) -> Result<OperationActionResponse> {
        let id = validate_segment("operation_id", &operation_id.into())?;
        self.client
            .post_json(
                self.url(&["operations", &id, "delete"])?,
                &serde_json::Value::Null,
                None,
            )
            .await
    }

    /// Poll an async operation until terminal, failed, or the deadline passes.
    pub async fn wait_for_operation(
        &self,
        operation_id: impl Into<String>,
        wait: OperationWait,
    ) -> Result<OperationStatusResponse> {
        let id = operation_id.into();
        let deadline = tokio::time::Instant::now() + wait.timeout;
        loop {
            let op = self.get_operation(&id, false).await?;
            if op.is_terminal() {
                return Ok(op);
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(op);
            }
            tokio::time::sleep(wait.poll_interval).await;
        }
    }

    // ------------------------------------------------------------------
    // Webhooks
    // ------------------------------------------------------------------

    /// Register a webhook.
    pub async fn register_webhook(&self, request: CreateWebhookRequest) -> Result<WebhookResponse> {
        self.client
            .post_json(self.url(&["webhooks"])?, &request, None)
            .await
    }

    /// List webhooks.
    pub async fn list_webhooks(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<WebhookPage> {
        #[derive(Serialize)]
        struct Q {
            #[serde(skip_serializing_if = "Option::is_none")]
            limit: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            offset: Option<i64>,
        }
        self.client
            .get_json(self.url(&["webhooks"])?, Some(&Q { limit, offset }))
            .await
    }

    /// Update a webhook.
    pub async fn update_webhook(
        &self,
        webhook_id: impl Into<String>,
        update: UpdateWebhookRequest,
    ) -> Result<WebhookResponse> {
        let id = validate_segment("webhook_id", &webhook_id.into())?;
        self.client
            .patch_json(self.url(&["webhooks", &id])?, &update)
            .await
    }

    /// Delete a webhook.
    pub async fn delete_webhook(&self, webhook_id: impl Into<String>) -> Result<serde_json::Value> {
        let id = validate_segment("webhook_id", &webhook_id.into())?;
        self.client
            .execute_json(
                reqwest::Method::DELETE,
                self.url(&["webhooks", &id])?,
                None,
                None,
            )
            .await
    }

    /// List a webhook's deliveries (cursor-paginated).
    pub async fn list_webhook_deliveries(
        &self,
        webhook_id: impl Into<String>,
        query: WebhookDeliveriesQuery,
    ) -> Result<WebhookDeliveryPage> {
        let id = validate_segment("webhook_id", &webhook_id.into())?;
        self.client
            .get_json(self.url(&["webhooks", &id, "deliveries"])?, Some(&query))
            .await
    }

    // ------------------------------------------------------------------
    // Transfer (bank & document export/import, clone, templates)
    // ------------------------------------------------------------------

    /// Export the bank (async); download the archive with
    /// [`Client::download_file`] once the operation completes.
    pub async fn export_bank(&self) -> Result<BankTransferSubmitResponse> {
        self.client
            .post_json(
                self.url(&["transfer", "export"])?,
                &serde_json::Value::Null,
                None,
            )
            .await
    }

    /// Import a bank transfer ZIP archive (async). `into_bank_id` places the
    /// data into an existing bank instead of the archive's original one.
    pub async fn import_bank(
        &self,
        archive: Vec<u8>,
        options: ImportBankOptions,
    ) -> Result<BankTransferSubmitResponse> {
        let mut url = self.url(&["transfer", "import"])?;
        if let Some(q) = options.to_query() {
            url.set_query(Some(&q));
        }
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(archive)
                .file_name("transfer.zip")
                .mime_str("application/zip")
                .map_err(|e| Error::InvalidInput(format!("invalid mime type: {e}")))?,
        );
        let resp = self
            .client
            .request(reqwest::Method::POST, url)
            .multipart(form)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::from_response(status, &body));
        }
        decode(resp).await
    }

    /// Clone the bank into `request.target_bank_id` (async).
    pub async fn clone_bank(
        &self,
        request: CloneBankRequest,
    ) -> Result<BankTransferSubmitResponse> {
        self.client
            .post_json(self.url(&["clone"])?, &request, None)
            .await
    }

    /// Export selected documents (async).
    pub async fn export_documents(
        &self,
        options: ExportDocumentsOptions,
    ) -> Result<DocumentExportSubmitResponse> {
        let mut url = self.url(&["document-transfer", "export"])?;
        if let Some(q) = options.to_query() {
            url.set_query(Some(&q));
        }
        self.client
            .execute_json(
                reqwest::Method::POST,
                url,
                Some(serde_json::Value::Null),
                None,
            )
            .await
    }

    /// Import a documents transfer ZIP archive (async).
    pub async fn import_documents(
        &self,
        archive: Vec<u8>,
        on_conflict: Option<String>,
    ) -> Result<DocumentImportSubmitResponse> {
        let mut url = self.url(&["document-transfer"])?;
        if let Some(c) = on_conflict {
            url.query_pairs_mut().append_pair("on_conflict", &c);
        }
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(archive)
                .file_name("documents.zip")
                .mime_str("application/zip")
                .map_err(|e| Error::InvalidInput(format!("invalid mime type: {e}")))?,
        );
        let resp = self
            .client
            .request(reqwest::Method::POST, url)
            .multipart(form)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::from_response(status, &body));
        }
        decode(resp).await
    }

    /// List previous document transfers.
    pub async fn list_document_transfers(&self) -> Result<serde_json::Value> {
        self.client
            .get_json(self.url(&["document-transfer"])?, Option::<&()>::None)
            .await
    }

    /// Import a bank template manifest (config + mental models + directives).
    pub async fn import_bank_template(
        &self,
        manifest: BankTemplateManifest,
        dry_run: bool,
    ) -> Result<BankTemplateImportResponse> {
        let mut url = self.url(&["import"])?;
        if dry_run {
            url.query_pairs_mut().append_pair("dry_run", "true");
        }
        self.client.post_json(url, &manifest, None).await
    }

    /// Export this bank as a template manifest.
    pub async fn export_bank_template(&self) -> Result<BankTemplateManifest> {
        self.client
            .get_json(self.url(&["export"])?, Option::<&()>::None)
            .await
    }

    // ------------------------------------------------------------------
    // Audit & LLM traces
    // ------------------------------------------------------------------

    /// List audit log entries.
    pub async fn list_audit_logs(&self, query: ListAuditLogsQuery) -> Result<AuditLogPage> {
        self.client
            .get_json(self.url(&["audit-logs"])?, Some(&query))
            .await
    }

    /// Audit log statistics.
    pub async fn audit_log_stats(&self) -> Result<AuditLogStatsResponse> {
        self.client
            .get_json(self.url(&["audit-logs", "stats"])?, Option::<&()>::None)
            .await
    }

    /// One audit log entry.
    pub async fn get_audit_log(&self, log_id: impl Into<String>) -> Result<AuditLogEntry> {
        let id = validate_segment("log_id", &log_id.into())?;
        self.client
            .get_json(self.url(&["audit-logs", &id])?, Option::<&()>::None)
            .await
    }

    /// List LLM request traces.
    pub async fn list_llm_requests(&self, query: ListLlmRequestsQuery) -> Result<LlmRequestPage> {
        self.client
            .get_json(self.url(&["llm-requests"])?, Some(&query))
            .await
    }

    /// LLM request statistics.
    pub async fn llm_request_stats(&self, query: LlmStatsQuery) -> Result<LlmRequestStatsResponse> {
        self.client
            .get_json(self.url(&["llm-requests", "stats"])?, Some(&query))
            .await
    }

    /// Preview the prompts an operation would send, without an LLM call.
    pub async fn preview_prompts(
        &self,
        request: PromptPreviewRequest,
    ) -> Result<PromptPreviewResponse> {
        self.client
            .post_json(self.url(&["prompts", "preview"])?, &request, None)
            .await
    }
}

// ----------------------------------------------------------------------
// ClientBuilder impl
// ----------------------------------------------------------------------

impl ClientBuilder {
    /// Set the server base URL (self-hosted `http://localhost:8888` or cloud
    /// `https://api.hindsight.vectorize.io`).
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(
            Url::parse(&url.into())
                .unwrap_or_else(|_| Url::parse("http://invalid.invalid").expect("static")),
        );
        self
    }

    /// Override the default path prefix (`v1/default`). Leave as-is unless
    /// your deployment uses a different tenant segment.
    #[must_use]
    pub fn path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.path_prefix = prefix.into().trim_matches('/').to_string();
        self
    }

    /// Set the API key (`hsk_…`), sent as `Authorization: Bearer …`.
    #[must_use]
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Overall request timeout (default 120 s; reflect gets at least 300 s).
    #[must_use]
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    /// TCP connect timeout (default 10 s).
    #[must_use]
    pub fn connect_timeout(mut self, d: Duration) -> Self {
        self.connect_timeout = Some(d);
        self
    }

    /// Override the `User-Agent` header (your app is appended: `myapp/1.0 hindsight-client-rs/0.2.0`).
    #[must_use]
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = format!("{} {DEFAULT_USER_AGENT}", ua.into());
        self
    }

    /// Set the retry policy.
    #[must_use]
    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    /// Also retry writes (POST/PUT/PATCH/DELETE) on 429/5xx. Off by default.
    #[must_use]
    pub fn retry_writes(mut self, yes: bool) -> Self {
        self.retry.scope = if yes { Scope::Everything } else { Scope::Reads };
        self
    }

    /// Idle connections kept per host (default 32).
    #[must_use]
    pub fn pool_max_idle_per_host(mut self, n: usize) -> Self {
        self.pool_max_idle_per_host = n;
        self
    }

    /// Build the client.
    ///
    /// # Errors
    /// Returns [`Error::InvalidInput`] when no base URL was given, or
    /// [`Error::Transport`] when the HTTP pool cannot be built.
    pub fn build(self) -> Result<Client> {
        let base_url = self.base_url.ok_or_else(|| {
            Error::InvalidInput("base_url is required (or use Client::from_env)".into())
        })?;
        if base_url.path() != "/"
            && !self.path_prefix.is_empty()
            && base_url.path() != "/v1/default/"
        {
            // A base URL with a path is allowed (e.g. reverse proxy prefix);
            // the path_prefix is then appended after it.
        }
        let mut origin = base_url.clone();
        origin.set_path("");
        origin.set_query(None);
        origin.set_fragment(None);

        let mut base = base_url.clone();
        base.set_query(None);
        base.set_fragment(None);
        if !self.path_prefix.is_empty() {
            for seg in self.path_prefix.split('/').filter(|s| !s.is_empty()) {
                let _ = base
                    .path_segments_mut()
                    .map_err(|()| {
                        Error::InvalidInput("base URL cannot be a base (no host?)".into())
                    })?
                    .push(seg);
            }
        }

        let http = reqwest::Client::builder()
            .user_agent(self.user_agent)
            .timeout(self.timeout)
            .connect_timeout(self.connect_timeout.unwrap_or(Duration::from_secs(10)))
            .pool_max_idle_per_host(self.pool_max_idle_per_host)
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .map_err(Error::Transport)?;

        Ok(Client {
            inner: Arc::new(Inner {
                http,
                origin,
                base,
                api_key: self.api_key,
                retry: self.retry,
                timeout: self.timeout,
            }),
        })
    }
}

// ----------------------------------------------------------------------
// Query structs used only by the client layer
// ----------------------------------------------------------------------

/// Query for [`Client::list_banks`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListBanksQuery {
    /// Search ids, aliases and names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl ListBanksQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Search banks by id/alias/name.
    #[must_use]
    pub fn search(mut self, q: impl Into<String>) -> Self {
        self.q = Some(q.into());
        self
    }

    /// Page size.
    #[must_use]
    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Page offset.
    #[must_use]
    pub fn offset(mut self, n: i64) -> Self {
        self.offset = Some(n);
        self
    }
}

/// Query for [`BankClient::list_tags`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListTagsQuery {
    /// Substring filter on tag names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// Filter by tag source (`memory`, `document`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl ListTagsQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Substring filter on tag names.
    #[must_use]
    pub fn search(mut self, q: impl Into<String>) -> Self {
        self.q = Some(q.into());
        self
    }

    /// Page size.
    #[must_use]
    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Page offset.
    #[must_use]
    pub fn offset(mut self, n: i64) -> Self {
        self.offset = Some(n);
        self
    }
}

/// Query for [`BankClient::memories_timeseries`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MemoriesTimeseriesQuery {
    /// Bucket period (`hour`, `day`, `week`, `month`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    /// Time axis (`created_at`, `mentioned_at`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_field: Option<TimeField>,
}

impl MemoriesTimeseriesQuery {
    /// An empty query (server default period).
    pub fn new() -> Self {
        Self::default()
    }

    /// Bucket period.
    #[must_use]
    pub fn period(mut self, p: impl Into<String>) -> Self {
        self.period = Some(p.into());
        self
    }

    /// Time axis.
    #[must_use]
    pub fn time_field(mut self, f: TimeField) -> Self {
        self.time_field = Some(f);
        self
    }
}

/// Query for [`BankClient::list_documents`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListDocumentsQuery {
    /// Substring search on id/text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// Tag filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// How `tags` are matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<TagsMatch>,
    /// Time axis for filtering/ordering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_field: Option<TimeField>,
    /// Inclusive lower bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Exclusive upper bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl ListDocumentsQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Substring search.
    #[must_use]
    pub fn search(mut self, q: impl Into<String>) -> Self {
        self.q = Some(q.into());
        self
    }

    /// Tag filter.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// How `tags` are matched.
    #[must_use]
    pub fn tags_match(mut self, m: TagsMatch) -> Self {
        self.tags_match = Some(m);
        self
    }

    /// Time window on `field`.
    #[must_use]
    pub fn time_window(
        mut self,
        field: TimeField,
        start: Option<String>,
        end: Option<String>,
    ) -> Self {
        self.time_field = Some(field);
        self.start_date = start;
        self.end_date = end;
        self
    }

    /// Page size.
    #[must_use]
    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Page offset.
    #[must_use]
    pub fn offset(mut self, n: i64) -> Self {
        self.offset = Some(n);
        self
    }
}

/// Page of documents.
pub type DocumentPage = Page<DocumentListItem>;

/// Query for [`BankClient::document_history`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DocumentHistoryQuery {
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl DocumentHistoryQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Page size.
    #[must_use]
    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Page offset.
    #[must_use]
    pub fn offset(mut self, n: i64) -> Self {
        self.offset = Some(n);
        self
    }
}

/// Query for [`BankClient::list_mental_models`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListMentalModelsQuery {
    /// Tag filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// How `tags` are matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<TagsMatch>,
    /// Include full content (`true`) or metadata only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<bool>,
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    /// Hide models backing knowledge pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_knowledge_pages: Option<bool>,
}

impl ListMentalModelsQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Tag filter.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// Include full content.
    #[must_use]
    pub fn detail(mut self, yes: bool) -> Self {
        self.detail = Some(yes);
        self
    }

    /// Page size.
    #[must_use]
    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Page offset.
    #[must_use]
    pub fn offset(mut self, n: i64) -> Self {
        self.offset = Some(n);
        self
    }
}

/// Page of mental models.
pub type MentalModelPage = Page<MentalModelResponse>;

/// Query for [`BankClient::list_directives`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListDirectivesQuery {
    /// Tag filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// How `tags` are matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<TagsMatch>,
    /// Only active directives.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_only: Option<bool>,
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl ListDirectivesQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Tag filter.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// Only active directives.
    #[must_use]
    pub fn active_only(mut self, yes: bool) -> Self {
        self.active_only = Some(yes);
        self
    }

    /// Page size.
    #[must_use]
    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Page offset.
    #[must_use]
    pub fn offset(mut self, n: i64) -> Self {
        self.offset = Some(n);
        self
    }
}

/// Page of directives.
pub type DirectivePage = Page<DirectiveResponse>;

/// Query for [`BankClient::list_webhook_deliveries`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct WebhookDeliveriesQuery {
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Opaque cursor from the previous page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

impl WebhookDeliveriesQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Page size.
    #[must_use]
    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Continuation cursor.
    #[must_use]
    pub fn cursor(mut self, c: impl Into<String>) -> Self {
        self.cursor = Some(c.into());
        self
    }
}

/// Options for [`BankClient::import_bank`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportBankOptions {
    /// Import mode (`merge`, `replace`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Target bank id override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_bank_id: Option<String>,
    /// Document conflict policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_conflict: Option<String>,
    /// Include memories/data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_data: Option<bool>,
    /// Include bank config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_bank_config: Option<bool>,
    /// Include history.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_history: Option<bool>,
}

impl ImportBankOptions {
    /// Default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Import mode (`merge`, `replace`, …).
    #[must_use]
    pub fn mode(mut self, mode: impl Into<String>) -> Self {
        self.mode = Some(mode.into());
        self
    }

    /// Target bank id override.
    #[must_use]
    pub fn target_bank_id(mut self, id: impl Into<String>) -> Self {
        self.target_bank_id = Some(id.into());
        self
    }

    /// Include memories/data.
    #[must_use]
    pub fn include_data(mut self, yes: bool) -> Self {
        self.include_data = Some(yes);
        self
    }

    /// Include bank config.
    #[must_use]
    pub fn include_bank_config(mut self, yes: bool) -> Self {
        self.include_bank_config = Some(yes);
        self
    }

    /// Include history.
    #[must_use]
    pub fn include_history(mut self, yes: bool) -> Self {
        self.include_history = Some(yes);
        self
    }

    /// Render as a query string (without `?`), or `None` when empty.
    pub fn to_query(&self) -> Option<String> {
        Self::to_query_impl(&serde_json::to_value(self).ok()?)
    }
}

/// Options for [`BankClient::export_documents`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ExportDocumentsOptions {
    /// Export only this document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Include derived observations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_observations: Option<bool>,
    /// Include knowledge-base pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_knowledge_base: Option<bool>,
}

impl ExportDocumentsOptions {
    /// Default options (everything in the bank).
    pub fn new() -> Self {
        Self::default()
    }

    /// Export only this document.
    #[must_use]
    pub fn document_id(mut self, id: impl Into<String>) -> Self {
        self.document_id = Some(id.into());
        self
    }

    /// Include derived observations.
    #[must_use]
    pub fn include_observations(mut self, yes: bool) -> Self {
        self.include_observations = Some(yes);
        self
    }

    /// Include knowledge-base pages.
    #[must_use]
    pub fn include_knowledge_base(mut self, yes: bool) -> Self {
        self.include_knowledge_base = Some(yes);
        self
    }

    /// Render as a query string, or `None` when empty.
    pub fn to_query(&self) -> Option<String> {
        ImportBankOptions::to_query_impl(&serde_json::to_value(self).ok()?)
    }
}

impl ImportBankOptions {
    fn to_query_impl(value: &serde_json::Value) -> Option<String> {
        let map = value.as_object()?;
        let mut pairs: Vec<String> = Vec::new();
        for (k, v) in map {
            if v.is_null() {
                continue;
            }
            let val = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                other => other.to_string(),
            };
            let enc = url::form_urlencoded::Serializer::new(String::new())
                .append_pair(k, &val)
                .finish();
            pairs.push(enc);
        }
        if pairs.is_empty() {
            None
        } else {
            Some(pairs.join("&"))
        }
    }
}

/// Query for [`BankClient::llm_request_stats`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LlmStatsQuery {
    /// Bucket period (`hour`, `day`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    /// Inclusive lower bound (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Exclusive upper bound (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
}

impl LlmStatsQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bucket period.
    #[must_use]
    pub fn period(mut self, p: impl Into<String>) -> Self {
        self.period = Some(p.into());
        self
    }

    /// Time window.
    #[must_use]
    pub fn window(mut self, start: impl Into<String>, end: impl Into<String>) -> Self {
        self.start_date = Some(start.into());
        self.end_date = Some(end.into());
        self
    }
}

/// Query for [`BankClient::knowledge_tree`].
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct KnowledgeTreeQuery {
    /// Tag filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// How `tags` are matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<TagsMatch>,
    /// Compound tag filters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_groups: Option<Vec<TagGroup>>,
}

impl KnowledgeTreeQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Tag filter.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }
}

/// Query for [`BankClient::search_knowledge_base`].
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct KnowledgeSearchQuery {
    /// The search query.
    pub q: String,
    /// Max hits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Tag filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// How `tags` are matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<TagsMatch>,
    /// Compound tag filters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_groups: Option<Vec<TagGroup>>,
}

impl KnowledgeSearchQuery {
    /// A search query.
    pub fn new(q: impl Into<String>) -> Self {
        Self {
            q: q.into(),
            limit: None,
            tags: None,
            tags_match: None,
            tag_groups: None,
        }
    }

    /// Max hits.
    #[must_use]
    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }
}

// ----------------------------------------------------------------------
// Misc public types
// ----------------------------------------------------------------------

/// A file to upload via [`BankClient::retain_files`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedFile {
    /// File contents.
    pub bytes: Vec<u8>,
    /// Display filename.
    pub filename: String,
    /// MIME type (e.g. `application/pdf`).
    pub mime_type: String,
}

impl UploadedFile {
    /// A file to upload.
    pub fn new(bytes: Vec<u8>, filename: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            bytes,
            filename: filename.into(),
            mime_type: mime_type.into(),
        }
    }
}

/// Create/update/patch response for a bank (id + whatever the server echoes).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BankWriteResponse {
    /// The bank id written to.
    pub bank_id: String,
    /// Other echoed fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Liveness response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LivenessResponse {
    /// Always `alive` when the handler answered.
    pub status: String,
    /// Server version.
    pub version: String,
    /// Seconds since process start.
    pub uptime_seconds: f64,
}

/// Version + feature flags response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VersionResponse {
    /// API version string.
    pub api_version: String,
    /// Enabled feature flags.
    pub features: JsonMap,
}

/// Detail of one memory unit (typed superset of the list item).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MemoryUnitDetail {
    /// Memory id.
    #[serde(default)]
    pub id: String,
    /// Fact text.
    #[serde(default)]
    pub text: String,
    /// Context label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Fact type.
    #[serde(default, rename = "fact_type", skip_serializing_if = "Option::is_none")]
    pub fact_type: Option<String>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Owning document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Curation state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Every other field the server returned.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// One observation history entry (shape is server-defined; lossless).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ObservationHistoryEntry {
    /// All fields the server returned.
    #[serde(flatten)]
    pub fields: JsonMap,
}

/// Prompt preview request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PromptPreviewRequest {
    /// Which operation's prompts to render (`retain`, `reflect`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    /// Named retain strategy to preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
}

impl PromptPreviewRequest {
    /// Preview retain prompts (the default).
    pub fn retain() -> Self {
        Self {
            operation: Some("retain".into()),
            strategy: None,
        }
    }

    /// Preview reflect prompts.
    pub fn reflect() -> Self {
        Self {
            operation: Some("reflect".into()),
            strategy: None,
        }
    }

    /// Preview a named strategy.
    pub fn strategy(strategy: impl Into<String>) -> Self {
        Self {
            operation: Some("retain".into()),
            strategy: Some(strategy.into()),
        }
    }
}

/// One prompt block.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PromptBlockModel {
    /// Block text (empty when inactive).
    pub text: String,
    /// `config` or `builtin`.
    pub source: String,
    /// Config field behind the block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Section slug for blocks the preview names itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// Heading extracted from the prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// Whether the block is active.
    #[serde(default = "crate::models::directive::default_true")]
    pub active: bool,
    /// Value shape (`text`, `boolean`, `choice`, `complex`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Choices for `choice` blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
    /// Whether the bank config API may override the field.
    #[serde(default)]
    pub editable: bool,
}

/// One prompt message.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PromptMessageModel {
    /// Role (`system`, `user`, …).
    pub role: String,
    /// Blocks composing the message.
    #[serde(default)]
    pub blocks: Vec<PromptBlockModel>,
}

/// One run setting that shapes the operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RunSettingModel {
    /// Setting field name.
    pub field: String,
    /// Rendered value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Value shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Whether the bank config API may override it.
    #[serde(default)]
    pub editable: bool,
}

/// Prompt preview response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PromptPreviewResponse {
    /// Messages in send order.
    #[serde(default)]
    pub messages: Vec<PromptMessageModel>,
    /// Strategy selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    /// The bank's retain strategies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategies: Option<Vec<String>>,
    /// Settings that shape the operation without appearing in prompts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_settings: Option<Vec<RunSettingModel>>,
    /// Response schema, when the operation produces one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<serde_json::Value>,
    /// Why the preview was skipped, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

fn push_segments(base: &Url, segments: &[&str]) -> Result<Url> {
    let mut url = base.clone();
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|()| Error::InvalidInput("base URL cannot contain segments".into()))?;
        for seg in segments {
            path.push(seg);
        }
    }
    Ok(url)
}

fn validate_bank_id(bank_id: impl Into<String>) -> Result<String> {
    validate_segment("bank_id", &bank_id.into())
}

fn validate_segment(name: &str, value: &str) -> Result<String> {
    if value.is_empty() {
        return Err(Error::InvalidInput(format!("{name} must not be empty")));
    }
    if value.contains(['\0', '\r', '\n']) || value.starts_with('_') {
        return Err(Error::InvalidInput(format!(
            "{name} contains control characters or starts with '_': {value:?}"
        )));
    }
    Ok(value.to_string())
}

fn attach_query(url: Url, query: Option<&impl Serialize>) -> Result<Url> {
    let Some(q) = query else {
        return Ok(url);
    };
    let value = serde_json::to_value(q).map_err(Error::Json)?;
    let serde_json::Value::Object(map) = &value else {
        return Ok(url);
    };
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (k, v) in map {
        push_query_value(&mut pairs, k, v);
    }
    let mut url = url;
    {
        let mut qp = url.query_pairs_mut();
        for (k, v) in pairs {
            qp.append_pair(&k, &v);
        }
    }
    Ok(url)
}

fn push_query_value(pairs: &mut Vec<(String, String)>, key: &str, v: &serde_json::Value) {
    match v {
        serde_json::Value::Null => {}
        serde_json::Value::Array(items) => {
            for item in items {
                push_query_value(pairs, key, item);
            }
        }
        serde_json::Value::String(s) => pairs.push((key.to_string(), s.clone())),
        serde_json::Value::Bool(b) => pairs.push((key.to_string(), b.to_string())),
        serde_json::Value::Number(n) => pairs.push((key.to_string(), n.to_string())),
        other @ serde_json::Value::Object(_) => {
            pairs.push((key.to_string(), other.to_string()));
        }
    }
}

fn is_retryable_transport(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_request() || (e.is_body() && !e.is_decode())
}

fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let raw = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    let secs: u64 = raw.trim().parse().ok()?;
    Some(Duration::from_secs(secs))
}
