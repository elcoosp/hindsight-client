//! Typed, matchable error type for every failure mode the client can produce.

use std::time::Duration;

use reqwest::StatusCode;

use crate::models::ValidationErrorDetail;

/// The error type returned by every fallible operation in this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The HTTP transport failed (DNS, connection reset, TLS, …).
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),

    /// The request exceeded its configured timeout.
    #[error("request timed out")]
    Timeout,

    /// The configured base URL or a derived URL was invalid.
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),

    /// The server rejected the credentials (`401`/`403`).
    /// Check the API key — Hindsight keys start with `hsk_`.
    #[error("authentication failed ({}): {}", .status, .message)]
    Auth {
        /// HTTP status (`401` or `403`).
        status: StatusCode,
        /// Server-provided message.
        message: String,
    },

    /// The targeted resource does not exist (`404`).
    #[error("not found (404): {message}")]
    NotFound {
        /// Server-provided message.
        message: String,
    },

    /// The server is rate limiting us (`429`) and retries were exhausted
    /// (or disabled for this request).
    #[error("rate limited (429){}", .retry_after.map(|d| format!("; retry after {d:?}")).unwrap_or_default())]
    RateLimited {
        /// `Retry-After` advertised by the server, if any.
        retry_after: Option<Duration>,
    },

    /// Any other non-success status from the API.
    #[error("API error ({}): {}", .status, .message)]
    Api {
        /// HTTP status.
        status: StatusCode,
        /// Best-effort message extracted from the body.
        message: String,
        /// Parsed `detail` field, when the body is a JSON error
        /// (FastAPI validation errors carry a list of [`ValidationErrorDetail`]-shaped objects).
        detail: Option<serde_json::Value>,
    },

    /// The response body could not be decoded as the expected type.
    #[error("failed to decode response: {0}")]
    Json(#[from] serde_json::Error),

    /// The client rejected the input before sending anything.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Every retry attempt failed; the last error is attached.
    #[error("retries exhausted after {} attempt(s): {}", .attempts, .last)]
    RetriesExhausted {
        /// Total attempts made (initial call included).
        attempts: u32,
        /// The final underlying error.
        last: Box<Error>,
    },

    /// `reflect` was asked for structured output but the model failed to
    /// produce it (server returned `structured_output_error`, or the JSON did
    /// not deserialize into the target type).
    #[error("structured output failure: {message}")]
    StructuredOutput {
        /// Why the structured output failed.
        message: String,
        /// Raw `structured_output` value, if the server returned one.
        value: Option<serde_json::Value>,
    },
}

/// One entry of a FastAPI `422` validation-error body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    /// Path of the offending field, e.g. `["body", "items", 0, "content"]`.
    pub loc: Vec<String>,
    /// Human-readable validation message.
    pub msg: String,
    /// Validation error kind (e.g. `missing`, `string_too_short`).
    pub kind: String,
}

impl std::fmt::Display for FieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (at {}): type {}",
            self.msg,
            self.loc.join("."),
            self.kind
        )
    }
}

impl Error {
    /// HTTP status code carried by this error, when it came from a response.
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Error::Auth { status, .. } | Error::Api { status, .. } => Some(*status),
            Error::NotFound { .. } => Some(StatusCode::NOT_FOUND),
            Error::RateLimited { .. } => Some(StatusCode::TOO_MANY_REQUESTS),
            _ => None,
        }
    }

    /// `true` when this error is [`Error::NotFound`] — useful for upsert flows.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound { .. })
    }

    /// `true` when this error is [`Error::RateLimited`].
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, Error::RateLimited { .. })
    }

    /// `true` when the error is a client-side [`Error::InvalidInput`].
    pub fn is_invalid_input(&self) -> bool {
        matches!(self, Error::InvalidInput(_))
    }

    /// Build an [`Error::Api`]-family error from a status + body text,
    /// choosing the most specific variant (`Auth`/`NotFound`/`RateLimited`/`Api`).
    pub(crate) fn from_response(status: StatusCode, body: &str) -> Error {
        let (message, detail) = parse_error_body(body);
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Error::Auth { status, message },
            StatusCode::NOT_FOUND => Error::NotFound { message },
            StatusCode::TOO_MANY_REQUESTS => Error::RateLimited { retry_after: None },
            _ => Error::Api {
                status,
                message,
                detail,
            },
        }
    }

    /// Convert FastAPI validation details into typed [`FieldError`]s, when present.
    pub fn validation_errors(&self) -> Vec<FieldError> {
        match self {
            Error::Api {
                detail: Some(serde_json::Value::Array(items)),
                ..
            } => items
                .iter()
                .filter_map(|v| serde_json::from_value::<ValidationErrorDetail>(v.clone()).ok())
                .map(|d| FieldError {
                    loc: d
                        .loc
                        .into_iter()
                        .map(|part| match part {
                            serde_json::Value::String(s) => s,
                            other => other.to_string(),
                        })
                        .collect(),
                    msg: d.msg,
                    kind: d.kind,
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// Crate-wide result alias: every fallible method returns this.
pub type Result<T> = std::result::Result<T, Error>;

fn parse_error_body(body: &str) -> (String, Option<serde_json::Value>) {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(body) else {
        let message = if body.is_empty() {
            "(empty response body)".to_string()
        } else {
            body.chars().take(200).collect()
        };
        return (message, None);
    };
    let detail = map.get("detail").cloned();
    let message = match &detail {
        Some(serde_json::Value::String(m)) => m.clone(),
        Some(serde_json::Value::Array(items)) => {
            // FastAPI validation array — summarize the first entries.
            let summary = items
                .iter()
                .filter_map(|i| {
                    let msg = i.get("msg")?.as_str()?;
                    let loc: Vec<String> = i
                        .get("loc")
                        .and_then(|l| l.as_array())
                        .map(|a| {
                            a.iter()
                                .map(|p| match p {
                                    serde_json::Value::Number(n) => n.to_string(),
                                    serde_json::Value::String(s) => s.clone(),
                                    other => other.to_string(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    Some(format!("{}: {msg}", loc.join(".")))
                })
                .take(5)
                .collect::<Vec<_>>()
                .join("; ");
            if summary.is_empty() {
                "validation error".to_string()
            } else {
                summary
            }
        }
        Some(other) => other.to_string(),
        None => body.chars().take(200).collect(),
    };
    (message, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fastapi_validation_array_is_summarized() {
        let body = r#"{"detail":[{"loc":["body","items"],"msg":"field required","type":"missing"},{"loc":["body","query"],"msg":"field required","type":"missing"}]}"#;
        let (message, detail) = parse_error_body(body);
        assert!(message.contains("body.items: field required"), "{message}");
        assert!(detail.is_some());
    }

    #[test]
    fn plain_text_body_becomes_message() {
        let (message, detail) = parse_error_body("internal boom");
        assert_eq!(message, "internal boom");
        assert!(detail.is_none());
    }

    #[test]
    fn from_response_maps_specific_statuses() {
        assert!(matches!(
            Error::from_response(StatusCode::UNAUTHORIZED, "nope"),
            Error::Auth { .. }
        ));
        assert!(matches!(
            Error::from_response(StatusCode::NOT_FOUND, "bank gone"),
            Error::NotFound { .. }
        ));
        assert!(matches!(
            Error::from_response(StatusCode::TOO_MANY_REQUESTS, "slow down"),
            Error::RateLimited { .. }
        ));
    }
}
