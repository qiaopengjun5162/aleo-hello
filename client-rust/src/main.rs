use anyhow::{Context, Result};
use dotenvy::dotenv;
use reqwest;
use snarkvm::algorithms::snark::varuna::VarunaVersion;
use snarkvm::circuit::AleoTestnetV0;
use snarkvm::ledger::block::Transaction;
use snarkvm::ledger::query::Query;
use snarkvm::ledger::store::helpers::memory::BlockMemory;
use snarkvm::prelude::{PrivateKey, Process, Program, TestRng, TestnetV0};
use std::env;
use std::io::Write;
use std::str::FromStr;

/// Fetch program source from the Aleo network and parse into a snarkVM Program object.
async fn fetch_program(node_url: &str, program_id: &str) -> Result<Program<TestnetV0>> {
    let url = format!("{}/program/{}", node_url, program_id);
    println!("[Network] GET {}", url);

    let response_text = reqwest::get(&url)
        .await
        .context("Failed to fetch program from network")?
        .text()
        .await
        .context("Failed to read response body")?;

    // The API returns the program source as a JSON string with escaped newlines
    let clean_text = response_text.trim_matches('"').replace("\\n", "\n");
    Program::<TestnetV0>::from_str(&clean_text).context("Failed to parse program source")
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // ── Configuration ───────────────────────────────────────
    // Two separate URLs are REQUIRED:
    //   node_url  → for manual reqwest HTTP requests (with /testnet path)
    //   v2_base   → for snarkVM Query (PURE base URL, snarkVM auto-appends /testnet)
    let node_url =
        env::var("NODE_URL").unwrap_or_else(|_| "https://api.provable.com/v2/testnet".to_string());
    let v2_base = "https://api.provable.com/v2";

    let pk_string =
        env::var("PRIVATE_KEY").context("PRIVATE_KEY not found in .env file")?;

    let program_name = "hello_paxon_2026.aleo";
    let function_name = "main";
    println!("\n🚀 Starting Aleo Rust Client (TestnetV0)\n");

    // ── Phase 0: Fetch program & init VM ────────────────────
    let program = fetch_program(&node_url, program_name).await?;
    println!("[Success] Program loaded: {}", program.id());

    let mut process = Process::<TestnetV0>::load_v0().context("Failed to init Process")?;
    process
        .add_program(&program)
        .context("Failed to add program to Process")?;

    let private_key =
        PrivateKey::<TestnetV0>::from_str(&pk_string).context("Invalid private key")?;
    println!("🔑 Account ready\n");

    let mut rng = TestRng::default();
    let inputs = vec!["10u32", "20u32"];
    println!("▶️  Inputs: {:?}", inputs);

    let start_time = std::time::Instant::now();

    // ── Phase 1: Authorize (dry-run logic check) ────────────
    println!("\n⏳ Phase 1: Authorizing transaction...");
    let authorization = process
        .authorize::<AleoTestnetV0, _>(
            &private_key,
            program.id(),
            function_name,
            inputs.into_iter(),
            &mut rng,
        )
        .context("Authorization failed — check input types")?;
    println!("✅ Authorization generated");

    // ── Phase 2: Local execution (no proof yet) ─────────────
    println!("\n⏳ Phase 2: Local execution...");
    let (response, mut trace) = process
        .execute::<AleoTestnetV0, _>(authorization, &mut rng)
        .context("Local execution failed")?;
    println!("✅ Execution completed in {:?}", start_time.elapsed());

    println!("\n=======================================================");
    println!("🌟 Execution Response");
    println!("=======================================================");
    for (i, output) in response.outputs().iter().enumerate() {
        println!("  Output [{}]: {}", i, output);
    }
    println!("=======================================================\n");

    // ── Phase 3: ZK Proving (CPU intensive) ─────────────────
    println!("=======================================================");
    println!("🔥 Phase 3: ZK Proving & Packaging");
    println!("=======================================================");
    let proving_start = std::time::Instant::now();

    // Query uses PURE v2 base URL — snarkVM auto-appends /testnet internally.
    // DO NOT pass v2/testnet here, or it becomes v2/testnet/testnet/...
    println!("\n🔍 Fetching state root from network...");
    let uri = v2_base
        .parse::<http::Uri>()
        .context("Failed to parse API base URL")?;
    let query = Query::<TestnetV0, BlockMemory<_>>::from(uri);

    // --- Execution Proof ---
    trace
        .prepare(&query)
        .context("Failed to prepare execution trace")?;

    // locator format: "{program_id}/{function_name}" (e.g. "hello_paxon_2026.aleo/main")
    let locator = format!("{}/{}", program.id(), function_name);
    let execution = trace
        .prove_execution::<AleoTestnetV0, _>(
            &locator,
            VarunaVersion::V1,
            &mut rng,
        )
        .context("Failed to generate execution proof")?;
    println!(
        "✅ Execution proof generated in {:?}",
        proving_start.elapsed()
    );

    // --- Fee Proof ---
    println!("\n⏳ Generating fee proof...");
    let fee_start = std::time::Instant::now();

    // Fee MUST exactly match circuit constraints — 1,327 is the precise cost for this program
    let base_fee = 1_327u64;
    let priority_fee = 1_000u64;

    let execution_id = execution
        .to_execution_id()
        .context("Failed to get execution ID")?;

    let fee_authorization = process
        .authorize_fee_public::<AleoTestnetV0, _>(
            &private_key,
            base_fee,
            priority_fee,
            execution_id,
            &mut rng,
        )
        .context("Failed to authorize fee")?;

    let (_fee_response, mut fee_trace) = process
        .execute::<AleoTestnetV0, _>(fee_authorization, &mut rng)
        .context("Failed to execute fee")?;

    fee_trace
        .prepare(&query)
        .context("Failed to prepare fee trace")?;

    let fee = fee_trace
        .prove_fee::<AleoTestnetV0, _>(VarunaVersion::V1, &mut rng)
        .context("Failed to generate fee proof")?;
    println!("✅ Fee proof generated in {:?}", fee_start.elapsed());

    // --- Local verification (like Python SDK's verify_execution/verify_fee) ---
    use snarkvm::prelude::{ConsensusVersion, InclusionVersion};
    println!("\n🔍 Verifying proofs locally...");
    process
        .verify_execution(
            ConsensusVersion::V1,
            VarunaVersion::V1,
            InclusionVersion::V0,
            &execution,
        )
        .context("Local execution verification FAILED — proof is invalid")?;
    println!("  ✅ Execution proof verified locally");

    process
        .verify_fee(
            ConsensusVersion::V1,
            VarunaVersion::V1,
            InclusionVersion::V0,
            &fee,
            execution_id,
        )
        .context("Local fee verification FAILED — proof is invalid")?;
    println!("  ✅ Fee proof verified locally");

    // --- Package ---
    let transaction = Transaction::<TestnetV0>::from_execution(execution, Some(fee))
        .context("Failed to package transaction")?;

    println!("\n📦 Transaction ID: {}", transaction.id());
    println!("=======================================================\n");

    // ── Phase 4: Broadcast ──────────────────────────────────
    println!("📡 Phase 4: Broadcasting to network...");
    let broadcast_url = format!("{}/transaction/broadcast", node_url);
    // Use Display trait (same as TS SDK's transaction.toString())
    let tx_json = transaction.to_string();

    let client = reqwest::Client::new();
    let resp = client
        .post(&broadcast_url)
        .header("Content-Type", "application/json")
        .body(tx_json)
        .send()
        .await
        .context("Failed to send broadcast request")?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Broadcast rejected ({}): {}", status, body);
    }

    let response_body = resp.text().await.unwrap_or_default();
    println!("🚀 Broadcast accepted: {}", response_body);

    // ── Phase 5: Wait for confirmation ─────────────────────
    println!("\n⏳ Phase 5: Waiting for confirmation... (polling every 5s)");
    let check_url = format!("{}/transaction/{}", node_url, transaction.id());
    let max_retries = 30;

    for retry in 1..=max_retries {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        match reqwest::get(&check_url).await {
            Ok(res) if res.status().is_success() => {
                println!("\n🎉 Confirmed on chain!");
                println!(
                    "🔗 https://testnet.explorer.provable.com/transaction/{}",
                    transaction.id()
                );
                break;
            }
            Err(_) if retry == max_retries => {
                println!(
                    "\n⚠️  Timed out after {} attempts. The transaction may have been dropped due to state root staleness.",
                    max_retries
                );
                println!(
                    "   Try running again — timing is probabilistic.\n   Manual check: https://testnet.explorer.provable.com/transaction/{}",
                    transaction.id()
                );
            }
            _ => {
                print!(".");
                std::io::stdout().flush()?;
            }
        }
    }

    Ok(())
}
