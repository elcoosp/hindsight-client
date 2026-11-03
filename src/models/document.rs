//! Document models and chunks.

use serde::{Deserialize, Serialize};

use super::common::{JsonMap, ObservationScopes, Page};
use super::memory::ChunkAttachment;

/// A document in list responses.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DocumentListItem {
    /// Document id.
    pub id: String,
    /// Owning bank.
    #[serde(default)]
    pub bank_id: String,
    /// Content hash (deduplication key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// First retained (ISO 8601).
    #[serde(default)]
    pub created_at: String,
    /// Last written (ISO 8601).
    #[serde(default)]
    pub updated_at: String,
    /// Stored text length in characters.
    #[serde(default)]
    pub text_length: i64,
    /// Memory units extracted from this document.
    #[serde(default)]
    pub memory_unit_count: i64,
    /// Retain parameters captured at ingest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_params: Option<JsonMap>,
    /// Document metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_metadata: Option<JsonMap>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Which fields matched a `q` search (`id`, `original_text`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_fields: Option<Vec<String>>,
}

/// A document with its full text.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DocumentResponse {
    /// Document id.
    pub id: String,
    /// Owning bank.
    pub bank_id: String,
    /// Stored text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_text: Option<String>,
    /// Content hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Created (ISO 8601).
    pub created_at: String,
    /// Updated (ISO 8601).
    pub updated_at: String,
    /// Memory units extracted.
    pub memory_unit_count: i64,
    /// Per-fact-type node counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes_by_fact_type: Option<JsonMap>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Document metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_metadata: Option<JsonMap>,
    /// Retain parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_params: Option<JsonMap>,
    /// Observation scoping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_scopes: Option<ObservationScopes>,
    /// Inline attachments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChunkAttachment>>,
}

/// Update-document request (`PATCH`): metadata/tags/observation scopes.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UpdateDocumentRequest {
    /// New document metadata (merged by the server).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_metadata: Option<JsonMap>,
    /// New tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// New observation scoping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_scopes: Option<ObservationScopes>,
}

impl UpdateDocumentRequest {
    /// An empty update.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set metadata from key/value pairs.
    #[must_use]
    pub fn metadata(
        mut self,
        metadata: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        self.document_metadata = Some(
            metadata
                .into_iter()
                .map(|(k, v)| (k.into(), serde_json::Value::String(v.into())))
                .collect(),
        );
        self
    }

    /// Set raw metadata JSON.
    #[must_use]
    pub fn metadata_json(mut self, metadata: JsonMap) -> Self {
        self.document_metadata = Some(metadata);
        self
    }

    /// Set tags.
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
}

/// Reprocess-document response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReprocessDocumentResponse {
    /// Whether the reprocess was accepted.
    pub success: bool,
    /// Async operation id.
    pub operation_id: String,
    /// Items queued.
    pub items_count: i64,
}

/// Delete-document response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeleteDocumentResponse {
    /// Whether the deletion succeeded.
    pub success: bool,
}

/// A document chunk.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChunkResponse {
    /// Chunk id.
    pub id: String,
    /// Owning document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Position in the document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_index: Option<i64>,
    /// Chunk text.
    pub text: String,
    /// Extra chunk fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Page of chunks.
pub type ChunkPage = Page<ChunkResponse>;

/// What changed in a document event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentEventKind {
    /// Initial retain.
    Retain,
    /// Append to an existing document.
    Append,
    /// Reprocessed.
    Reprocess,
    /// Retagged.
    Retag,
    /// A fact was edited.
    FactEdited,
    /// A fact was invalidated.
    FactInvalidated,
    /// A fact was restored.
    FactRestored,
    /// A fact was deleted.
    FactDeleted,
    /// Observations were cleared.
    ObservationsCleared,
    /// Consolidation ran.
    Consolidation,
}

/// Sample of observation ids touched by an event.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ObservationIdSample {
    /// Number touched.
    #[serde(default)]
    pub count: i64,
    /// First few ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,
}

/// One document history event.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DocumentEvent {
    /// The document.
    pub document_id: String,
    /// What happened.
    pub kind: DocumentEventKind,
    /// When (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Triggering operation, when async.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// Fact touched by the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_id: Option<String>,
    /// Facts added.
    #[serde(default)]
    pub facts_added: i64,
    /// Facts removed.
    #[serde(default)]
    pub facts_removed: i64,
    /// Observations created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observations_created: Option<ObservationIdSample>,
    /// Observations updated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observations_updated: Option<ObservationIdSample>,
    /// Observations removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observations_removed: Option<ObservationIdSample>,
}

/// Document history response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DocumentHistoryResponse {
    /// The document.
    pub document_id: String,
    /// Total events.
    pub total: i64,
    /// Page size.
    pub limit: i64,
    /// Offset.
    pub offset: i64,
    /// Events on this page.
    pub items: Vec<DocumentEvent>,
}
