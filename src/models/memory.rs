//! Memory models: retain, recall, list, curate, graph, dry-run extraction.

use serde::{Deserialize, Serialize};

use super::common::{
    Budget, Content, EntityInput, FactType, IncludeOptions, JsonMap, MinScores, ObservationScopes,
    Page, RerankOptions, StringMap, TagGroup, TagsMatch, TemporalWindow, TimeField, TokenUsage,
};

/// One memory item in a retain request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryItem {
    /// Raw content: plain text or multimodal blocks.
    pub content: Content,
    /// Reference timestamp for relative-time resolution (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Context label for the memory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Free-form metadata stored with the memory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<StringMap>,
    /// Group this item into a document (deduplicated by content hash).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Entity hints to attach beyond auto-extraction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Vec<EntityInput>>,
    /// Resolve supplied entity names against the bank's existing entities (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_entities: Option<bool>,
    /// Tags to scope the memories with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// How observations are scoped for this item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation_scopes: Option<ObservationScopes>,
    /// Named retain strategy (a key of the bank's `retain_strategies`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    /// `replace` clears the document's memories before writing; `append` adds to them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_mode: Option<UpdateMode>,
}

/// How a retain call writes into an existing document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    /// Clear the document's memories first.
    Replace,
    /// Add to the document's existing memories.
    Append,
}

impl MemoryItem {
    /// A memory item from plain-text content.
    pub fn new(content: impl Into<Content>) -> Self {
        Self {
            content: content.into(),
            timestamp: None,
            context: None,
            metadata: None,
            document_id: None,
            entities: None,
            resolve_entities: None,
            tags: None,
            observation_scopes: None,
            strategy: None,
            update_mode: None,
        }
    }

    /// Set the reference timestamp (ISO 8601).
    #[must_use]
    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = Some(ts.into());
        self
    }

    /// Set the context label.
    #[must_use]
    pub fn context(mut self, ctx: impl Into<String>) -> Self {
        self.context = Some(ctx.into());
        self
    }

    /// Attach free-form metadata.
    #[must_use]
    pub fn metadata(mut self, metadata: StringMap) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Insert one metadata key/value.
    #[must_use]
    pub fn meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .get_or_insert_with(Default::default)
            .insert(key.into(), value.into());
        self
    }

    /// Group into a document (deduplicated by content hash).
    #[must_use]
    pub fn document_id(mut self, id: impl Into<String>) -> Self {
        self.document_id = Some(id.into());
        self
    }

    /// Attach entity hints (anything convertible, e.g. `&str` or `EntityInput`).
    #[must_use]
    pub fn entities(mut self, entities: impl IntoIterator<Item: Into<EntityInput>>) -> Self {
        self.entities = Some(entities.into_iter().map(Into::into).collect());
        self
    }

    /// Take supplied entity names literally instead of resolving them (default true).
    #[must_use]
    pub fn literal_entities(mut self) -> Self {
        self.resolve_entities = Some(false);
        self
    }

    /// Tag the memories.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// Set observation scoping.
    #[must_use]
    pub fn observation_scopes(mut self, scopes: ObservationScopes) -> Self {
        self.observation_scopes = Some(scopes);
        self
    }

    /// Use a named retain strategy.
    #[must_use]
    pub fn strategy(mut self, strategy: impl Into<String>) -> Self {
        self.strategy = Some(strategy.into());
        self
    }

    /// Replace the target document's memories.
    #[must_use]
    pub fn replace_document(mut self) -> Self {
        self.update_mode = Some(UpdateMode::Replace);
        self
    }

    /// Append to the target document's memories.
    #[must_use]
    pub fn append_to_document(mut self) -> Self {
        self.update_mode = Some(UpdateMode::Append);
        self
    }
}

impl<T: Into<Content>> From<T> for MemoryItem {
    fn from(content: T) -> Self {
        MemoryItem::new(content)
    }
}

/// Retain request body: `POST /v1/default/banks/{bank_id}/memories`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetainRequest {
    /// Items to retain (at least one).
    pub items: Vec<MemoryItem>,
    /// Process asynchronously in the background (default false).
    #[serde(rename = "async", default, skip_serializing_if = "std::ops::Not::not")]
    pub async_: bool,
    /// Extra tags applied to every document created by this call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_tags: Option<Vec<String>>,
    /// Caller-supplied idempotency key for the async operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

impl RetainRequest {
    /// A retain request from items (each convertible into a [`MemoryItem`]).
    pub fn new(items: impl IntoIterator<Item: Into<MemoryItem>>) -> Self {
        Self {
            items: items.into_iter().map(Into::into).collect(),
            async_: false,
            document_tags: None,
            operation_id: None,
        }
    }

    /// Process asynchronously (returns an operation id to poll).
    #[must_use]
    pub fn as_async(mut self) -> Self {
        self.async_ = true;
        self
    }

    /// Tag every document this call creates.
    #[must_use]
    pub fn document_tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.document_tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// Idempotency key for the async operation.
    #[must_use]
    pub fn operation_id(mut self, id: impl Into<String>) -> Self {
        self.operation_id = Some(id.into());
        self
    }
}

/// Retain response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RetainResponse {
    /// Whether the retain succeeded.
    pub success: bool,
    /// The bank written to.
    pub bank_id: String,
    /// Number of items accepted.
    pub items_count: i64,
    /// Whether the operation was processed asynchronously.
    #[serde(rename = "async")]
    pub processed_async: bool,
    /// Async operation id, when `async` was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// Per-item operation ids, when the server reports them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_ids: Option<Vec<String>>,
    /// LLM token usage (empty for async retains).
    #[serde(default)]
    pub usage: TokenUsage,
}

/// Recall request body: `POST /v1/default/banks/{bank_id}/memories/recall`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallRequest {
    /// The natural-language query.
    pub query: String,
    /// Fact types to recall (all types when omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<FactType>>,
    /// Let observations supersede the raw facts they were consolidated from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefer_observations: Option<bool>,
    /// Decision-model reranker toggles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rerank: Option<RerankOptions>,
    /// Retrieval budget (server default `mid`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<Budget>,
    /// Maximum tokens the returned facts may collectively occupy (default 4096).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    /// Include a timing trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<bool>,
    /// Query-time anchor for relative temporal expressions (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_timestamp: Option<String>,
    /// Extra data to include in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<IncludeOptions>,
    /// Tag scope filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// How `tags` are matched (server default `any`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<TagsMatch>,
    /// Compound boolean tag filters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_groups: Option<Vec<TagGroup>>,
    /// Per-stage score floors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_scores: Option<MinScores>,
    /// Explicit temporal window for the temporal arm (ranks, does not filter).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temporal_window: Option<TemporalWindow>,
}

impl RecallRequest {
    /// A recall request for `query`.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            types: None,
            prefer_observations: None,
            rerank: None,
            budget: None,
            max_tokens: None,
            trace: None,
            query_timestamp: None,
            include: None,
            tags: None,
            tags_match: None,
            tag_groups: None,
            min_scores: None,
            temporal_window: None,
        }
    }

    /// Restrict to these fact types.
    #[must_use]
    pub fn types(mut self, types: impl IntoIterator<Item = FactType>) -> Self {
        self.types = Some(types.into_iter().collect());
        self
    }

    /// Drop raw facts superseded by a returned observation.
    #[must_use]
    pub fn prefer_observations(mut self, yes: bool) -> Self {
        self.prefer_observations = Some(yes);
        self
    }

    /// Decision-model reranker toggles.
    #[must_use]
    pub fn rerank(mut self, prune: bool, dedup: bool) -> Self {
        self.rerank = Some(RerankOptions {
            prune: Some(prune),
            dedup: Some(dedup),
        });
        self
    }

    /// Retrieval budget.
    #[must_use]
    pub fn budget(mut self, budget: Budget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Token budget for returned facts.
    #[must_use]
    pub fn max_tokens(mut self, n: i64) -> Self {
        self.max_tokens = Some(n);
        self
    }

    /// Ask for the timing trace.
    #[must_use]
    pub fn trace(mut self, yes: bool) -> Self {
        self.trace = Some(yes);
        self
    }

    /// Anchor relative temporal expressions to this instant (ISO 8601).
    #[must_use]
    pub fn query_timestamp(mut self, ts: impl Into<String>) -> Self {
        self.query_timestamp = Some(ts.into());
        self
    }

    /// Include raw source-text chunks.
    #[must_use]
    pub fn include_chunks(mut self, max_tokens: i64) -> Self {
        self.include.get_or_insert_with(Default::default).chunks =
            Some(super::common::ChunkIncludeOptions {
                max_tokens: Some(max_tokens),
            });
        self
    }

    /// Include entity observations with a token budget.
    #[must_use]
    pub fn include_entities(mut self, max_tokens: i64) -> Self {
        self.include.get_or_insert_with(Default::default).entities =
            Some(super::common::EntityIncludeOptions {
                max_tokens: Some(max_tokens),
            });
        self
    }

    /// Include source facts for observation results.
    #[must_use]
    pub fn include_source_facts(mut self, max_tokens: i64) -> Self {
        self.include
            .get_or_insert_with(Default::default)
            .source_facts = Some(super::common::SourceFactsIncludeOptions {
            max_tokens: Some(max_tokens),
            max_tokens_per_observation: None,
        });
        self
    }

    /// Filter by tags.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// How `tags` are matched.
    #[must_use]
    pub fn tags_match(mut self, mode: TagsMatch) -> Self {
        self.tags_match = Some(mode);
        self
    }

    /// Compound boolean tag filters (AND-ed together).
    #[must_use]
    pub fn tag_groups(mut self, groups: impl IntoIterator<Item = TagGroup>) -> Self {
        self.tag_groups = Some(groups.into_iter().collect());
        self
    }

    /// Per-stage score floors.
    #[must_use]
    pub fn min_scores(mut self, scores: MinScores) -> Self {
        self.min_scores = Some(scores);
        self
    }

    /// Explicit temporal window (ranks, does not filter).
    #[must_use]
    pub fn temporal_window(mut self, window: TemporalWindow) -> Self {
        self.temporal_window = Some(window);
        self
    }
}

impl From<&str> for RecallRequest {
    fn from(q: &str) -> Self {
        RecallRequest::new(q)
    }
}

impl From<String> for RecallRequest {
    fn from(q: String) -> Self {
        RecallRequest::new(q)
    }
}

/// One recall result.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RecallResult {
    /// Fact id.
    pub id: String,
    /// The fact text.
    pub text: String,
    /// Fact type, when reported.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Entity names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entities: Option<Vec<String>>,
    /// Context label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Event-range start (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_start: Option<String>,
    /// Event-range end (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_end: Option<String>,
    /// When the fact was said/written (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentioned_at: Option<String>,
    /// Owning document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Owning chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_id: Option<String>,
    /// Free-form metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonMap>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// For observations: ids of the source facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_fact_ids: Option<Vec<String>>,
    /// Ranking scores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scores: Option<RecallScores>,
    /// Inline attachments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChunkAttachment>>,
}

/// Ranking scores of a recall result.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct RecallScores {
    /// Final ranking score (reranker + recency/temporal/proof boosts).
    #[serde(rename = "final")]
    pub final_score: f64,
    /// Decision-model reranker score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reranker: Option<f64>,
    /// Semantic arm score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic: Option<f64>,
    /// Keyword (BM25) arm score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<f64>,
}

/// An attachment served back inline with a result/chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkAttachment {
    /// Id inside the text's placeholder (prefix of the bytes' sha256).
    pub id: String,
    /// Full sha256 of the attachment bytes.
    pub hash: String,
    /// `image` or `file`, as the caller sent it.
    pub kind: String,
    /// MIME type.
    pub media_type: String,
    /// Size in bytes.
    pub byte_size: i64,
    /// Optional display filename.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// Bank-scoped API path serving the bytes.
    pub url: String,
}

/// A raw source-text chunk returned by recall.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChunkData {
    /// Chunk id.
    pub id: String,
    /// Position within the document.
    pub chunk_index: i64,
    /// Chunk text.
    pub text: String,
}

/// An entity's state as included by recall.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntityStateResponse {
    /// Canonical entity id.
    pub entity_id: String,
    /// Canonical entity name.
    pub canonical_name: String,
    /// Entity observations.
    #[serde(default)]
    pub observations: Vec<EntityObservationResponse>,
}

/// One entity observation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntityObservationResponse {
    /// Observation text.
    pub text: String,
    /// When last mentioned (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentioned_at: Option<String>,
}

/// Recall response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RecallResponse {
    /// The recalled results.
    pub results: Vec<RecallResult>,
    /// Timing trace, when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<JsonMap>,
    /// Entity observations keyed by canonical name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entities: Option<std::collections::BTreeMap<String, EntityStateResponse>>,
    /// Raw source-text chunks keyed by chunk id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunks: Option<std::collections::BTreeMap<String, ChunkData>>,
    /// Source facts for observation results, keyed by fact id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_facts: Option<std::collections::BTreeMap<String, RecallResult>>,
    /// Whether source facts hit the token budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_facts_truncated: Option<bool>,
}

impl RecallResponse {
    /// Format the results as an LLM-ready context string, grouped by fact
    /// type, with entities and timestamps inline. This is the bridge between
    /// recall and "inject memories into my prompt".
    #[must_use]
    pub fn to_context_string(&self) -> String {
        use std::fmt::Write as _;
        if self.results.is_empty() {
            return String::new();
        }
        let mut out = String::new();
        let mut last_type: Option<&str> = None;
        for r in &self.results {
            let kind = r.kind.as_deref().unwrap_or("memory");
            if last_type != Some(kind) {
                if !out.is_empty() {
                    out.push('\n');
                }
                let _ = writeln!(out, "# {kind}");
                last_type = Some(kind);
            }
            out.push_str("- ");
            out.push_str(r.text.trim());
            if let Some(entities) = &r.entities {
                if !entities.is_empty() {
                    let _ = write!(out, "  [entities: {}]", entities.join(", "));
                }
            }
            if let Some(at) = &r.mentioned_at {
                let _ = write!(out, "  (mentioned {at})");
            }
            out.push('\n');
        }
        out
    }
}

/// A memory unit as returned by the list endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MemoryUnitListItem {
    /// Memory id.
    pub id: String,
    /// The fact text.
    #[serde(default)]
    pub text: String,
    /// Context label.
    #[serde(default)]
    pub context: String,
    /// Event date (ISO 8601), empty when unknown.
    #[serde(default)]
    pub date: String,
    /// Fact type (`world`/`experience`/`observation`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_type: Option<String>,
    /// Owning document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// When the fact was said/written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mentioned_at: Option<String>,
    /// Event-range start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurred_start: Option<String>,
    /// Event-range end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurred_end: Option<String>,
    /// Comma-separated canonical entity names.
    #[serde(default)]
    pub entities: String,
    /// Owning chunk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_id: Option<String>,
    /// Independent sightings of this fact.
    #[serde(default)]
    pub proof_count: i64,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Free-form metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonMap>,
    /// When the observation was consolidated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consolidated_at: Option<String>,
    /// When consolidation permanently failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consolidation_failed_at: Option<String>,
    /// Curation state.
    #[serde(default)]
    pub state: String,
    /// Why the memory was invalidated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalidation_reason: Option<String>,
    /// When the memory was invalidated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalidated_at: Option<String>,
    /// Last manual text edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<String>,
    /// Last write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// An observation's source facts; empty for a source fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_memory_ids: Option<Vec<String>>,
}

/// Body for curating (editing/invalidating) a memory:
/// `PATCH /v1/default/banks/{bank_id}/memories/{memory_id}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UpdateMemoryRequest {
    /// New fact text (re-embeds, drops derived observations/links, re-consolidates).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// New context; `Some("")` clears it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// New occurred-range start; `Some("")` clears it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_start: Option<String>,
    /// New occurred-range end; `Some("")` clears it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_end: Option<String>,
    /// New mention time; `Some("")` clears it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentioned_at: Option<String>,
    /// Reclassify as `world` or `experience`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fact_type: Option<String>,
    /// Replace the fact's entities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Vec<String>>,
    /// Take entity names literally instead of fuzzy-resolving (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_entities: Option<bool>,
    /// `invalidated` to soft-retire, `valid` to revert.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Free-text reason recorded when invalidating.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl UpdateMemoryRequest {
    /// An empty curation request.
    pub fn new() -> Self {
        Self::default()
    }

    /// Edit the fact text.
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Edit the context.
    #[must_use]
    pub fn context(mut self, ctx: impl Into<String>) -> Self {
        self.context = Some(ctx.into());
        self
    }

    /// Reclassify the fact.
    #[must_use]
    pub fn fact_type(mut self, ft: FactType) -> Self {
        self.fact_type = Some(ft.to_string());
        self
    }

    /// Replace entities (taken literally when `literal` is set).
    #[must_use]
    pub fn entities(mut self, entities: impl IntoIterator<Item: Into<String>>) -> Self {
        self.entities = Some(entities.into_iter().map(Into::into).collect());
        self
    }

    /// Take supplied entity names literally.
    #[must_use]
    pub fn literal_entities(mut self) -> Self {
        self.resolve_entities = Some(false);
        self
    }

    /// Soft-retire the memory with an optional reason.
    #[must_use]
    pub fn invalidate(mut self, reason: Option<String>) -> Self {
        self.state = Some("invalidated".into());
        self.reason = reason;
        self
    }

    /// Revert an invalidated memory to `valid`.
    #[must_use]
    pub fn restore(mut self) -> Self {
        self.state = Some("valid".into());
        self
    }
}

/// One candidate fact from a dry-run extraction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedFact {
    /// Candidate fact text.
    pub text: String,
    /// Fact type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fact_type: Option<String>,
    /// Event-range start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_start: Option<String>,
    /// Event-range end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_end: Option<String>,
    /// Entity names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entities: Option<Vec<String>>,
    /// Chunk the fact came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_index: Option<i64>,
    /// Inline attachments referenced by the fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<serde_json::Value>>,
}

/// One chunk from a dry-run extraction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionChunk {
    /// Chunk text.
    pub text: String,
    /// Facts extracted from this chunk.
    #[serde(default)]
    pub fact_count: i64,
}

/// Dry-run extraction result (nothing is persisted).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct DryRunExtractionResult {
    /// Candidate facts.
    #[serde(default)]
    pub facts: Vec<ExtractedFact>,
    /// Chunks the input was cut into.
    #[serde(default)]
    pub chunks: Vec<ExtractionChunk>,
    /// Aggregated token usage.
    #[serde(default)]
    pub usage: TokenUsage,
}

/// Request for previewing extraction without persistence:
/// `POST .../memories/dry-run-extract`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DryRunExtractRequest {
    /// Raw content to extract from (string or blocks).
    pub content: Content,
    /// Optional context about the content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Reference timestamp for relative times (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Named retain strategy to extract under.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    /// Override the bank's retain mission for this call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_mission: Option<String>,
    /// Override the extraction mode for this call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_extraction_mode: Option<String>,
    /// Override custom extraction instructions for this call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_custom_instructions: Option<String>,
    /// Override causal-link extraction for this call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_extract_causal_links: Option<bool>,
    /// Override chunk size for this call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_chunk_size: Option<i64>,
    /// Controlled entity-label vocabulary for this call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_labels: Option<Vec<serde_json::Value>>,
    /// Allow free-form entity labels for this call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities_allow_free_form: Option<bool>,
    /// Override the extraction output language for this call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_output_language: Option<String>,
}

impl DryRunExtractRequest {
    /// A dry-run for `content`.
    pub fn new(content: impl Into<Content>) -> Self {
        Self {
            content: content.into(),
            context: None,
            timestamp: None,
            strategy: None,
            retain_mission: None,
            retain_extraction_mode: None,
            retain_custom_instructions: None,
            retain_extract_causal_links: None,
            retain_chunk_size: None,
            entity_labels: None,
            entities_allow_free_form: None,
            llm_output_language: None,
        }
    }

    /// Set the context.
    #[must_use]
    pub fn context(mut self, ctx: impl Into<String>) -> Self {
        self.context = Some(ctx.into());
        self
    }

    /// Set the reference timestamp.
    #[must_use]
    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = Some(ts.into());
        self
    }

    /// Extract under a named retain strategy.
    #[must_use]
    pub fn strategy(mut self, s: impl Into<String>) -> Self {
        self.strategy = Some(s.into());
        self
    }
}

/// A memory-graph node.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MemoryGraphNode {
    /// Node data.
    pub data: MemoryGraphNodeData,
}

/// Data of a memory-graph node.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MemoryGraphNodeData {
    /// Memory id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Full text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// A memory-graph edge.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MemoryGraphEdge {
    /// Edge data.
    pub data: MemoryGraphEdgeData,
}

/// Data of a memory-graph edge.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MemoryGraphEdgeData {
    /// Edge id (e.g. `1-2-semantic`).
    pub id: String,
    /// Link type (e.g. `semantic`); wire field `linkType`.
    #[serde(rename = "linkType")]
    pub link_type: String,
    /// Source node id.
    pub source: String,
    /// Target node id.
    pub target: String,
    /// Edge weight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

/// One row of the graph table view.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MemoryGraphTableRow {
    /// Memory id.
    pub id: String,
    /// Fact text.
    pub text: String,
    /// Context label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Event date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Entity summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entities: Option<String>,
}

/// Graph response for visualization.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GraphDataResponse {
    /// Graph nodes.
    pub nodes: Vec<MemoryGraphNode>,
    /// Graph edges.
    pub edges: Vec<MemoryGraphEdge>,
    /// Table rows (one per memory).
    pub table_rows: Vec<MemoryGraphTableRow>,
    /// Total memory units in the bank.
    pub total_units: i64,
    /// Limit applied.
    pub limit: i64,
}

/// A tag with its memory count.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TagItem {
    /// The tag.
    pub tag: String,
    /// Memories carrying it.
    pub count: i64,
}

/// Page of tags.
pub type TagPage = Page<TagItem>;

/// Observation scopes response (`GET .../observations/scopes`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ObservationScopesResponse {
    /// Scopes present in the bank.
    pub scopes: Vec<ObservationScope>,
    /// Total scopes.
    pub total: i64,
    /// Page size.
    pub limit: i64,
    /// Page offset.
    pub offset: i64,
}

/// Response for clearing one memory's derived observations.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ClearMemoryObservationsResponse {
    /// How many observations were removed.
    pub deleted_count: i64,
}

/// An observation scope with its memory count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationScope {
    /// The scope's tags (empty for the shared/global scope).
    pub tags: Vec<String>,
    /// Memories in this scope.
    pub count: i64,
}

/// Query for listing memory units (`GET .../memories/list`).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ListMemoriesQuery {
    /// Filter by fact type (wire field `type`).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Case-insensitive substring filter on text and context (unindexed — slow on large banks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// Filter by consolidation state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidation_state: Option<String>,
    /// Filter by curation state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Only memories from this document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Only memories linked to this entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    /// Tag filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// How `tags` are matched.
    #[serde(rename = "tags_match", skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<TagsMatch>,
    /// Time axis to filter and order by.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_field: Option<TimeField>,
    /// Inclusive ISO datetime lower bound on `time_field`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Exclusive ISO datetime upper bound on `time_field`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Page size (server default 100).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl ListMemoriesQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by fact type.
    #[must_use]
    pub fn fact_type(mut self, ft: FactType) -> Self {
        self.kind = Some(ft.to_string());
        self
    }

    /// Substring filter (unindexed — prefer recall for latency-sensitive paths).
    #[must_use]
    pub fn search(mut self, q: impl Into<String>) -> Self {
        self.q = Some(q.into());
        self
    }

    /// Only memories from this document.
    #[must_use]
    pub fn document(mut self, id: impl Into<String>) -> Self {
        self.document_id = Some(id.into());
        self
    }

    /// Only memories linked to this entity.
    #[must_use]
    pub fn entity(mut self, id: impl Into<String>) -> Self {
        self.entity_id = Some(id.into());
        self
    }

    /// Filter by tags.
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

    /// Filter and order by a time window on `field`.
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

/// Page of memory units.
pub type MemoryUnitPage = Page<MemoryUnitListItem>;

/// Query for the graph endpoint (`GET .../graph`).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct GraphQuery {
    /// Filter nodes by fact type (wire field `type`).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Node limit (server default 1000).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Substring filter on text and context (unindexed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// Tag filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// How `tags` are matched (server default `all_strict`).
    #[serde(rename = "tags_match", skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<TagsMatch>,
    /// Only memories from this document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Only memories from this chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_id: Option<String>,
}

impl GraphQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by fact type.
    #[must_use]
    pub fn fact_type(mut self, ft: FactType) -> Self {
        self.kind = Some(ft.to_string());
        self
    }

    /// Node limit.
    #[must_use]
    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Substring filter.
    #[must_use]
    pub fn search(mut self, q: impl Into<String>) -> Self {
        self.q = Some(q.into());
        self
    }
}
