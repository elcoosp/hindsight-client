# hindsight-client

Rust HTTP client for the Hindsight agent memory system.

## Usage

```rust
use hindsight_client::HindsightClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HindsightClient::new("http://localhost:8888")?;
    client.retain("my-bank", "Rust is awesome", None, None).await?;
    let memories = client.recall("my-bank", "Rust", Some(5)).await?;
    for mem in memories {
        println!("{} (score: {})", mem.content, mem.score);
    }
    Ok(())
}
```

## License

MIT
