//! Mental models: user-curated summaries refreshed from the bank.

use serde::{Deserialize, Serialize};

use super::common::{Budget, FactType, TagGroup, TagsMatch};

/// Refresh trigger configuration of a mental model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MentalModelTrigger {
    /// Refresh budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Budget>,
    /// Refresh mode: `full` regenerates from scratch, `delta` edits the existing document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Refresh after every observations consolidation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_after_consolidation: Option<bool>,
    /// Cron expression for scheduled refreshes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_cron: Option<String>,
    /// Cron expression for scheduled full refreshes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_refresh_cron: Option<String>,
    /// Minimum seconds between refreshes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_refresh_interval_seconds: Option<i64>,
    /// Fact types the refresh may ground on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_types: Option<Vec<FactType>>,
    /// Exclude all mental models from the refresh loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_mental_models: Option<bool>,
    /// Exclude specific mental models from the refresh loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_mental_model_ids: Option<Vec<String>>,
    /// Tag scope of the refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<TagsMatch>,
    /// Compound tag filters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_groups: Option<Vec<TagGroup>>,
    /// Include raw chunks in grounding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_chunks: Option<bool>,
    /// Recall token budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_max_tokens: Option<i64>,
    /// Chunk token budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_chunks_max_tokens: Option<i64>,
    /// Observation-search token budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflect_search_observations_max_tokens: Option<i64>,
    /// Include entities in observation search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflect_search_observations_include_entities: Option<bool>,
    /// JSON schema for the generated content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<serde_json::Value>,
    /// Record a trace for every refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_trace: Option<bool>,
}

/// Create-mental-model request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateMentalModelRequest {
    /// Stable id (lowercase alphanumeric with hyphens); server-generated when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Human-readable name.
    pub name: String,
    /// The query run to generate content.
    pub source_query: String,
    /// Tags for scoped visibility.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Maximum tokens for generated content.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i64,
    /// Refresh trigger configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<MentalModelTrigger>,
}

fn default_max_tokens() -> i64 {
    2048
}

impl CreateMentalModelRequest {
    /// A mental model request.
    pub fn new(name: impl Into<String>, source_query: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            source_query: source_query.into(),
            tags: Vec::new(),
            max_tokens: default_max_tokens(),
            trigger: None,
        }
    }

    /// Set a stable id.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set tags.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Set the content token budget.
    #[must_use]
    pub fn max_tokens(mut self, n: i64) -> Self {
        self.max_tokens = n;
        self
    }

    /// Set the trigger configuration.
    #[must_use]
    pub fn trigger(mut self, trigger: MentalModelTrigger) -> Self {
        self.trigger = Some(trigger);
        self
    }
}

/// Update-mental-model request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateMentalModelRequest {
    /// New name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New source query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_query: Option<String>,
    /// New tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// New content token budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    /// New trigger configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<MentalModelTrigger>,
}

impl UpdateMentalModelRequest {
    /// An empty update.
    pub fn new() -> Self {
        Self::default()
    }

    /// Rename.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Change the generating query.
    #[must_use]
    pub fn source_query(mut self, q: impl Into<String>) -> Self {
        self.source_query = Some(q.into());
        self
    }

    /// Change tags.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// Change the trigger configuration.
    #[must_use]
    pub fn trigger(mut self, trigger: MentalModelTrigger) -> Self {
        self.trigger = Some(trigger);
        self
    }
}

/// A mental model.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MentalModelResponse {
    /// Model id.
    pub id: String,
    /// Owning bank.
    pub bank_id: String,
    /// Display name.
    pub name: String,
    /// The generating query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_query: Option<String>,
    /// Current content (markdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Content token budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    /// Trigger configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<MentalModelTrigger>,
    /// Last successful refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refreshed_at: Option<String>,
    /// Newest memory the source query would see.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_memory_seen_at: Option<String>,
    /// Last failed refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh_failed_at: Option<String>,
    /// Last full (non-delta) refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_full_refreshed_at: Option<String>,
    /// Creation time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Whether this model backs a knowledge page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_knowledge_page: Option<bool>,
    /// Whether the content is stale relative to the bank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_stale: Option<bool>,
}

/// Refresh request body.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshMentalModelRequest {
    /// Override the trigger's mode (`full` or `delta`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

impl RefreshMentalModelRequest {
    /// Default refresh.
    pub fn new() -> Self {
        Self::default()
    }

    /// Force a mode.
    #[must_use]
    pub fn mode(mut self, mode: impl Into<String>) -> Self {
        self.mode = Some(mode.into());
        self
    }
}

/// Counts of facts a refresh/dry-run retrieved and used.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MentalModelFactCounts {
    /// Facts the tool calls returned, by fact type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieved: Option<serde_json::Value>,
    /// Facts the agent used, by fact type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used: Option<serde_json::Value>,
}

/// Delta operations applied/skipped in a delta refresh.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MentalModelDeltaOperations {
    /// Operations applied, in order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied: Option<Vec<serde_json::Value>>,
    /// Operations dropped, each with a reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<Vec<serde_json::Value>>,
}

/// Retraction detection result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MentalModelRetraction {
    /// Whether a retraction was detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected: Option<bool>,
    /// Extra retraction fields.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

/// Scope summary of a refresh run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MentalModelRefreshScope {
    /// Effective tags in scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Extra scope fields.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

/// Time window of a refresh run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MentalModelRefreshWindow {
    /// Window start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// Window end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

/// One tool call in a refresh trace.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MentalModelTraceToolCall {
    /// Tool name.
    pub tool: String,
    /// Execution time in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    /// Extra fields.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

/// Refresh trace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MentalModelRefreshTrace {
    /// Tool calls made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<MentalModelTraceToolCall>>,
    /// Extra trace fields.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

/// Dry-run refresh result (nothing is persisted).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MentalModelDryRunRefreshResult {
    /// The mental model previewed.
    pub mental_model_id: String,
    /// Display name.
    pub name: String,
    /// Mode asked for.
    pub requested_mode: String,
    /// Mode actually run.
    pub effective_mode: String,
    /// Why the mode fell back, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_fallback_reason: Option<String>,
    /// What a real refresh would do with the document.
    pub outcome: String,
    /// Whether a real refresh would write new content.
    pub would_persist: bool,
    /// Scope summary.
    #[serde(default)]
    pub scope: MentalModelRefreshScope,
    /// Time window.
    #[serde(default)]
    pub window: MentalModelRefreshWindow,
    /// Fact counts.
    #[serde(default)]
    pub facts: MentalModelFactCounts,
    /// Current content.
    pub current_content: String,
    /// Raw reflect synthesis.
    pub candidate_content: String,
    /// What a real refresh would store.
    pub preview_content: String,
    /// Unified diff from current to preview; empty when identical.
    pub diff: String,
    /// Delta operations.
    #[serde(default)]
    pub delta_operations: MentalModelDeltaOperations,
    /// Retraction detection.
    #[serde(default)]
    pub retraction: MentalModelRetraction,
    /// Refresh trace.
    #[serde(default)]
    pub trace: MentalModelRefreshTrace,
    /// Token usage.
    #[serde(default)]
    pub usage: super::common::TokenUsage,
    /// Wall-clock duration.
    #[serde(default)]
    pub duration_ms: i64,
    /// Conditions worth a human's attention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

/// One history entry of a mental model.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MentalModelHistoryEntry {
    /// Snapshot content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: super::common::JsonMap,
}
