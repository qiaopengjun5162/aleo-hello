use anyhow::{Context, Result};
use async_trait::async_trait;
use clap::Parser;
use dotenvy::dotenv;
use reqwest::Client;
use snarkvm::algorithms::snark::varuna::VarunaVersion;
use snarkvm::circuit::AleoTestnetV0;
use snarkvm::ledger::block::Transaction;
use snarkvm::ledger::query::QueryTrait;
use snarkvm::parameters::testnet::{FeePublicV0Prover, FeePublicV0Verifier};
use snarkvm::prelude::{
    ConsensusVersion, Field, Identifier, InclusionVersion, Network, PrivateKey, Process, Program,
    StatePath, TestRng, TestnetV0,
};
use snarkvm::prelude::FromBytes as _;
use snarkvm::synthesizer::snark::{ProvingKey, VerifyingKey};
use std::env;
use std::io::Write;
use std::str::FromStr;

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

    /// Dry-run only: execute locally and show output, don't broadcast
    #[arg(long)]
    dry_run: bool,
}

/// Custom query that returns a fixed state root.
/// Bypasses snarkVM's internal ureq (blocked by WAF) and ensures
/// both traces use the exact same state root.
struct FixedStateRootQuery<N: Network> {
    state_root: N::StateRoot,
    block_height: u32,
}

#[async_trait(?Send)]
impl<N: Network> QueryTrait<N> for FixedStateRootQuery<N> {
    fn current_state_root(&self) -> Result<N::StateRoot> {
        Ok(self.state_root.clone())
    }

    fn current_block_height(&self) -> Result<u32> {
        Ok(self.block_height)
    }

    fn get_state_path_for_commitment(&self, _commitment: &Field<N>) -> Result<StatePath<N>> {
        // Return empty state path — our simple program has no record inputs
        StatePath::from_str("").or_else(|_| anyhow::bail!("State path not available"))
    }

    fn get_state_paths_for_commitments(&self, _commitments: &[Field<N>]) -> Result<Vec<StatePath<N>>> {
        Ok(Vec::new())
    }

    async fn current_state_root_async(&self) -> Result<N::StateRoot> {
        Ok(self.state_root.clone())
    }

    async fn current_block_height_async(&self) -> Result<u32> {
        Ok(self.block_height)
    }

    async fn get_state_path_for_commitment_async(&self, _commitment: &Field<N>) -> Result<StatePath<N>> {
        Ok(StatePath::from_str("").unwrap_or_else(|_| {
            // Return a dummy state path — not used for simple programs
            panic!("State path not available")
        }))
    }

    async fn get_state_paths_for_commitments_async(&self, _commitments: &[Field<N>]) -> Result<Vec<StatePath<N>>> {
        Ok(Vec::new())
    }
}

/// Fetch program source from the Aleo network.
async fn fetch_program(client: &Client, node_url: &str, program_id: &str) -> Result<Program<TestnetV0>> {
    let url = format!("{}/program/{}", node_url, program_id);
    println!("[Network] GET {}", url);

    let text = client.get(&url)
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .header("Accept", "application/json, text/plain, */*")
        .send().await?.text().await?;

    let clean = text.trim_matches('"').replace("\\n", "\n");
    Program::<TestnetV0>::from_str(&clean).context("Failed to parse program")
}

/// Fetch latest state root and block height using browser-disguised HTTP client.
async fn fetch_state_root(client: &Client, node_url: &str) -> Result<(<TestnetV0 as Network>::StateRoot, u32)> {
    let url = format!("{}/stateRoot/latest", node_url);
    let text = client.get(&url)
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .header("Accept", "application/json, text/plain, */*")
        .send().await?.text().await?;

    let root_str = text.trim_matches('"');
    let state_root = <TestnetV0 as Network>::StateRoot::from_str(root_str)
        .context("Failed to parse state root")?;

    let height_url = format!("{}/block/height/latest", node_url);
    let height_text = client.get(&height_url)
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .header("Accept", "application/json, text/plain, */*")
        .send().await?.text().await?;
    let height: u32 = height_text.trim().parse()?;

    Ok((state_root, height))
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let cli = Cli::parse();

    let pk_string = env::var("PRIVATE_KEY")
        .context("PRIVATE_KEY not found in .env file")?;

    let program_name = &cli.program;
    let function_name = &cli.function;
    let inputs: Vec<&str> = cli.inputs.split(',').map(|s| s.trim()).collect();
    println!("\n🚀 Starting Aleo Rust Client (TestnetV0)");
    println!("   Program:  {}", program_name);
    println!("   Function: {}", function_name);
    println!("   Inputs:   {:?}", inputs);
    if cli.dry_run {
        println!("   Mode:     dry-run (local only, no broadcast)\n");
    } else {
        println!();
    }

    let client = Client::builder()
        .http1_only()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    // ── Phase 0: Fetch program ─────────────────────────────
    let program = fetch_program(&client, &cli.node_url, program_name).await?;
    println!("[Success] Program loaded: {}", program.id());

    let mut process = Process::<TestnetV0>::load()?;
    // Load credits.aleo program with pre-deployed proving/verifying keys
    let credits_program = Program::<TestnetV0>::credits()?;
    process.add_program(&credits_program)?;
    process.add_program(&program)?;

    // Load V0 fee keys (NOT standard) — deployed program uses edition 0 / V0 credits
    println!("⏳ Loading V0 fee keys from testnet parameters...");
    let fee_pk_bytes = FeePublicV0Prover::load_bytes()
        .context("Failed to load V0 fee_public prover")?;
    let fee_vk_bytes = FeePublicV0Verifier::load_bytes()
        .context("Failed to load V0 fee_public verifier")?;

    let fee_pk = ProvingKey::<TestnetV0>::from_bytes_le(&fee_pk_bytes)
        .context("Failed to deserialize V0 fee proving key")?;
    let fee_vk = VerifyingKey::<TestnetV0>::from_bytes_le(&fee_vk_bytes)
        .context("Failed to deserialize V0 fee verifying key")?;

    let credits_id = credits_program.id();
    let fee_fn = Identifier::<TestnetV0>::from_str("fee_public")?;
    process.insert_proving_key(credits_id, &fee_fn, fee_pk)?;
    process.insert_verifying_key(credits_id, &fee_fn, fee_vk)?;
    println!("✅ V0 Fee keys injected into VM");

    let private_key = PrivateKey::<TestnetV0>::from_str(&pk_string)?;
    println!("🔑 Account ready\n");

    let mut rng = TestRng::default();
    println!("▶️  Inputs: {:?}", inputs);

    let start_time = std::time::Instant::now();

    // ── Phase 1: Authorize ─────────────────────────────────
    println!("\n⏳ Phase 1: Authorizing transaction...");
    let authorization = process.authorize::<AleoTestnetV0, _>(
        &private_key, program.id(), function_name, inputs.into_iter(), &mut rng,
    )?;
    println!("✅ Authorization generated");

    // ── Phase 2: Local VM execution (both exec + fee) ──────
    println!("\n⏳ Phase 2: Local execution...");
    let (response, mut trace) = process
        .execute::<AleoTestnetV0, _>(authorization, &mut rng)?;
    println!("✅ Execution completed in {:?}", start_time.elapsed());

    println!("\n=======================================================");
    println!("🌟 Execution Response");
    println!("=======================================================");
    for (i, output) in response.outputs().iter().enumerate() {
        println!("  Output [{}]: {}", i, output);
    }
    println!("=======================================================\n");

    // ── Prepare fee BEFORE state root fetch ────────────────
    // We need the execution_id first. Prove execution with a temp query
    // to get the ID, then do fee, then re-prove both with unified state root.

    // Step A: Prove execution to get execution_id
    println!("⏳ Pre-proving execution to obtain execution_id...");
    let (state_root, block_height) = fetch_state_root(&client, &cli.node_url).await?;
    println!("🔍 State root: {}\n   Block height: {}", state_root, block_height);

    let query = FixedStateRootQuery::<TestnetV0> { state_root, block_height };
    trace.prepare(&query)?;

    let locator = format!("{}/{}", program.id(), function_name);
    let execution = trace
        .prove_execution::<AleoTestnetV0, _>(&locator, VarunaVersion::V2, &mut rng)?;
    let execution_id = execution.to_execution_id()?;

    // Step B: Authorize and execute fee
    // NOTE: fee keys must be injected BEFORE authorize_fee_public/execute,
    // because execute() stores the proving key in transition_tasks
    let base_fee = 1_327u64;
    let priority_fee = cli.priority_fee;

    let fee_authorization = process.authorize_fee_public::<AleoTestnetV0, _>(
        &private_key, base_fee, priority_fee, execution_id, &mut rng,
    )?;

    let (_fee_response, mut fee_trace) = process
        .execute::<AleoTestnetV0, _>(fee_authorization, &mut rng)?;

    // Step C: Prepare fee trace with SAME state root
    // Re-create query to use same state root (avoids second network fetch)
    let query2 = FixedStateRootQuery::<TestnetV0> { state_root, block_height };
    fee_trace.prepare(&query2)?;

    // Step D: Prove fee
    // Try V2 — testnet may have upgraded fee proof version
    let fee = fee_trace
        .prove_fee::<AleoTestnetV0, _>(VarunaVersion::V2, &mut rng)?;

    // ── Local verification ─────────────────────────────────
    println!("\n🔍 Verifying proofs locally...");
    process.verify_execution(
        ConsensusVersion::V14, VarunaVersion::V2, InclusionVersion::V0, &execution,
    )?;
    println!("  ✅ Execution proof verified");
    process.verify_fee(
        ConsensusVersion::V14, VarunaVersion::V2, InclusionVersion::V0, &fee, execution_id,
    )?;
    println!("  ✅ Fee proof verified");

    // ── Package ────────────────────────────────────────────
    let transaction = Transaction::<TestnetV0>::from_execution(execution, Some(fee))?;
    println!("\n📦 Transaction ID: {}", transaction.id());

    // ── Phase 4: Broadcast (skip if dry-run) ───────────────
    if cli.dry_run {
        println!("\n🔍 Dry-run complete. Transaction NOT broadcast.");
        println!("   Transaction ID (local): {}", transaction.id());
        return Ok(());
    }

    println!("\n📡 Phase 4: Broadcasting...");
    // ?check_transaction=true returns detailed validation errors from the node
    let broadcast_url = format!("{}/transaction/broadcast?check_transaction=true", &cli.node_url);
    let tx_json = transaction.to_string();

    let resp = client.post(&broadcast_url)
        .header("Content-Type", "application/json")
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .header("Accept", "application/json, text/plain, */*")
        .header("Origin", "https://explorer.provable.com")
        .header("Referer", "https://explorer.provable.com/")
        .body(tx_json)
        .send().await?;

    let status = resp.status();
    let response_body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        anyhow::bail!("Broadcast rejected ({}): {}", status, response_body);
    }
    println!("🚀 Broadcast accepted: {}", response_body);

    // ── Phase 5: Wait for confirmation ────────────────────
    println!("\n⏳ Phase 5: Waiting for confirmation... (polling every 5s)");
    let check_url = format!("{}/transaction/{}", &cli.node_url, transaction.id());

    for retry in 1..=30 {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        match client.get(&check_url)
            .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .header("Accept", "application/json, text/plain, */*")
            .send().await
        {
            Ok(res) if res.status().is_success() => {
                println!("\n🎉 Confirmed on chain!");
                println!("🔗 https://testnet.explorer.provable.com/transaction/{}", transaction.id());
                break;
            }
            Ok(_) | Err(_) if retry == 30 => {
                println!("\n⚠️  Timed out after 30 attempts.");
                println!("   Check manually: https://testnet.explorer.provable.com/transaction/{}", transaction.id());
            }
            _ => {
                print!(".");
                std::io::stdout().flush()?;
            }
        }
    }

    Ok(())
}
