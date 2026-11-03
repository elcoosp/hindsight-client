//! The agent memory loop: recall before answering, retain after.
//!
//! `recall_text` gives you an LLM-ready context string; `retain_async` +
//! `wait_for_operation` keeps the write off the hot path.

use hindsight_client::{Client, MemoryItem, OperationWait};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let bank = client.bank("agent-session-42");

    // Before the LLM call: pull memories into the prompt.
    let context = bank
        .recall(
            hindsight_client::RecallRequest::new("user preferences for code review")
                .budget(hindsight_client::Budget::Mid)
                .max_tokens(2048)
                .prefer_observations(true),
        )
        .await?
        .to_context_string();
    println!("--- prompt context ---\n{context}");

    // ... call your LLM with `context` in the system prompt ...

    // After the turn: store what happened without blocking the next turn.
    let submitted = bank
        .retain([MemoryItem::new("User asked to shorten PR descriptions")
            .context("code review session")
            .tags(["preference"])])
        .await?;
    let op = bank
        .wait_for_operation(
            submitted.operation_id.unwrap_or_else(|| "noop".into()),
            OperationWait::new(),
        )
        .await;
    if let Ok(op) = op {
        println!("retain finished in state: {}", op.status);
    }
    Ok(())
}
