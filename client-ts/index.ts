import {
    Account,
    initThreadPool,
    ProgramManager,
    AleoKeyProvider
} from '@provablehq/sdk';

async function waitForConfirmation(
    programManager: ProgramManager,
    txId: string,
    timeoutMs = 60000,
): Promise<string> {
    const start = Date.now();
    let attempts = 0;
    while (Date.now() - start < timeoutMs) {
        attempts++;
        try {
            const tx = await programManager.networkClient.getConfirmedTransaction(txId);
            if (tx.status === "accepted" || tx.status === "confirmed") return tx.status;
            if (tx.status === "rejected") throw new Error(`Transaction rejected`);
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            if (!msg.includes("404") && !msg.includes("not found")) {
                console.warn(`  Polling error: ${msg}`);
            }
        }
        if (attempts === 1) process.stdout.write("  Waiting.");
        process.stdout.write(".");
        await new Promise((r) => setTimeout(r, 2000));
    }
    console.log();
    throw new Error(`Transaction not confirmed within ${timeoutMs}ms`);
}

async function main() {
    console.log("Initializing WASM thread pool...");
    await initThreadPool();
    console.log("Thread pool ready");

    const privateKey = process.env.PRIVATE_KEY;
    if (!privateKey) {
        console.error("PRIVATE_KEY not found.");
        process.exit(1);
    }

    const account = new Account({ privateKey });
    console.log(`Account: ${account.address()}`);

    const keyProvider = new AleoKeyProvider();
    keyProvider.useCache(true);

    const programManager = new ProgramManager("https://api.provable.com/v2", keyProvider);
    programManager.setAccount(account);

    const programName = "hello_paxon_2026.aleo";
    const functionName = "main";
    const inputs = ["10u32", "20u32"];
    const cacheKey = `${programName}:${functionName}`;
    const expectedOutput = "30u32";

    // Fetch program source from the network
    const program = await programManager.networkClient.getProgram(programName);

    // Step 0: Local dry-run to verify expected output (fast, no proof)
    console.log("\n[0] Local dry-run (verifying expected output)...");
    const localResult = await programManager.run(
        program,
        functionName,
        inputs,
        false, // don't prove — fast
    );
    const localOutputs = localResult.getOutputs();
    console.log(`  Expected: ${expectedOutput}`);
    console.log(`  Got:      ${localOutputs[0]}`);
    if (localOutputs[0] !== expectedOutput) {
        console.error(`  MISMATCH!`);
        process.exit(1);
    }
    console.log("  OK");

    // Step 1: Build execution transaction (ZK proof generation)
    console.log("\n[1] Building execution transaction...");
    const startTime = Date.now();
    const tx = await programManager.buildExecutionTransaction({
        programName,
        functionName,
        priorityFee: 0.001,
        privateFee: false,
        inputs,
        keySearchParams: { cacheKey },
    });
    console.log(`  Done in ${((Date.now() - startTime) / 1000).toFixed(1)}s`);

    // Step 2: Submit
    console.log("[2] Submitting...");
    const txId = await programManager.networkClient.submitTransaction(tx);

    // Step 3: Wait for confirmation
    console.log("[3] Waiting for confirmation...");
    const status = await waitForConfirmation(programManager, txId);

    // Get confirmed transaction details
    const transaction = await programManager.networkClient.getTransaction(txId);

    console.log(`\nTransaction: ${txId}`);
    console.log(`Status:     ${status}`);
    console.log(`Type:       ${transaction.type}`);
    console.log(`Result:     ${localOutputs[0]} (verified via local execution)`);
    console.log(`Explorer:   https://testnet.explorer.provable.com/transaction/${txId}`);
}

main();
