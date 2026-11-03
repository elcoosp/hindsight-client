//! Entities: extracted people/organizations/concepts and their graph.

use serde::{Deserialize, Serialize};

use super::common::{JsonMap, Page};

/// An entity in list responses.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntityListItem {
    /// Entity id.
    pub id: String,
    /// Canonical name.
    pub canonical_name: String,
    /// How many memories mention it.
    pub mention_count: i64,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Page of entities.
pub type EntityListPage = Page<EntityListItem>;

/// One entity observation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntityObservation {
    /// Observation text.
    pub text: String,
    /// When last mentioned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mentioned_at: Option<String>,
}

/// Entity detail response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntityDetailResponse {
    /// Entity id.
    pub id: String,
    /// Canonical name.
    pub canonical_name: String,
    /// Mention count.
    pub mention_count: i64,
    /// Consolidated observations.
    #[serde(default)]
    pub observations: Vec<EntityObservation>,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Graph node.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntityGraphNode {
    /// Node data.
    pub data: EntityGraphNodeData,
}

/// Graph node data.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntityGraphNodeData {
    /// Entity id.
    pub id: String,
    /// Canonical name.
    pub label: String,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Graph edge.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EntityGraphEdge {
    /// Edge data.
    pub data: EntityGraphEdgeData,
}

/// Graph edge data.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EntityGraphEdgeData {
    /// Edge id.
    pub id: String,
    /// Source entity id.
    pub source: String,
    /// Target entity id.
    pub target: String,
    /// Co-occurrence weight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Entity co-occurrence graph.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EntityGraphResponse {
    /// Nodes.
    pub nodes: Vec<EntityGraphNode>,
    /// Edges.
    pub edges: Vec<EntityGraphEdge>,
}

/// Regenerate-observations response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntityObservationResponse2 {
    /// Whether regeneration started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Query params for listing entities.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListEntitiesQuery {
    /// Substring filter on canonical name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// Page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Page offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl ListEntitiesQuery {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Substring filter on the canonical name.
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
