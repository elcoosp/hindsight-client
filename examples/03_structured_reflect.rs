//! Type-safe structured reflect: the answer lands in your own struct.
//!
//! Run with: `cargo run --features schemars --example 03_structured_reflect`

use hindsight_client::{Client, ReflectRequest};
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct TripPlan {
    /// Destination city.
    destination: String,
    /// 3-5 day plan, one entry per day.
    itinerary: Vec<String>,
    /// Estimated budget in USD.
    budget_usd: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let bank = client.bank("travel-planner");

    bank.retain(["The user visited Kyoto in 2024 and loved the temples"])
        .await?;

    let plan: TripPlan = bank
        .reflect_structured(
            ReflectRequest::new("Plan a 3-day trip based on the user's travel history")
                .budget(hindsight_client::Budget::High),
        )
        .await?;

    println!("destination: {}", plan.destination);
    for (i, day) in plan.itinerary.iter().enumerate() {
        println!("day {}: {day}", i + 1);
    }
    println!("budget: ${:.0}", plan.budget_usd);
    Ok(())
}
