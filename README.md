<div align="center">
  <img src="docs/logo.png" alt="hindsight-client Logo" width="300"/>
  <p>
    <strong>Hindsight Client</strong><br/>
    A high-performance, asynchronous Rust HTTP client for the <a href="https://hindsight.vectorize.io/">Hindsight</a> agent memory system.
  </p>
  <p>
    <img src="https://img.shields.io/badge/Rust-1.94%20%7C%202024-000000?style=flat-square&logo=rust" alt="Rust"/>
    <img src="https://img.shields.io/badge/License-MIT-blue?style=flat-square" alt="License MIT"/>
    <img src="https://img.shields.io/badge/Async-Tokio-000000?style=flat-square&logo=tokio" alt="Tokio"/>
  </p>
</div>

---

`hindsight-client` provides a seamless, type-safe interface for interacting with the Hindsight agent memory system from Rust applications. It enables AI agents and applications to easily store (`retain`) and retrieve (`recall`) contextual memories with semantic scoring.

## Features

- **Asynchronous by Design**: Built on `tokio` for non-blocking, high-performance I/O.
- **Semantic Memory Operations**: Simple APIs for retaining new memories and recalling relevant ones with similarity scores.
- **Flexible Queries**: Support for limiting result counts and custom query parameters.
- **Ergonomic Error Handling**: Returns standard Rust `Result` types for easy integration into existing error-handling flows.

## Installation

Add `hindsight-client` to your `Cargo.toml` dependencies:

```toml
[dependencies]
hindsight-client = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Usage

Ensure your Hindsight server is running and accessible, then initialize the client to start managing agent memory.

```rust
use hindsight_client::HindsightClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the client with the Hindsight server endpoint
    let client = HindsightClient::new("http://localhost:8888")?;

    // Retain a new memory in a specific namespace (e.g., "my-bank")
    client.retain("my-bank", "Rust is awesome", None, None).await?;

    // Recall up to 5 memories related to the query "Rust"
    let memories = client.recall("my-bank", "Rust", Some(5)).await?;

    // Process the retrieved memories
    for mem in memories {
        println!("{} (score: {})", mem.content, mem.score);
    }

    Ok(())
}
```

> [!NOTE]
> **Server Requirement**  
> This crate is a client library. It requires a running instance of the Hindsight memory server. Refer to the [Hindsight documentation](https://hindsight.vectorize.io/) for server setup and configuration instructions.

## API Overview

| Method | Description |
| :--- | :--- |
| `HindsightClient::new(url)` | Constructs a new client instance targeting the specified Hindsight server URL. |
| `retain(namespace, content, metadata, tags)` | Stores a new memory fragment in the specified namespace. Optional metadata and tags can be attached. |
| `recall(namespace, query, limit)` | Searches the namespace for memories matching the query, returning a scored list of results up to the specified limit. |

> [!TIP]
> **Namespaces**  
> Use namespaces (like `"my-bank"` in the example) to logically separate memory contexts for different users, sessions, or application domains, preventing cross-contamination of agent knowledge.

---

*For vulnerability reporting and security posture, please refer to the project's security guidelines. This project is distributed under the terms of the MIT License.*
