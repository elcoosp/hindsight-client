//! Multi-tenant fan-out with bounded concurrency: recall across many user
//! banks without hand-rolled `futures` plumbing.

use hindsight_client::util::map_concurrent;
use hindsight_client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let user_banks = ["user-1", "user-2", "user-3", "user-4", "user-5"];

    // At most 4 in-flight requests; results keep input order.
    let shared = client.clone();
    let top_hits = map_concurrent(user_banks, 4, move |bank_id| {
        let client = shared.clone();
        async move {
            client
                .bank(bank_id)
                .recall_text("what did we decide last week?")
                .await
                .unwrap_or_default()
        }
    })
    .await;

    for (bank_id, ctx) in user_banks.iter().zip(top_hits) {
        println!("=== {bank_id} ===\n{ctx}");
    }
    Ok(())
}
