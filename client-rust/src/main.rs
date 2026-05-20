mod core;
mod network;
mod query;

use anyhow::{Context, Result};
use clap::Parser;
use dotenvy::dotenv;
use snarkvm::prelude::{PrivateKey, TestRng, TestnetV0};
use std::env;
use std::str::FromStr;

use crate::core::Engine;
use crate::network::{broadcast_transaction, build_client, fetch_program, fetch_state_root, wait_for_confirmation};
use crate::query::FixedStateRootQuery;

/// Aleo program execution client
#[derive(Parser)]
#[command(name = "aleo-execute", about = "Execute an Aleo program on testnet")]
struct Cli {
    /// Program name (e.g. hello_paxon_2026.aleo)
    #[arg(long, default_value = "hello_paxon_2026.aleo")]
    program: String,

    /// Function name
    #[arg(long, default_value = "main")]
    function: String,

    /// Inputs, comma-separated (e.g. "10u32,20u32")
    #[arg(long, default_value = "10u32,20u32")]
    inputs: String,

    /// Priority fee in microcredits
    #[arg(long, default_value_t = 100_000)]
    priority_fee: u64,

    /// Node URL
    #[arg(long, env = "NODE_URL", default_value = "https://api.provable.com/v2/testnet")]
    node_url: String,

    /// Dry-run only: execute locally, verify, don't broadcast
    #[arg(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let cli = Cli::parse();

    let pk_string = env::var("PRIVATE_KEY").context("PRIVATE_KEY not found in .env file")?;
    let inputs: Vec<&str> = cli.inputs.split(',').map(|s| s.trim()).collect();

    println!("\n🚀 Aleo Rust Client (TestnetV0)");
    println!("   Program:  {}", cli.program);
    println!("   Function: {}", cli.function);
    println!("   Inputs:   {:?}", inputs);
    if cli.dry_run { println!("   Mode:     dry-run\n"); } else { println!(); }

    // ── Setup ────────────────────────────────────────────────
    let client = build_client()?;
    let program = fetch_program(&client, &cli.node_url, &cli.program).await?;
    println!("[Success] Program loaded: {}", program.id());

    let mut engine = Engine::init(&program)?;
    let private_key = PrivateKey::<TestnetV0>::from_str(&pk_string)?;
    let mut rng = TestRng::default();

    // ── Execute & Prove ──────────────────────────────────────
    let start_time = std::time::Instant::now();
    let prog_id = program.id();
    let (_response, trace) = engine.authorize_and_execute(
        &private_key, prog_id, &cli.function, inputs, &mut rng,
    )?;
    println!("✅ Execution completed in {:?}", start_time.elapsed());

    let (state_root, block_height) = fetch_state_root(&client, &cli.node_url).await?;
    println!("🔍 State root: {}\n   Block height: {}", state_root, block_height);
    let query = FixedStateRootQuery::<TestnetV0> { state_root, block_height };

    let proving_start = std::time::Instant::now();
    let transaction = engine.prove_and_package(
        trace, &private_key, prog_id, &cli.function,
        1_327, cli.priority_fee, &query, &mut rng,
    )?;
    println!("✅ Proving + packaging done in {:?}", proving_start.elapsed());
    println!("📦 Transaction ID: {}", transaction.id());

    // ── Broadcast ────────────────────────────────────────────
    if cli.dry_run {
        println!("\n🔍 Dry-run complete. Transaction NOT broadcast.");
        return Ok(());
    }

    let tx_json = transaction.to_string();
    let response = broadcast_transaction(&client, &cli.node_url, tx_json).await?;
    println!("🚀 Broadcast accepted: {}", response);

    wait_for_confirmation(&client, &cli.node_url, &transaction.id().to_string()).await;
    Ok(())
}
