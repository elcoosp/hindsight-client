use serde::{Deserialize, Serialize};

/// Request body for `retain`
#[derive(Debug, Serialize)]
pub struct RetainRequest {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>, // ISO 8601
}

/// A single memory hit from `recall` or `reflect`
#[derive(Debug, Deserialize, Clone)]
pub struct MemoryHit {
    pub content: String,
    pub score: f32,
    // Additional fields may be present; we ignore them for now.
}
