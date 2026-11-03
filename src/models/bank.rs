//! Bank models: lifecycle, config, aliases, stats, templates, consolidation.

use serde::{Deserialize, Serialize};

use super::common::{JsonMap, ObservationScopes, StringMap, TokenUsage};
use super::memory::ObservationScope;

/// A bank's personality: skepticism, literalism and empathy on 1–5 scales.
/// Shapes `reflect` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispositionTraits {
    /// 1 = trusting, 5 = skeptical.
    #[serde(default)]
    pub skepticism: i64,
    /// 1 = flexible, 5 = literal.
    #[serde(default)]
    pub literalism: i64,
    /// 1 = detached, 5 = empathetic.
    #[serde(default)]
    pub empathy: i64,
}

impl Default for DispositionTraits {
    fn default() -> Self {
        Self {
            skepticism: 3,
            literalism: 3,
            empathy: 3,
        }
    }
}

impl DispositionTraits {
    /// Balanced disposition (3/3/3).
    pub fn balanced() -> Self {
        Self::default()
    }

    /// A disposition from explicit traits.
    pub fn new(skepticism: i64, literalism: i64, empathy: i64) -> Self {
        Self {
            skepticism,
            literalism,
            empathy,
        }
    }
}

/// Bank-level overrides: retention/reflection missions, retrieval toggles.
/// Mirrors `CreateBankRequest`/`UpdateBankRequest`/`UpdateBankConfigRequest`
/// (the server accepts the same shape for create, patch and config update).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BankSettings {
    /// Human-readable name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Full disposition (all three traits).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition: Option<DispositionTraits>,
    /// Skepticism override (1–5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition_skepticism: Option<i64>,
    /// Literalism override (1–5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition_literalism: Option<i64>,
    /// Empathy override (1–5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition_empathy: Option<i64>,
    /// The bank's identity in plain language; shapes reflect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mission: Option<String>,
    /// Background knowledge merged into prompts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Mission used by reflect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflect_mission: Option<String>,
    /// Mission used by retain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_mission: Option<String>,
    /// Retain extraction mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_extraction_mode: Option<String>,
    /// Custom extraction instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_custom_instructions: Option<String>,
    /// Plain-text chunk size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_chunk_size: Option<i64>,
    /// Structured-content chunk size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_structured_chunk_size: Option<i64>,
    /// Maximum attachments per chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_max_attachments_per_chunk: Option<i64>,
    /// Enable observation synthesis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_observations: Option<bool>,
    /// Mission used for observation consolidation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observations_mission: Option<String>,
    /// Enable the keyword (BM25) retrieval arm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_text_search: Option<bool>,
    /// Enable the temporal retrieval arm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_temporal_retrieval: Option<bool>,
    /// Enable the graph retrieval arm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_graph_retrieval: Option<bool>,
    /// Enable the decision-model reranker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_reranking: Option<bool>,
}

impl BankSettings {
    /// Empty settings (server defaults everywhere).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the bank name.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the mission (shapes reflect).
    #[must_use]
    pub fn mission(mut self, mission: impl Into<String>) -> Self {
        self.mission = Some(mission.into());
        self
    }

    /// Set the retain mission.
    #[must_use]
    pub fn retain_mission(mut self, m: impl Into<String>) -> Self {
        self.retain_mission = Some(m.into());
        self
    }

    /// Set the reflect mission.
    #[must_use]
    pub fn reflect_mission(mut self, m: impl Into<String>) -> Self {
        self.reflect_mission = Some(m.into());
        self
    }

    /// Set the full disposition.
    #[must_use]
    pub fn disposition(mut self, d: DispositionTraits) -> Self {
        self.disposition = Some(d);
        self
    }

    /// Set one disposition trait (1–5).
    #[must_use]
    pub fn traits(mut self, skepticism: i64, literalism: i64, empathy: i64) -> Self {
        self.disposition = Some(DispositionTraits::new(skepticism, literalism, empathy));
        self
    }

    /// Toggle observation synthesis.
    #[must_use]
    pub fn enable_observations(mut self, yes: bool) -> Self {
        self.enable_observations = Some(yes);
        self
    }

    /// Toggle the keyword retrieval arm.
    #[must_use]
    pub fn enable_text_search(mut self, yes: bool) -> Self {
        self.enable_text_search = Some(yes);
        self
    }

    /// Toggle the temporal retrieval arm.
    #[must_use]
    pub fn enable_temporal_retrieval(mut self, yes: bool) -> Self {
        self.enable_temporal_retrieval = Some(yes);
        self
    }

    /// Toggle the graph retrieval arm.
    #[must_use]
    pub fn enable_graph_retrieval(mut self, yes: bool) -> Self {
        self.enable_graph_retrieval = Some(yes);
        self
    }

    /// Toggle the reranker.
    #[must_use]
    pub fn enable_reranking(mut self, yes: bool) -> Self {
        self.enable_reranking = Some(yes);
        self
    }
}

/// Body for creating or fully updating a bank.
pub type CreateBankRequest = BankSettings;

/// Body for a partial bank update (`PATCH`).
pub type UpdateBankRequest = BankSettings;

/// One bank in the list response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BankListItem {
    /// Bank id.
    pub bank_id: String,
    /// Display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Personality.
    pub disposition: DispositionTraits,
    /// Mission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission: Option<String>,
    /// Creation time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last update time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Number of memory units.
    #[serde(default)]
    pub fact_count: i64,
    /// Last document write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_document_at: Option<String>,
    /// Last write of any kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_write_at: Option<String>,
    /// Alias currently shown in place of the id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_alias: Option<String>,
    /// Aliases that matched a search `q`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_aliases: Option<Vec<String>>,
}

/// List banks response.
pub type BankListResponse = super::common::Page<BankListItem>;

/// Fully-resolved bank configuration.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BankConfigResponse {
    /// Bank id.
    pub bank_id: String,
    /// Full configuration with all overrides applied.
    pub config: JsonMap,
    /// Bank-specific overrides only.
    pub overrides: JsonMap,
}

/// Bank statistics (`GET .../stats`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BankStatsResponse {
    /// Bank id.
    pub bank_id: String,
    /// Total memory units.
    #[serde(default)]
    pub total_nodes: i64,
    /// Total links between memories.
    #[serde(default)]
    pub total_links: i64,
    /// Total documents.
    #[serde(default)]
    pub total_documents: i64,
    /// Total observations.
    #[serde(default)]
    pub total_observations: i64,
    /// Node counts by fact type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes_by_fact_type: Option<JsonMap>,
    /// Link counts by link type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links_by_link_type: Option<JsonMap>,
    /// Link counts by fact type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links_by_fact_type: Option<JsonMap>,
    /// Link breakdown blob.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links_breakdown: Option<serde_json::Value>,
    /// Async operations grouped by status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operations_by_status: Option<JsonMap>,
    /// Pending async operations.
    #[serde(default)]
    pub pending_operations: i64,
    /// Failed async operations.
    #[serde(default)]
    pub failed_operations: i64,
    /// Source memories still queued for consolidation.
    #[serde(default)]
    pub pending_consolidation: i64,
    /// Source memories whose consolidation permanently failed.
    #[serde(default)]
    pub failed_consolidation: i64,
    /// Last consolidation run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_consolidated_at: Option<String>,
    /// Last write of any kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_memory_write_at: Option<String>,
}

/// Update-bank-config body: overrides keyed in Python field format.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BankConfigUpdate {
    /// Configuration overrides.
    pub updates: JsonMap,
}

impl BankConfigUpdate {
    /// Overrides from a JSON map.
    pub fn new(updates: JsonMap) -> Self {
        Self { updates }
    }

    /// Overrides from raw JSON (must be an object).
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidInput`] when `value` is not a JSON object.
    pub fn from_json(value: serde_json::Value) -> crate::Result<Self> {
        match value {
            serde_json::Value::Object(map) => Ok(Self {
                updates: map.into_iter().collect(),
            }),
            other => Err(crate::Error::InvalidInput(format!(
                "bank config update must be a JSON object, got: {other}"
            ))),
        }
    }
}

/// One alias of a bank.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankAliasEntry {
    /// The alias.
    pub alias: String,
    /// Whether it is shown in place of the bank's own id (display only).
    #[serde(default)]
    pub primary: bool,
}

/// Aliases response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BankAliasesResponse {
    /// The bank's own id (an alias never replaces it).
    pub bank_id: String,
    /// Extra ids reaching this bank, primary first.
    pub aliases: Vec<BankAliasEntry>,
}

/// LLM health of one operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LlmOperationHealth {
    /// Operation name.
    pub operation: String,
    /// Whether the LLM call succeeded.
    pub ok: bool,
    /// Status detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Bank LLM health response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BankLlmHealthResponse {
    /// Bank id.
    pub bank_id: String,
    /// Per-operation health.
    pub operations: Vec<LlmOperationHealth>,
}

/// One timeseries bucket.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MemoryTimeseriesBucket {
    /// Bucket start (truncated to the period).
    pub time: String,
    /// Retained memories in this bucket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained: Option<i64>,
    /// Documents created in this bucket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<i64>,
}

/// Memory ingestion timeseries.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MemoriesTimeseriesResponse {
    /// Bank id.
    pub bank_id: String,
    /// Bucket period (e.g. `day`).
    pub period: String,
    /// The buckets.
    pub buckets: Vec<MemoryTimeseriesBucket>,
}

/// Profile response (mission/background/disposition).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BankProfileResponse {
    /// Bank id.
    pub bank_id: String,
    /// Display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Mission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission: Option<String>,
    /// Personality.
    pub disposition: DispositionTraits,
}

/// Set-background response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BackgroundResponse {
    /// Effective mission.
    pub mission: String,
    /// Effective background.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Effective disposition.
    pub disposition: DispositionTraits,
}

/// Trigger consolidation request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ConsolidationRequest {
    /// Only consolidate observations in these tag scopes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_scopes: Option<Vec<ObservationScope>>,
}

/// Trigger-consolidation response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConsolidationResponse {
    /// Id of the async consolidation operation.
    pub operation_id: String,
    /// True when an existing pending task was reused.
    #[serde(default)]
    pub deduplicated: bool,
}

/// Recover-consolidation response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RecoverConsolidationResponse {
    /// How many failed consolidations were requeued.
    pub retried_count: i64,
}

/// One consolidation strategy rule preview.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StrategyRulePreview {
    /// Rule description as the server renders it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Extra rule fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// One consolidation strategy preview.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StrategyPreview {
    /// Whether the strategy is active.
    pub active: bool,
    /// Observations the strategy claims.
    pub claimed_count: i64,
    /// Rendered rules.
    #[serde(default)]
    pub rules: Vec<StrategyRulePreview>,
}

/// Consolidation strategies preview response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConsolidationStrategiesPreview {
    /// Whether all scopes are covered.
    pub complete: bool,
    /// The default strategy preview.
    pub default: StrategyPreview,
    /// Scopes scanned.
    #[serde(default)]
    pub scopes_scanned: Vec<String>,
    /// Named strategies.
    #[serde(default)]
    pub strategies: JsonMap,
}

/// Bank template configuration section.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BankTemplateConfig {
    /// Reflect mission.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflect_mission: Option<String>,
    /// Retain mission.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_mission: Option<String>,
    /// Retain extraction mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_extraction_mode: Option<String>,
    /// Custom retain instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_custom_instructions: Option<String>,
    /// Plain-text chunk size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_chunk_size: Option<i64>,
    /// Structured-content chunk size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_structured_chunk_size: Option<i64>,
    /// Enable observations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_observations: Option<bool>,
    /// Observation mission.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observations_mission: Option<String>,
    /// Enable keyword search arm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_text_search: Option<bool>,
    /// Enable temporal arm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_temporal_retrieval: Option<bool>,
    /// Enable graph arm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_graph_retrieval: Option<bool>,
    /// Enable reranker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_reranking: Option<bool>,
    /// Disposition traits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition_skepticism: Option<i64>,
    /// Disposition literalism.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition_literalism: Option<i64>,
    /// Disposition empathy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition_empathy: Option<i64>,
    /// Extra config fields preserved from the export.
    #[serde(flatten, skip_serializing_if = "JsonMap::is_empty")]
    pub extra: JsonMap,
}

/// A mental model inside a bank template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BankTemplateMentalModel {
    /// Stable id (lowercase alphanumeric with hyphens).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Query used to (re)generate content.
    pub source_query: String,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Content token budget.
    #[serde(default = "default_mm_tokens")]
    pub max_tokens: i64,
    /// Refresh trigger configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<super::mental_model::MentalModelTrigger>,
}

fn default_mm_tokens() -> i64 {
    2048
}

/// A directive inside a bank template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankTemplateDirective {
    /// Name (match key on re-import).
    pub name: String,
    /// Directive text.
    pub content: String,
    /// Higher priority directives are injected first.
    #[serde(default)]
    pub priority: i64,
    /// Whether active.
    #[serde(default = "crate::models::directive::default_true")]
    pub is_active: bool,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// A portable bank template (config + mental models + directives).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BankTemplateManifest {
    /// Manifest schema version (currently `1`).
    pub version: String,
    /// Bank configuration section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bank: Option<BankTemplateConfig>,
    /// Mental models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mental_models: Option<Vec<BankTemplateMentalModel>>,
    /// Directives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directives: Option<Vec<BankTemplateDirective>>,
}

impl BankTemplateManifest {
    /// A v1 manifest with the given config.
    pub fn v1(bank: BankTemplateConfig) -> Self {
        Self {
            version: "1".into(),
            bank: Some(bank),
            mental_models: None,
            directives: None,
        }
    }
}

/// Result of importing a bank template.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BankTemplateImportResponse {
    /// Bank imported into.
    pub bank_id: String,
    /// Whether bank config was updated.
    pub config_applied: bool,
    /// Newly created mental models.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mental_models_created: Vec<String>,
    /// Updated mental models.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mental_models_updated: Vec<String>,
    /// Newly created directives (by name).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives_created: Vec<String>,
    /// Updated directives (by name).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives_updated: Vec<String>,
    /// Async content-generation operation ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operation_ids: Vec<String>,
    /// True when the import was validation-only.
    #[serde(default)]
    pub dry_run: bool,
}

/// Retain request variant used by the file-upload endpoint.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct FilesRetainMetadata {
    /// Context for this file.
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
    /// Observation scoping for this file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_scopes: Option<ObservationScopes>,
}

/// The JSON `request` part of the file-retain multipart body.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct FilesRetainRequest {
    /// Per-file metadata, in upload order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_metadata: Option<Vec<FilesRetainMetadata>>,
}

/// Token usage helper re-export for convenience.
pub type Usage = TokenUsage;
