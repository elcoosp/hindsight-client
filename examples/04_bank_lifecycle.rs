//! Bank lifecycle: create with mission/disposition, mental models, directives,
//! templates for environment promotion.

use hindsight_client::{
    BankSettings, Client, CreateDirectiveRequest, CreateMentalModelRequest, DispositionTraits,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;

    // 1. Create a bank with a personality.
    client
        .create_or_update_bank(
            "support-agent",
            BankSettings::new()
                .name("Support Agent")
                .mission("I am a support assistant; I prefer verified answers.")
                .disposition(DispositionTraits::new(4, 3, 5))
                .enable_observations(true),
        )
        .await?;

    let bank = client.bank("support-agent");

    // 2. A hard rule the reflect loop must obey.
    bank.create_directive(CreateDirectiveRequest::new(
        "no- refunds-policy",
        "Never promise refunds; direct the user to the billing team.",
    ))
    .await?;

    // 3. A curated summary refreshed automatically after consolidation.
    bank.create_mental_model(CreateMentalModelRequest::new(
        "open-issues",
        "What issues is the user currently facing?",
    ))
    .await?;

    // 4. Export the whole setup as a portable template and clone it to prod.
    let manifest = bank.export_bank_template().await?;
    println!("template version: {}", manifest.version);
    client
        .bank("support-agent-prod")
        .import_bank_template(manifest, false)
        .await?;
    Ok(())
}
