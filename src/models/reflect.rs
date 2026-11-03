//! Reflect models: request, response, citations and trace.

use serde::{Deserialize, Serialize};

use super::common::{
    Budget, FactType, JsonMap, ReflectIncludeOptions, TagGroup, TagsMatch, TokenUsage,
};
use super::memory::ChunkAttachment;

/// Reflect request body: `POST /v1/default/banks/{bank_id}/reflect`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectRequest {
    /// The question to answer.
    pub query: String,
    /// Retrieval budget (server default `low`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<Budget>,
    /// Additional context to prime the reflection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Maximum tokens for the response (server default 4096).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    /// Trace/include options.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<ReflectIncludeOptions>,
    /// JSON schema the answer's `structured_output` must satisfy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<serde_json::Value>,
    /// Tag scope for directives and mental models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// How `tags` are matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<TagsMatch>,
    /// Compound boolean tag filters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_groups: Option<Vec<TagGroup>>,
    /// Apply every active directive regardless of tag scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_all_directives: Option<bool>,
    /// Restrict the fact types the loop may retrieve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fact_types: Option<Vec<FactType>>,
    /// Exclude all mental models from the loop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_mental_models: Option<bool>,
    /// Exclude specific mental models from the loop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_mental_model_ids: Option<Vec<String>>,
    /// Token budget for the loop's observation search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflect_search_observations_max_tokens: Option<i64>,
    /// Include entity data in the loop's observation search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflect_search_observations_include_entities: Option<bool>,
    /// Token budget for the loop's mental-model search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflect_search_mental_models_max_tokens: Option<i64>,
}

impl ReflectRequest {
    /// A reflect request for `query`.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            budget: None,
            context: None,
            max_tokens: None,
            include: None,
            response_schema: None,
            tags: None,
            tags_match: None,
            tag_groups: None,
            apply_all_directives: None,
            fact_types: None,
            exclude_mental_models: None,
            exclude_mental_model_ids: None,
            reflect_search_observations_max_tokens: None,
            reflect_search_observations_include_entities: None,
            reflect_search_mental_models_max_tokens: None,
        }
    }

    /// Retrieval budget.
    #[must_use]
    pub fn budget(mut self, b: Budget) -> Self {
        self.budget = Some(b);
        self
    }

    /// Prime the reflection with extra context.
    #[must_use]
    pub fn context(mut self, ctx: impl Into<String>) -> Self {
        self.context = Some(ctx.into());
        self
    }

    /// Maximum response tokens.
    #[must_use]
    pub fn max_tokens(mut self, n: i64) -> Self {
        self.max_tokens = Some(n);
        self
    }

    /// Require the answer's `structured_output` to satisfy a JSON schema.
    #[must_use]
    pub fn response_schema(mut self, schema: serde_json::Value) -> Self {
        self.response_schema = Some(schema);
        self
    }

    /// Tag scope for directives and mental models.
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

    /// Apply every active directive regardless of tag scope.
    #[must_use]
    pub fn apply_all_directives(mut self, yes: bool) -> Self {
        self.apply_all_directives = Some(yes);
        self
    }

    /// Restrict retrievable fact types.
    #[must_use]
    pub fn fact_types(mut self, types: impl IntoIterator<Item = FactType>) -> Self {
        self.fact_types = Some(types.into_iter().collect());
        self
    }

    /// Exclude mental models from the loop (all, or specific ids).
    #[must_use]
    pub fn exclude_mental_models(mut self, ids: Option<Vec<String>>) -> Self {
        self.exclude_mental_models = Some(ids.as_ref().is_none() || ids.is_some());
        self.exclude_mental_model_ids = ids;
        self
    }

    /// Include the tool-call trace with outputs.
    #[must_use]
    pub fn include_tool_calls(mut self, with_output: bool) -> Self {
        self.include.get_or_insert_with(Default::default).tool_calls =
            Some(super::common::ToolCallsIncludeOptions {
                output: Some(with_output),
            });
        self
    }
}

impl From<&str> for ReflectRequest {
    fn from(q: &str) -> Self {
        ReflectRequest::new(q)
    }
}

impl From<String> for ReflectRequest {
    fn from(q: String) -> Self {
        ReflectRequest::new(q)
    }
}

/// Reflect response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ReflectResponse {
    /// The answer as formatted markdown.
    pub text: String,
    /// What the answer was grounded on.
    #[serde(default)]
    pub based_on: ReflectBasedOn,
    /// Structured output satisfying `response_schema`, when requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<serde_json::Value>,
    /// Why structured output failed, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output_error: Option<String>,
    /// Token usage.
    #[serde(default)]
    pub usage: TokenUsage,
    /// Tool-call trace, when requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<ReflectTrace>,
}

/// A memory fact cited by reflect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectFact {
    /// Fact id, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Fact text (markdown for observations).
    pub text: String,
    /// Fact type.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Context label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Event-range start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_start: Option<String>,
    /// Event-range end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_end: Option<String>,
    /// Mention time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentioned_at: Option<String>,
    /// Owning document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Owning chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_id: Option<String>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Free-form metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonMap>,
    /// Inline attachments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChunkAttachment>>,
}

/// A mental model cited by reflect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectMentalModel {
    /// Mental model id.
    pub id: String,
    /// Its content.
    pub text: String,
    /// Context, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// A directive applied during reflection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectDirective {
    /// Directive id.
    pub id: String,
    /// Directive name.
    pub name: String,
    /// Directive content.
    pub content: String,
}

/// The evidence reflect grounded its answer on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ReflectBasedOn {
    /// Memory facts used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memories: Vec<ReflectFact>,
    /// Mental models used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mental_models: Vec<ReflectMentalModel>,
    /// Directives applied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives: Vec<ReflectDirective>,
}

/// One tool call in the reflect trace.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ReflectToolCall {
    /// Tool name (`lookup`, `recall`, `learn`, `expand`, …).
    pub tool: String,
    /// Tool input parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    /// Tool output, when included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    /// Execution time in milliseconds.
    #[serde(default)]
    pub duration_ms: i64,
    /// 1-based loop iteration when the tool was called.
    #[serde(default)]
    pub iteration: i64,
}

/// One LLM call in the reflect trace.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReflectLlmCall {
    /// Call scope (`agent_1`, `final`, …).
    pub scope: String,
    /// Execution time in milliseconds.
    #[serde(default)]
    pub duration_ms: i64,
}

/// The reflect agentic-loop trace.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ReflectTrace {
    /// Tool calls made during reflection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ReflectToolCall>,
    /// LLM calls made during reflection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub llm_calls: Vec<ReflectLlmCall>,
}
