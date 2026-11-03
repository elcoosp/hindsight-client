//! Webhooks: registration and delivery tracking.

use serde::{Deserialize, Serialize};

use super::common::{JsonMap, Page};

/// HTTP delivery configuration of a webhook.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WebhookHttpConfig {
    /// HTTP method: GET or POST.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Delivery timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i64>,
    /// Custom headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<JsonMap>,
    /// Custom query parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<JsonMap>,
}

/// Register-webhook request.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreateWebhookRequest {
    /// HTTP(S) endpoint to deliver events to.
    pub url: String,
    /// HMAC secret used to sign deliveries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// Event types to deliver. Supported: `retain.completed`,
    /// `consolidation.completed`, `consolidation.failed`, …
    #[serde(default = "default_event_types", skip_serializing_if = "Vec::is_empty")]
    pub event_types: Vec<String>,
    /// Whether the webhook is active.
    #[serde(default = "crate::models::directive::default_true")]
    pub enabled: bool,
    /// HTTP delivery configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_config: Option<WebhookHttpConfig>,
}

fn default_event_types() -> Vec<String> {
    vec!["consolidation.completed".into()]
}

impl CreateWebhookRequest {
    /// A webhook for `url` delivering the default event type.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            secret: None,
            event_types: default_event_types(),
            enabled: true,
            http_config: None,
        }
    }

    /// Set the HMAC secret.
    #[must_use]
    pub fn secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// Set event types to deliver.
    #[must_use]
    pub fn event_types(mut self, types: impl IntoIterator<Item: Into<String>>) -> Self {
        self.event_types = types.into_iter().map(Into::into).collect();
        self
    }

    /// Start disabled.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set HTTP delivery configuration.
    #[must_use]
    pub fn http_config(mut self, config: WebhookHttpConfig) -> Self {
        self.http_config = Some(config);
        self
    }
}

/// Update-webhook request.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UpdateWebhookRequest {
    /// New URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// New secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// New event types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_types: Option<Vec<String>>,
    /// New active flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// New HTTP configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_config: Option<WebhookHttpConfig>,
}

impl UpdateWebhookRequest {
    /// An empty update.
    pub fn new() -> Self {
        Self::default()
    }

    /// Change the URL.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Change the secret.
    #[must_use]
    pub fn secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// Change event types.
    #[must_use]
    pub fn event_types(mut self, types: impl IntoIterator<Item: Into<String>>) -> Self {
        self.event_types = Some(types.into_iter().map(Into::into).collect());
        self
    }

    /// Toggle active.
    #[must_use]
    pub fn enabled(mut self, yes: bool) -> Self {
        self.enabled = Some(yes);
        self
    }
}

/// A registered webhook.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WebhookResponse {
    /// Webhook id.
    pub id: String,
    /// Owning bank.
    pub bank_id: String,
    /// Delivery URL.
    pub url: String,
    /// HMAC secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// Event types delivered.
    pub event_types: Vec<String>,
    /// Active flag.
    pub enabled: bool,
    /// HTTP configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_config: Option<WebhookHttpConfig>,
    /// Created (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Updated (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// One webhook delivery attempt record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WebhookDeliveryResponse {
    /// Delivery id.
    pub id: String,
    /// The webhook.
    pub webhook_id: String,
    /// Delivery URL.
    pub url: String,
    /// Event type delivered.
    pub event_type: String,
    /// Delivery status.
    pub status: String,
    /// Attempt count.
    #[serde(default)]
    pub attempts: i64,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: JsonMap,
}

/// Page of webhooks.
pub type WebhookPage = Page<WebhookResponse>;

/// Page of webhook deliveries.
pub type WebhookDeliveryPage = Page<WebhookDeliveryResponse>;
