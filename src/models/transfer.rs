//! Transfer models: bank/document export & import, files, attachments.

use serde::{Deserialize, Serialize};

use super::common::StringMap;

/// Bank export submission response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BankTransferSubmitResponse {
    /// Async operation id (an export archive download key is in its result).
    pub operation_id: String,
    /// Initial status.
    #[serde(default)]
    pub status: String,
}

/// Document export submission response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DocumentExportSubmitResponse {
    /// Async operation id.
    pub operation_id: String,
    /// Initial status.
    #[serde(default)]
    pub status: String,
}

/// Document import submission response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DocumentImportSubmitResponse {
    /// Async operation id.
    pub operation_id: String,
    /// Initial status.
    #[serde(default)]
    pub status: String,
}

/// File-retain response: operation ids to poll.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileRetainResponse {
    /// One operation id per uploaded file.
    pub operation_ids: Vec<String>,
}

/// Clone-bank request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CloneBankRequest {
    /// Target bank id for the clone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_bank_id: Option<String>,
    /// Human-readable name for the clone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl CloneBankRequest {
    /// A clone request with a target bank id.
    pub fn new(target_bank_id: impl Into<String>) -> Self {
        Self {
            target_bank_id: Some(target_bank_id.into()),
            name: None,
        }
    }

    /// Name the clone.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

/// Metadata per uploaded file (multipart `files/retain`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FileMetadata {
    /// Context label for this file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Document id for this file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Tags for this file's document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Metadata for this file's document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<StringMap>,
}
