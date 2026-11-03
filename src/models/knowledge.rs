//! Knowledge base: folders and pages (mental-model-backed documents).

use serde::{Deserialize, Serialize};

use super::common::{JsonMap, Page};
use super::mental_model::MentalModelTrigger;

/// A node in the knowledge-base tree.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KnowledgeNode {
    /// Node id.
    pub id: String,
    /// `folder` or `page`.
    pub kind: String,
    /// Display name.
    pub name: String,
    /// Parent node id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Backing mental model id (pages only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mental_model_id: Option<String>,
    /// `true` = system-owned, `false` = hand-authored.
    #[serde(default)]
    pub managed: bool,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Timestamp label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Whether the content is stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_stale: Option<bool>,
    /// Last failed refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh_failed_at: Option<String>,
    /// Trigger configuration (pages only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<MentalModelTrigger>,
    /// Children (folders only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<KnowledgeNode>>,
}

/// Tree response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KnowledgeTreeResponse {
    /// Root nodes.
    pub roots: Vec<KnowledgeNode>,
}

/// Create-folder request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateFolderRequest {
    /// Folder name.
    pub name: String,
    /// Parent folder id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

impl CreateFolderRequest {
    /// A new folder at the root.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parent_id: None,
        }
    }

    /// Place the folder under `parent`.
    #[must_use]
    pub fn parent(mut self, id: impl Into<String>) -> Self {
        self.parent_id = Some(id.into());
        self
    }
}

/// Create-page request.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreatePageRequest {
    /// Page name.
    pub name: String,
    /// Query used to generate and refresh content.
    pub source_query: String,
    /// Parent folder id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Content token budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    /// Refresh trigger configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<MentalModelTrigger>,
}

impl CreatePageRequest {
    /// A new page at the root.
    pub fn new(name: impl Into<String>, source_query: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source_query: source_query.into(),
            parent_id: None,
            tags: None,
            max_tokens: None,
            trigger: None,
        }
    }

    /// Place the page under `parent`.
    #[must_use]
    pub fn parent(mut self, id: impl Into<String>) -> Self {
        self.parent_id = Some(id.into());
        self
    }

    /// Set tags.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// Set the refresh trigger.
    #[must_use]
    pub fn trigger(mut self, trigger: MentalModelTrigger) -> Self {
        self.trigger = Some(trigger);
        self
    }
}

/// A knowledge page with markdown.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KnowledgePageResponse {
    /// Page id.
    pub id: String,
    /// Page name.
    pub name: String,
    /// Page type (from a `type:<x>` tag, else `knowledge-page`).
    pub kind: String,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Timestamp label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Markdown body (no frontmatter).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Full markdown: YAML frontmatter + body.
    pub markdown: String,
}

/// One search hit over knowledge pages.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KnowledgePageSearchResult {
    /// Page id.
    pub id: String,
    /// Page name.
    pub name: String,
    /// Relevance score.
    pub score: f64,
    /// Match snippet.
    pub snippet: String,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Search response (BM25 + vector hybrid).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KnowledgePageSearchResponse {
    /// Hits.
    pub results: Vec<KnowledgePageSearchResult>,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Page of knowledge nodes (list endpoints).
pub type KnowledgeNodePage = Page<KnowledgeNode>;

/// Update-node request (`PATCH .../knowledge-base/nodes/{node_id}`).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UpdateNodeRequest {
    /// New name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Move under this parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// New generating query (pages only).
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

impl UpdateNodeRequest {
    /// An empty update.
    pub fn new() -> Self {
        Self::default()
    }

    /// Rename the node.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Move the node under a parent.
    #[must_use]
    pub fn parent(mut self, id: impl Into<String>) -> Self {
        self.parent_id = Some(id.into());
        self
    }
}

/// Result of creating a page: node id, backing mental model, refresh operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreateKnowledgePageResponse {
    /// The new page node id.
    pub page_id: String,
    /// The backing mental model id.
    pub mental_model_id: String,
    /// Async content-generation operation id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// One file of a markdown bundle export.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KnowledgePageBundleFile {
    /// Path within the bundle.
    pub path: String,
    /// File content.
    pub content: String,
}

/// Markdown bundle export.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KnowledgePageBundleResponse {
    /// Bundle files.
    pub files: Vec<KnowledgePageBundleFile>,
}
