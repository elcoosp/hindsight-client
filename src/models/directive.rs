//! Directives: hard rules injected into reflect prompts.

use serde::{Deserialize, Serialize};

pub(crate) fn default_true() -> bool {
    true
}

/// Create-directive request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateDirectiveRequest {
    /// Human-readable name.
    pub name: String,
    /// The directive text injected into prompts.
    pub content: String,
    /// Higher priority directives are injected first.
    #[serde(default)]
    pub priority: i64,
    /// Whether this directive is active.
    #[serde(default = "default_true")]
    pub is_active: bool,
    /// Execution scope: empty = global; non-empty requires matching request tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl CreateDirectiveRequest {
    /// A directive request.
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            priority: 0,
            is_active: true,
            tags: Vec::new(),
        }
    }

    /// Set priority (higher first).
    #[must_use]
    pub fn priority(mut self, p: i64) -> Self {
        self.priority = p;
        self
    }

    /// Start inactive.
    #[must_use]
    pub fn inactive(mut self) -> Self {
        self.is_active = false;
        self
    }

    /// Scope execution to tags.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }
}

/// Update-directive request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateDirectiveRequest {
    /// New name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// New priority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    /// New active flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
    /// New tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

impl UpdateDirectiveRequest {
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

    /// Change content.
    #[must_use]
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Change priority.
    #[must_use]
    pub fn priority(mut self, p: i64) -> Self {
        self.priority = Some(p);
        self
    }

    /// Toggle active.
    #[must_use]
    pub fn active(mut self, yes: bool) -> Self {
        self.is_active = Some(yes);
        self
    }

    /// Change tags.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item: Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }
}

/// A directive.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DirectiveResponse {
    /// Directive id.
    pub id: String,
    /// Owning bank.
    pub bank_id: String,
    /// Name.
    pub name: String,
    /// Content.
    pub content: String,
    /// Priority.
    #[serde(default)]
    pub priority: i64,
    /// Active flag.
    #[serde(default = "default_true")]
    pub is_active: bool,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Creation time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}
