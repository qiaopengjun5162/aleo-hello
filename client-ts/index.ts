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

    // Step 1: Build the execution transaction (includes ZK proof generation)
    console.log("\n[1/3] Building execution transaction...");
    const startTime = Date.now();
    const tx = await programManager.buildExecutionTransaction({
        programName,
        functionName,
        priorityFee: 0.001,
        privateFee: false,
        inputs: ["10u32", "20u32"],
        keySearchParams: { cacheKey: `${programName}:${functionName}` },
    });
    console.log(`  Done in ${((Date.now() - startTime) / 1000).toFixed(1)}s`);

    // Step 2: Submit the transaction to the network
    // submitTransaction accepts both Transaction object and string — passing
    // the object directly is consistent with the official docs.
    console.log("[2/3] Submitting...");
    const txId = await programManager.networkClient.submitTransaction(tx);

    // Step 3: Wait for confirmation (1-3 blocks, ~3-9 seconds)
    console.log("[3/3] Waiting for confirmation...");
    const status = await waitForConfirmation(programManager, txId);

    // Get the confirmed transaction details from the network
    const transaction = await programManager.networkClient.getTransaction(txId);

    console.log(`\nTransaction: ${txId}`);
    console.log(`Status:     ${status}`);
    console.log(`Type:       ${transaction.type}`);
    console.log(`Explorer:   https://testnet.explorer.provable.com/transaction/${txId}`);
}

main();
