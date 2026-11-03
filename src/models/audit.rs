//! Observability: audit logs and LLM request traces.

use serde::{Deserialize, Serialize};

use super::common::{JsonMap, Page};

/// One audit log entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AuditLogEntry {
    /// Entry id.
    pub id: String,
    /// Action performed (e.g. `retain`, `recall`).
    pub action: String,
    /// Transport (`http`, `mcp`, …).
    pub transport: String,
    /// Bank acted on.
    pub bank_id: String,
    /// Start time.
    pub started_at: String,
    /// End time.
    pub ended_at: String,
    /// Duration in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    /// Free-form metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonMap>,
    /// Sanitized request payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<serde_json::Value>,
    /// Sanitized response payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<serde_json::Value>,
}

/// One audit log entry in list responses (summary shape).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AuditLogSummary {
    /// Entry id.
    pub id: String,
    /// Action performed.
    pub action: String,
    /// Transport.
    pub transport: String,
    /// Bank acted on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bank_id: Option<String>,
    /// Start time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// End time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// Duration in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    /// Free-form metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonMap>,
}

/// Page of audit log summaries.
pub type AuditLogPage = Page<AuditLogSummary>;

/// One histogram bucket of audit stats.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AuditLogStatsBucket {
    /// Bucket key (time or action, per the server's grouping).
    pub key: String,
    /// Count in this bucket.
    pub count: i64,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Audit stats response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AuditLogStatsResponse {
    /// Histogram buckets.
    #[serde(default)]
    pub buckets: Vec<AuditLogStatsBucket>,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Query for listing audit logs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListAuditLogsQuery {
    /// Filter by action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// Inclusive lower bound (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Exclusive upper bound (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl ListAuditLogsQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by action.
    #[must_use]
    pub fn action(mut self, a: impl Into<String>) -> Self {
        self.action = Some(a.into());
        self
    }

    /// Time window.
    #[must_use]
    pub fn window(mut self, start: impl Into<String>, end: impl Into<String>) -> Self {
        self.start_date = Some(start.into());
        self.end_date = Some(end.into());
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

/// One LLM request trace entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LlmRequestEntry {
    /// Entry id.
    pub id: String,
    /// Bank the call served.
    pub bank_id: String,
    /// Operation (retain, recall, reflect, consolidation, …).
    pub operation: String,
    /// Scope within the operation (agent_1, final, …).
    pub scope: String,
    /// Provider (openai, anthropic, …).
    pub provider: String,
    /// Model name.
    pub model: String,
    /// Call status.
    pub status: String,
    /// Start time.
    pub started_at: String,
    /// End time.
    pub ended_at: String,
    /// Duration in milliseconds.
    #[serde(default)]
    pub duration_ms: i64,
    /// Input tokens.
    #[serde(default)]
    pub input_tokens: i64,
    /// Output tokens.
    #[serde(default)]
    pub output_tokens: i64,
    /// Total tokens.
    #[serde(default)]
    pub total_tokens: i64,
    /// Cached prompt tokens.
    #[serde(default)]
    pub cached_tokens: i64,
    /// Reasoning tokens.
    #[serde(default)]
    pub thoughts_tokens: i64,
    /// Error message, when failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Trace id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Span id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    /// Parent span id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    /// Provider info blob.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_info: Option<JsonMap>,
    /// Free-form metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonMap>,
}

/// Page of LLM request traces.
pub type LlmRequestPage = Page<LlmRequestEntry>;

/// One histogram bucket of LLM stats.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LlmRequestStatsBucket {
    /// Bucket key.
    pub key: String,
    /// Extra aggregated fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// LLM request stats response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LlmRequestStatsResponse {
    /// Bank (when scoped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bank_id: Option<String>,
    /// Period label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    /// Window start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// Truncation info.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunc: Option<serde_json::Value>,
    /// Histogram buckets.
    #[serde(default)]
    pub buckets: Vec<LlmRequestStatsBucket>,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Query for listing LLM traces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListLlmRequestsQuery {
    /// Filter by operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    /// Filter by status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Inclusive lower bound (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Exclusive upper bound (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl ListLlmRequestsQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by operation.
    #[must_use]
    pub fn operation(mut self, op: impl Into<String>) -> Self {
        self.operation = Some(op.into());
        self
    }

    /// Filter by status.
    #[must_use]
    pub fn status(mut self, s: impl Into<String>) -> Self {
        self.status = Some(s.into());
        self
    }

    /// Time window.
    #[must_use]
    pub fn window(mut self, start: impl Into<String>, end: impl Into<String>) -> Self {
        self.start_date = Some(start.into());
        self.end_date = Some(end.into());
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
