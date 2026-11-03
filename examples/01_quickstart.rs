//! Quickstart: retain, recall, reflect.
//!
//! Run with: `cargo run --example 01_quickstart`
//! Requires a Hindsight server (`HINDSIGHT_API_URL`, optional `HINDSIGHT_API_KEY`).

use hindsight_client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let bank = client.bank("quickstart");

    bank.retain([
        "Alice works at Google on the AI team",
        "Alice prefers functional programming",
    ])
    .await?;

    let hits = bank.recall("Where does Alice work?").await?;
    println!("recall:\n{}", hits.to_context_string());

    let answer = bank
        .reflect_text("Summarize what you know about Alice.")
        .await?;
    println!("reflect:\n{answer}");
    Ok(())
}
