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

/// Fetches the program source code from the Aleo network and parses it into a `Program` AST.
async fn fetch_program(node_url: &str, program_id: &str) -> Result<Program<TestnetV0>> {
    let url = format!("{}/program/{}", node_url, program_id);
    println!("[Network] Sending GET request to: {}", url);

    let response_text = reqwest::get(&url)
        .await
        .context("Failed to execute HTTP request")?
        .text()
        .await
        .context("Failed to read response payload")?;

    let clean_text = response_text.trim_matches('"').replace("\\n", "\n");

    // Parse the raw text into a snarkVM Program structure
    let program = Program::<TestnetV0>::from_str(&clean_text)
        .context("Failed to parse the program via snarkVM")?;

    Ok(program)
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load environment variables
    dotenv().ok();

    let node_url =
        env::var("NODE_URL").unwrap_or_else(|_| "https://api.provable.com/v2/testnet".to_string());

    let pk_string =
        env::var("PRIVATE_KEY").context("PRIVATE_KEY not found in environment variables")?;

    let program_name = "hello_paxon_2026.aleo";
    println!("\n🚀 Starting Aleo Rust Client (TestnetV0)...\n");

    // 2. Fetch program from the network
    let program = fetch_program(&node_url, program_name).await?;
    println!("[Success] Program parsed successfully: {}", program.id());

    // 3. Initialize the Process (Local VM)
    println!("\n🧠 [VM] Initializing Process container...");
    let process = Process::<TestnetV0>::load_v0().context("Failed to initialize Process")?;

    println!("[VM] Adding program to the Process...");
    process
        .lock()
        .add_program(&program)
        .context("Failed to add program to Process")?;

    // 4. Load the Account Private Key
    let private_key =
        PrivateKey::<TestnetV0>::from_str(&pk_string).context("Invalid private key format")?;

    println!("🔑 Account ready. Preparing for local execution (Dry-Run)...");

    // 5. Setup Cryptographic RNG and Inputs
    let mut rng = TestRng::default();
    let inputs = vec!["10u32", "20u32"];
    println!("▶️  Inputs: {:?}", inputs);

    let start_time = std::time::Instant::now();

    // 6. Phase 1: Execute Authorize (Dry-Run / Logic Constraints)
    println!("\n⏳ Phase 1: Authorizing transaction (Dry-Run)...");
    let authorization = process
        .authorize::<AleoTestnetV0, _>(
            &private_key,
            program.id(),
            "main",
            inputs.into_iter(),
            &mut rng,
        )
        .context("Local authorization/execution failed. Check input types.")?;

    println!("✅ Authorization generated successfully!");

    // Note: Uncomment the following line to inspect the raw encrypted transitions
    // println!("\n📜 Execution Transitions:\n{:#?}", authorization.transitions());

    // 7. Phase 2: Full Local Execution (Generate Trace and Response)
    println!("\n⏳ Phase 2: Full local execution...");
    let (response, mut trace) = process
        .execute::<AleoTestnetV0, _>(authorization, &mut rng)
        .context("Full local execution failed.")?;

    println!(
        "✅ Local execution completed successfully! Elapsed: {:?}",
        start_time.elapsed()
    );

    // 8. Extract and Display Plaintext Response
    println!("\n=======================================================");
    println!("🌟 [Transaction Execution Response]");
    println!("=======================================================");

    for (i, output) in response.outputs().iter().enumerate() {
        println!("💡 Response Output [{}]: {}", i, output);
    }

    println!("=======================================================\n");

    // =========================================================================
    // ⬇️ NEW CODE: Phase 3 - Generate ZK Proof & Package Transaction
    // =========================================================================

    // Note: Remove the underscore from `_trace` in Phase 2
    // Change: let (response, _trace) = ...  -> let (response, mut trace) = ...
    // You MUST update Phase 2 to make `trace` mutable so we can prepare and prove it.

    println!("\n=======================================================");
    println!("🔥 [Phase 3: Proving & Packaging - WARNING: CPU INTENSIVE] 🔥");
    println!("=======================================================");

    let proving_start_time = std::time::Instant::now();
    println!("⏳ Synthesizing Zero-Knowledge Proofs... (This may take a while, please wait...)");

    // 1. Prepare the trace for proving.

    let v2_node_url = "https://api.provable.com/v2";
    println!("\n🔍 Fetching latest state root from network (V2 API)...");

    let uri = v2_node_url
        .parse::<http::Uri>()
        .context("Failed to parse node URL into http::Uri")?;

    let query = Query::<TestnetV0, BlockMemory<_>>::from(uri);

    trace
        .prepare(&query)
        .context("Failed to prepare trace for proving")?;

    // =========================================================================
    // 2. 核心逻辑证明 (Execution Proof)
    // =========================================================================
    let execution = trace
        .prove_execution::<AleoTestnetV0, _>("hello_paxon_2026", VarunaVersion::V1, &mut rng)
        .context("Failed to generate ZK execution proof")?;

    println!(
        "✅ Execution Proof generated successfully! Elapsed: {:?}",
        proving_start_time.elapsed()
    );

    // =========================================================================
    // ⬇️ 手续费证明 (Fee Proof)
    // =========================================================================
    println!("\n⏳ Synthesizing Fee Proof... (Paying public fee)");
    let fee_start_time = std::time::Instant::now();

    // 设定手续费参数 (单位: microcredits)。总费用 = base_fee + priority_fee
    let base_fee = 1_327u64;
    let priority_fee = 100_000u64; // priorityFee: 0.001 (即 1,000 microcredits)

    let execution_id = execution
        .to_execution_id()
        .context("Failed to get execution ID")?;

    // 2.1 授权公开手续费交易
    let fee_authorization = process
        .authorize_fee_public::<AleoTestnetV0, _>(
            &private_key,
            base_fee,
            priority_fee,
            execution_id,
            &mut rng,
        )
        .context("Failed to authorize public fee")?;

    // 2.2 本地执行手续费逻辑 (生成手续费 Trace)
    let (_fee_response, mut fee_trace) = process
        .execute::<AleoTestnetV0, _>(fee_authorization, &mut rng)
        .context("Failed to execute fee")?;

    // 2.3 为手续费 Trace 准备账本查询数据
    fee_trace
        .prepare(&query)
        .context("Failed to prepare fee trace")?;

    // 2.4 榨干算力：炼制专属的手续费 ZK Proof！
    let fee = fee_trace
        .prove_fee::<AleoTestnetV0, _>(VarunaVersion::V1, &mut rng)
        .context("Failed to generate fee proof")?;

    println!(
        "✅ Fee Proof generated successfully! Elapsed: {:?}",
        fee_start_time.elapsed()
    );

    // =========================================================================
    // 3. Package into a Transaction object. 终极组装：主交易 + 手续费
    // =========================================================================
    let transaction = Transaction::<TestnetV0>::from_execution(execution, Some(fee))
        .context("Failed to package transaction")?;

    println!("\n📦 [Final Packaged Transaction ID]: {}", transaction.id());
    // println!("Payload:\n{:#?}", transaction);
    // =========================================================================
    // ⬇️ 美化打印：将对象序列化为带格式的纯净 JSON
    // =========================================================================
    let pretty_json = serde_json::to_string_pretty(&transaction)
        .context("Failed to serialize transaction to pretty JSON")?;

    println!("Payload:\n{}", pretty_json);

    println!("=======================================================\n");

    // 9. Phase 4: Broadcast Transaction to Testnet
    println!("\n📡 Phase 4: Broadcasting transaction to Provable API v2...");
    let broadcast_url = format!("{}/transaction/broadcast", node_url);

    // 将 transaction 序列化为 JSON 字符串
    let tx_json = serde_json::to_string(&transaction)?;

    let client = reqwest::Client::new();
    let response = client
        .post(&broadcast_url)
        .header("Content-Type", "application/json")
        .body(tx_json)
        .send()
        .await
        .context("Failed to send broadcast request")?;

    if response.status().is_success() {
        let response_body = response.text().await.unwrap_or_default();
        // 节点通常会返回包含 TxID 的字符串，打印出来让心里有底
        println!(
            "🚀 Broadcast Success! Node accepted and returned: {}",
            response_body
        );

        // 10. Phase 5: Poll for Confirmation
        println!("⏳ Phase 5: Waiting for chain confirmation... (Polling every 5s)");
        let check_url = format!("{}/transaction/{}", node_url, transaction.id());

        // 💡 增加超时机制，防止死循环 (最多查 30 次，即 150 秒)
        let mut retries = 0;
        let max_retries = 30;

        loop {
            if retries >= max_retries {
                println!(
                    "\n⚠️ Polling timed out after {} attempts. The transaction might still be pending in the mempool, or it was dropped.",
                    max_retries
                );
                println!(
                    "🔍 Check manually later: https://testnet.explorer.provable.com/transaction/{}",
                    transaction.id()
                );
                break;
            }

            // 💡 使用 tokio 的非阻塞睡眠，绝不能用 std::thread::sleep！
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            retries += 1;

            if let Ok(res) = reqwest::get(&check_url).await {
                if res.status().is_success() {
                    println!("🎉 Permanent Ledger Confirmation Found!");
                    println!(
                        "🔗 View your transaction here: https://testnet.explorer.provable.com/transaction/{}",
                        transaction.id()
                    );
                    break;
                }
            }
            print!(".");
            std::io::stdout().flush()?;
        }
    } else {
        println!("❌ Broadcast rejected by node: {:?}", response.text().await);
    }

    Ok(())
}
