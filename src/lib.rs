//! # hindsight-client
//!
//! Fast, typed, batteries-included async Rust client for the
//! [Hindsight](https://hindsight.vectorize.io) agent memory system
//! (self-hosted or [Hindsight Cloud](https://api.hindsight.vectorize.io)).
//!
//! Hindsight gives AI agents long-term memory: **retain** facts, **recall**
//! them with 4-way search (semantic + keyword + graph + temporal), and
//! **reflect** with an agentic loop that answers from the bank.
//!
//! ## Quick start
//!
//! ```no_run
//! use hindsight_client::Client;
//!
//! #[tokio::main]
//! async fn main() -> hindsight_client::Result<()> {
//!     let client = Client::from_env()?; // HINDSIGHT_API_URL + HINDSIGHT_API_KEY
//!     let bank = client.bank("my-bank");
//!
//!     bank.retain(["Alice works at Google on the AI team"]).await?;
//!     let answer = bank.reflect_text("Where does Alice work?").await?;
//!     println!("{answer}");
//!     Ok(())
//! }
//! ```
//!
//! ## Design highlights (see DESIGN.md for the full rationale)
//!
//! * **Typed everything** — every endpoint of the v1 API is a method with
//!   request builders; no positional `Option<&str>` soup.
//! * **`client.bank("id")`** — all 70+ bank-scoped calls on one handle,
//!   bank ids percent-encoded automatically.
//! * **Typed, matchable errors** — `Auth`, `NotFound`, `RateLimited`,
//!   `Api` (with parsed FastAPI validation details), `Timeout`, `StructuredOutput`.
//! * **Retries built in** — jittered backoff, `Retry-After` honored, writes
//!   opt-in (`retry_writes`).
//! * **Pagination helpers** — `*_paged` methods return a [`pagination::Pager`].
//! * **`recall_text`** — memories formatted as an LLM-ready context string.
//! * **`reflect_structured::<T>`** (feature `schemars`) — schema-guided,
//!   type-safe structured output.
//! * **Async pipelines** — `retain_async` + `wait_for_operation` with polling.
//!
//! ## Error handling
//!
//! ```no_run
//! use hindsight_client::{Client, Error};
//!
//! # async fn demo(client: &Client) {
//! match client.bank("nope").recall("anything").await {
//!     Ok(r) => println!("{} hits", r.results.len()),
//!     Err(Error::NotFound { message }) => eprintln!("bank missing: {message}"),
//!     Err(Error::RateLimited { retry_after }) => eprintln!("slow down: {retry_after:?}"),
//!     Err(e) => eprintln!("other: {e}"),
//! }
//! # }
//! ```
//!
//! ## Server requirement
//!
//! This crate talks to a running Hindsight server — see the
//! [installation guide](https://hindsight.vectorize.io/developer/installation)
//! (Docker, Helm, pip) or point it at Hindsight Cloud with an `hsk_…` key.

#![deny(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![warn(missing_docs)]

pub mod client;
pub mod error;
pub mod models;
pub mod pagination;
pub mod retry;
pub mod util;

pub use client::{
    BankClient, BankWriteResponse, Client, ClientBuilder, DocumentHistoryQuery,
    ExportDocumentsOptions, ImportBankOptions, KnowledgeSearchQuery, KnowledgeTreeQuery,
    ListBanksQuery, ListDirectivesQuery, ListDocumentsQuery, ListMentalModelsQuery, ListTagsQuery,
    LivenessResponse, LlmStatsQuery, OperationWait, PromptPreviewRequest, UploadedFile,
    VersionResponse, WebhookDeliveriesQuery, ENV_API_KEY, ENV_API_URL,
};
pub use error::{Error, Result};
pub use models::*;
pub use retry::RetryPolicy;
