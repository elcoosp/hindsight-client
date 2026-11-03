//! Async operations: submission, status, retry, cancel.

use serde::{Deserialize, Serialize};

use super::common::{JsonMap, Page};

/// One async operation.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OperationStatusResponse {
    /// Operation id.
    pub operation_id: String,
    /// Status (`pending`, `processing`, `completed`, `failed`, `cancelled`).
    pub status: String,
    /// Operation type (e.g. `retain`, `consolidation`, `refresh_mental_model`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_type: Option<String>,
    /// Legacy alias of `operation_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Task type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// Mental model targeted by the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mental_model_id: Option<String>,
    /// Created (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last update (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Completion time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// Failure message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Retry count so far.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_count: Option<i64>,
    /// Next scheduled retry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_retry_at: Option<String>,
    /// Coarse progress snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<OperationProgress>,
    /// Result metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_metadata: Option<JsonMap>,
    /// Details for mental-model refresh operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<RefreshMentalModelOperationDetails>,
    /// Child operations (batched retains).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_operations: Option<Vec<ChildOperationStatus>>,
    /// Task payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_payload: Option<JsonMap>,
}

impl OperationStatusResponse {
    /// Whether the operation reached a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "failed" | "cancelled")
    }

    /// Whether the operation succeeded.
    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.status == "completed"
    }

    /// Whether the operation failed.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.status == "failed"
    }
}

/// Progress snapshot of an operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OperationProgress {
    /// Coarse phase (e.g. `processing_batch`).
    pub stage: String,
    /// When the snapshot was written.
    pub at: String,
    /// Items processed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processed: Option<i64>,
    /// Items total.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
    /// Free-form detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

/// Details of a mental-model refresh operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RefreshMentalModelOperationDetails {
    /// Discriminator: `refresh_mental_model`.
    #[serde(default)]
    pub operation_type: String,
    /// What the refresh did with the document (`content_written`, `no_changes`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    /// Failure reason, when failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

/// One child operation of a batch.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChildOperationStatus {
    /// Child operation id.
    pub operation_id: String,
    /// Status.
    pub status: String,
    /// Index within the batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_batch_index: Option<i64>,
    /// Items in this child.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items_count: Option<i64>,
    /// Failure message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Page of operations.
pub type OperationsPage = Page<OperationStatusResponse>;

/// Response when submitting an async operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AsyncOperationSubmitResponse {
    /// Operation id to poll.
    pub operation_id: String,
    /// Initial status.
    pub status: String,
}

/// Generic success + message envelope (cancel/retry/delete operations).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OperationActionResponse {
    /// Whether the action succeeded.
    pub success: bool,
    /// Human-readable outcome.
    pub message: String,
    /// The operation acted on.
    pub operation_id: String,
}

/// Generic success envelope (clear memories, delete documents, …).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeleteResponse {
    /// Whether the deletion succeeded.
    pub success: bool,
}

/// Query for listing operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListOperationsQuery {
    /// Filter by status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Filter by operation type (wire parameter `type`).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl ListOperationsQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by status.
    #[must_use]
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Filter by task type.
    #[must_use]
    pub fn task_type(mut self, t: impl Into<String>) -> Self {
        self.task_type = Some(t.into());
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
