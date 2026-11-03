//! Shared types: enums, content blocks, include options, tag filters, usage.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Retrieval depth for `recall`/`reflect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Budget {
    /// Cheapest, narrowest retrieval.
    Low,
    /// Balanced (server default).
    Mid,
    /// Deepest, broadest retrieval.
    High,
}

impl std::fmt::Display for Budget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Budget::Low => write!(f, "low"),
            Budget::Mid => write!(f, "mid"),
            Budget::High => write!(f, "high"),
        }
    }
}

/// Fact type of a memory unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactType {
    /// General knowledge about people, places, events, things.
    World,
    /// Memories about experience: conversations, actions, tasks.
    Experience,
    /// Consolidated knowledge synthesized from facts.
    Observation,
}

impl std::fmt::Display for FactType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FactType::World => write!(f, "world"),
            FactType::Experience => write!(f, "experience"),
            FactType::Observation => write!(f, "observation"),
        }
    }
}

/// How a `tags` filter matches memories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagsMatch {
    /// OR — match any tag, includes untagged memories.
    Any,
    /// AND — match all tags, includes untagged memories.
    All,
    /// OR — match any tag, excludes untagged memories.
    AnyStrict,
    /// AND — match all tags, excludes untagged memories.
    AllStrict,
    /// Set equality on the full tag scope, excludes untagged memories.
    Exact,
}

/// Which time axis a list filter (and its ordering) uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeField {
    /// Ingest time.
    CreatedAt,
    /// Last-write time.
    UpdatedAt,
    /// When the fact was said/written.
    MentionedAt,
    /// Event-range start.
    OccurredStart,
    /// Event-range end.
    OccurredEnd,
}

/// Curation state of a memory unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryState {
    /// Active and recallable.
    Valid,
    /// Soft-retired: excluded from recall/consolidation, kept for audit.
    Invalidated,
}

/// How retain attaches observations to tags (`observation_scopes`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObservationScopes {
    /// A named scope strategy.
    Named(#[serde(with = "scope_name")] NamedScope),
    /// Explicit per-tag groups.
    Groups(Vec<Vec<String>>),
}

/// Named observation-scope strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedScope {
    /// One scope per tag.
    PerTag,
    /// One shared scope for all tags (default).
    Combined,
    /// One scope per tag combination.
    AllCombinations,
    /// A single global scope written as untagged.
    Shared,
}

mod scope_name {
    use serde::{Deserialize, Deserializer, Serializer};
    // The `&Self` signature is mandated by serde's `Serialize` trait.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S: Serializer>(v: &super::NamedScope, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(match v {
            super::NamedScope::PerTag => "per_tag",
            super::NamedScope::Combined => "combined",
            super::NamedScope::AllCombinations => "all_combinations",
            super::NamedScope::Shared => "shared",
        })
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<super::NamedScope, D::Error> {
        match String::deserialize(d)?.as_str() {
            "per_tag" => Ok(super::NamedScope::PerTag),
            "combined" => Ok(super::NamedScope::Combined),
            "all_combinations" => Ok(super::NamedScope::AllCombinations),
            "shared" => Ok(super::NamedScope::Shared),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["per_tag", "combined", "all_combinations", "shared"],
            )),
        }
    }
}

/// A content block inside multimodal [`Content`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text, placed inline where it appears.
    Text {
        /// The text.
        text: String,
    },
    /// A base64-encoded image.
    Image {
        /// Base64 attachment source.
        source: AttachmentSource,
    },
    /// A base64-encoded file (e.g. a PDF).
    File {
        /// Base64 attachment source.
        source: AttachmentSource,
        /// Optional display filename.
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
    },
}

/// Raw content to retain: either a plain string or an ordered list of blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    /// Plain text (the common case — serializes as a JSON string).
    Text(String),
    /// Multimodal blocks: text, images and files inline.
    Blocks(Vec<ContentBlock>),
}

impl From<&str> for Content {
    fn from(s: &str) -> Self {
        Content::Text(s.to_string())
    }
}

impl From<String> for Content {
    fn from(s: String) -> Self {
        Content::Text(s)
    }
}

impl From<Vec<ContentBlock>> for Content {
    fn from(b: Vec<ContentBlock>) -> Self {
        Content::Blocks(b)
    }
}

/// Base64-encoded attachment source (no `data:` URI prefix).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachmentSource {
    /// MIME type, e.g. `image/png`, `application/pdf`.
    pub media_type: String,
    /// Base64-encoded bytes.
    pub data: String,
}

impl AttachmentSource {
    /// Encode raw bytes as a base64 attachment source.
    ///
    /// # Panics
    /// Panics if the bytes cannot be encoded (cannot happen with the standard alphabet).
    pub fn from_bytes(media_type: impl Into<String>, bytes: &[u8]) -> Self {
        Self {
            media_type: media_type.into(),
            data: base64_encode(bytes),
        }
    }
}

/// Minimal standard base64 encoder (no external dependency).
pub(crate) fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// LLM token usage as reported by the server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input/prompt tokens consumed.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub input_tokens: i64,
    /// Visible output tokens (excludes reasoning tokens).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub output_tokens: i64,
    /// Total tokens (input + visible output).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub total_tokens: i64,
    /// Cached/cache-read prompt tokens, when reported.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cached_tokens: i64,
    /// Reasoning/thinking tokens, when reported.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub thoughts_tokens: i64,
}

// The `&Self` signature is mandated by serde's `skip_serializing_if`.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(v: &i64) -> bool {
    *v == 0
}

/// A caller-supplied entity hint for retain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityInput {
    /// Entity name/text.
    pub text: String,
    /// Optional label type (e.g. `PERSON`, `ORGANIZATION`, `CONCEPT`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl EntityInput {
    /// A new entity hint.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: None,
        }
    }

    /// A new entity hint with an explicit label type.
    pub fn typed(text: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: Some(kind.into()),
        }
    }
}

impl From<&str> for EntityInput {
    fn from(s: &str) -> Self {
        EntityInput::new(s)
    }
}

/// One FastAPI `422` validation error detail.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ValidationErrorDetail {
    /// Field path; entries are strings or integers rendered as strings.
    #[serde(default)]
    pub loc: Vec<serde_json::Value>,
    /// Human-readable message.
    pub msg: String,
    /// Error kind.
    #[serde(rename = "type")]
    pub kind: String,
}

/// Per-stage score floors for recall.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct MinScores {
    /// Retrieval-level cutoff for the semantic arm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic: Option<f64>,
    /// Retrieval-level cutoff for the keyword (BM25) arm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<f64>,
    /// Post-ranking floor applied to every returned result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reranker: Option<f64>,
    /// Final post-ranking floor applied to every returned result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_score: Option<f64>,
}

/// Explicit temporal window for recall's temporal arm (ranks, does not filter).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalWindow {
    /// Window start (inclusive, ISO 8601).
    pub start: String,
    /// Window end (inclusive, ISO 8601).
    pub end: String,
}

impl TemporalWindow {
    /// A new temporal window.
    pub fn new(start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
        }
    }
}

/// What a decision-model reranker does beyond ordering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RerankOptions {
    /// Drop results that do not help instead of only ranking them last.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune: Option<bool>,
    /// Drop results repeating a better-ranked one (one extra decision-model call).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedup: Option<bool>,
}

/// Include-options for `recall`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncludeOptions {
    /// Entity observations (enabled by default on the server).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<EntityIncludeOptions>,
    /// Raw source-text chunks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunks: Option<ChunkIncludeOptions>,
    /// Source facts for observation-type results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_facts: Option<SourceFactsIncludeOptions>,
}

/// Entity include tuning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityIncludeOptions {
    /// Entity data token budget (server default 500).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
}

/// Chunk include tuning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkIncludeOptions {
    /// Chunk token budget (server default 8192).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
}

/// Source-facts include tuning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFactsIncludeOptions {
    /// Total token budget across all observations (-1 = unlimited).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    /// Token budget per observation (-1 = unlimited).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens_per_observation: Option<i64>,
}

/// Include-options for `reflect`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectIncludeOptions {
    /// Include grounded facts in `based_on.memories`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facts: Option<FactsIncludeOptions>,
    /// Include the tool-call trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<ToolCallsIncludeOptions>,
}

/// Facts include tuning (presence toggles inclusion).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactsIncludeOptions {
    /// Reserved for future server options.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserved: Option<()>,
}

/// Tool-calls include tuning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallsIncludeOptions {
    /// Include tool outputs (false = inputs only, smaller trace).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<bool>,
}

/// A leaf tag filter: tags plus how to combine them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagGroupLeaf {
    /// Tags to match.
    pub tags: Vec<String>,
    /// Match mode.
    #[serde(rename = "match")]
    pub match_mode: TagsMatch,
    /// Resolve tags against the bank's tags by trigram similarity (typo tolerance).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve: Option<Resolve>,
}

/// Tag resolution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolve {
    /// Fuzzy-match tags via trigram similarity.
    Fuzzy,
}

/// Compound tag filter: leaves AND-ed/OR-ed/NOT-ed. A list of groups is AND-ed together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TagGroup {
    /// A leaf filter.
    Leaf(TagGroupLeaf),
    /// All sub-groups must match.
    And {
        /// Sub-groups.
        and: Vec<TagGroup>,
    },
    /// Any sub-group must match.
    Or {
        /// Sub-groups.
        or: Vec<TagGroup>,
    },
    /// The sub-group must not match.
    Not {
        /// Negated sub-group.
        not: Box<TagGroup>,
    },
}

impl TagGroup {
    /// A leaf group matching any of `tags`.
    pub fn any(tags: impl IntoIterator<Item: Into<String>>) -> TagGroup {
        TagGroup::Leaf(TagGroupLeaf {
            tags: tags.into_iter().map(Into::into).collect(),
            match_mode: TagsMatch::Any,
            resolve: None,
        })
    }

    /// A leaf group matching all of `tags`.
    pub fn all(tags: impl IntoIterator<Item: Into<String>>) -> TagGroup {
        TagGroup::Leaf(TagGroupLeaf {
            tags: tags.into_iter().map(Into::into).collect(),
            match_mode: TagsMatch::AllStrict,
            resolve: None,
        })
    }

    /// Make this leaf fuzzy-resolve tags (typo tolerance).
    #[must_use]
    pub fn fuzzy(mut self) -> TagGroup {
        if let TagGroup::Leaf(leaf) = &mut self {
            leaf.resolve = Some(Resolve::Fuzzy);
        }
        self
    }
}

impl From<&str> for TagGroup {
    fn from(s: &str) -> Self {
        TagGroup::any([s])
    }
}

/// Generic server-paginated list envelope: `{items, total, limit, offset}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Page<T> {
    /// Items on this page.
    pub items: Vec<T>,
    /// Total matching rows across all pages.
    pub total: i64,
    /// Page size requested.
    pub limit: i64,
    /// Offset requested.
    pub offset: i64,
}

impl<T> Page<T> {
    /// Items on this page.
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// Consume into the items vector.
    pub fn into_items(self) -> Vec<T> {
        self.items
    }

    /// Offset to request for the next page, or `None` when this page is the last.
    pub fn next_offset(&self) -> Option<i64> {
        let next = self.offset + self.items.len() as i64;
        (next < self.total).then_some(next)
    }
}

/// Convenience alias for string-keyed free-form maps the API sends
/// (`metadata`, config blobs, …).
pub type StringMap = BTreeMap<String, String>;

/// Convenience alias for free-form JSON maps the API sends.
pub type JsonMap = BTreeMap<String, serde_json::Value>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_text_serializes_as_plain_string() {
        let c = Content::from("hello");
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            serde_json::json!("hello")
        );
    }

    #[test]
    fn content_blocks_serialize_tagged() {
        let c = Content::Blocks(vec![
            ContentBlock::Text {
                text: "look".into(),
            },
            ContentBlock::Image {
                source: AttachmentSource::from_bytes("image/png", b"\x89PNG"),
            },
        ]);
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v[0]["type"], "text");
        assert_eq!(v[1]["type"], "image");
        assert_eq!(v[1]["source"]["media_type"], "image/png");
    }

    #[test]
    fn base64_encoder_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn tag_groups_roundtrip() {
        let g = TagGroup::And {
            and: vec![
                TagGroup::any(["a", "b"]).fuzzy(),
                TagGroup::Not {
                    not: Box::new(TagGroup::all(["x"])),
                },
            ],
        };
        let v = serde_json::to_value(&g).unwrap();
        assert!(v["and"].is_array());
        assert_eq!(v["and"][0]["match"], "any");
        let back: TagGroup = serde_json::from_value(v).unwrap();
        assert_eq!(back, g);
    }

    #[test]
    fn observation_scopes_roundtrip() {
        let s = ObservationScopes::Named(NamedScope::PerTag);
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            serde_json::json!("per_tag")
        );
        let g = ObservationScopes::Groups(vec![vec!["a".into()], vec!["b".into()]]);
        assert_eq!(
            serde_json::to_value(&g).unwrap(),
            serde_json::json!([["a"], ["b"]])
        );
    }

    #[test]
    fn page_next_offset() {
        let p: Page<u8> = Page {
            items: vec![1, 2],
            total: 5,
            limit: 2,
            offset: 2,
        };
        assert_eq!(p.next_offset(), Some(4));
        let p: Page<u8> = Page {
            items: vec![1, 2],
            total: 2,
            limit: 10,
            offset: 0,
        };
        assert_eq!(p.next_offset(), None);
    }
}
