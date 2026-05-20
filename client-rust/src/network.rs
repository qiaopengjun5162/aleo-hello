use anyhow::{Context, Result};
use reqwest::Client;
use snarkvm::prelude::{Network, Program, TestnetV0};
use std::io::Write;
use std::str::FromStr;

/// Browser-like User-Agent to bypass Cloudflare WAF.
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// Build a shared HTTP client with browser-like fingerprint.
pub(crate) fn build_client() -> Result<Client> {
    Client::builder()
        .http1_only()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .context("Failed to build HTTP client")
}

/// Fetch program source from the Aleo network.
pub(crate) async fn fetch_program(client: &Client, node_url: &str, program_id: &str) -> Result<Program<TestnetV0>> {
    let url = format!("{}/program/{}", node_url, program_id);
    println!("[Network] GET {}", url);

    let text = client.get(&url)
        .header("User-Agent", UA)
        .header("Accept", "application/json, text/plain, */*")
        .send().await?.text().await?;

    let clean = text.trim_matches('"').replace("\\n", "\n");
    Program::<TestnetV0>::from_str(&clean).context("Failed to parse program")
}

/// Fetch latest state root and block height using browser-disguised HTTP client.
pub(crate) async fn fetch_state_root(client: &Client, node_url: &str) -> Result<(<TestnetV0 as Network>::StateRoot, u32)> {
    let url = format!("{}/stateRoot/latest", node_url);
    let text = client.get(&url)
        .header("User-Agent", UA)
        .header("Accept", "application/json, text/plain, */*")
        .send().await?.text().await?;

    let root_str = text.trim_matches('"');
    let state_root = <TestnetV0 as Network>::StateRoot::from_str(root_str)
        .context("Failed to parse state root")?;

    let height_url = format!("{}/block/height/latest", node_url);
    let height_text = client.get(&height_url)
        .header("User-Agent", UA)
        .header("Accept", "application/json, text/plain, */*")
        .send().await?.text().await?;
    let height: u32 = height_text.trim().parse()?;

    Ok((state_root, height))
}

/// Broadcast a transaction and return the response body.
pub(crate) async fn broadcast_transaction(client: &Client, node_url: &str, tx_json: String) -> Result<String> {
    let broadcast_url = format!("{}/transaction/broadcast?check_transaction=true", node_url);
    println!("📡 Broadcasting to network...");

    let resp = client.post(&broadcast_url)
        .header("Content-Type", "application/json")
        .header("User-Agent", UA)
        .header("Accept", "application/json, text/plain, */*")
        .header("Origin", "https://explorer.provable.com")
        .header("Referer", "https://explorer.provable.com/")
        .body(tx_json)
        .send().await?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        anyhow::bail!("Broadcast rejected ({}): {}", status, body);
    }
    Ok(body)
}

/// Poll for transaction confirmation.
pub(crate) async fn wait_for_confirmation(client: &Client, node_url: &str, tx_id: &str) {
    let check_url = format!("{}/transaction/{}", node_url, tx_id);
    println!("⏳ Waiting for confirmation... (polling every 5s)");

    for retry in 1..=30 {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        match client.get(&check_url)
            .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .header("Accept", "application/json, text/plain, */*")
            .send().await
        {
            Ok(res) if res.status().is_success() => {
                println!("\n🎉 Confirmed on chain!");
                println!("🔗 https://testnet.explorer.provable.com/transaction/{}", tx_id);
                return;
            }
            Ok(_) | Err(_) if retry == 30 => {
                println!("\n⚠️  Timed out after 30 attempts.");
                println!("   Check manually: https://testnet.explorer.provable.com/transaction/{}", tx_id);
            }
            _ => {
                print!(".");
                std::io::stdout().flush().ok();
            }
        }
    }
}
