import {
    Account,
    initThreadPool,
    ProgramManager,
    AleoKeyProvider
} from '@provablehq/sdk';

/**
 * 轮询等待交易被网络确认。
 *
 * SDK 内置的 waitForTransactionConfirmation 每次 404 都会 console.warn，
 * 日志非常脏。这里自己实现一个干净的版本：404 静默重试，只打印进度点。
 */
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
            // 404 是正常的：交易刚提交，还未被打包进区块
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
    // ── 初始化 ──────────────────────────────────────────────
    // initThreadPool 启动 12 个 WASM worker 线程，用于并行 ZK 计算
    console.log("Initializing WASM thread pool...");
    await initThreadPool();
    console.log("Thread pool ready");

    // 从 .env 文件加载私钥（Node.js v21+ 原生支持 --env-file）
    const privateKey = process.env.PRIVATE_KEY;
    if (!privateKey) {
        console.error("PRIVATE_KEY not found.");
        process.exit(1);
    }

    // Account 对象持有私钥、View Key、Compute Key、Address
    const account = new Account({ privateKey });
    console.log(`Account: ${account.address()}`);

    // KeyProvider 管理 proving/verifying keys 的缓存
    const keyProvider = new AleoKeyProvider();
    keyProvider.useCache(true);

    // ProgramManager 是执行 Aleo 程序的核心入口
    const programManager = new ProgramManager("https://api.provable.com/v2", keyProvider);
    programManager.setAccount(account);

    // ── 参数 ────────────────────────────────────────────────
    const programName = "hello_paxon_2026.aleo";
    const functionName = "main";
    const inputs = ["10u32", "20u32"];
    const expectedOutput = "30u32";
    const cacheKey = `${programName}:${functionName}`;

    // ── 获取链上已部署的合约源码 ──────────────────────────────
    // buildExecutionTransaction 内部也会从网络获取合约源码，
    // 但 run() 需要显式传入，所以提前获取一次。
    const program = await programManager.networkClient.getProgram(programName);

    /*
     * ── [Step 0] 本地试运行（不生成 ZK proof）────────────────
     *
     * 为什么要这一步？
     *
     * Aleo 的隐私模型决定了：链上所有私有数据都是密文。
     * 你的合约函数签名是：
     *
     *   fn main(public a: u32, b: u32) -> u32
     *
     * 编译为 Aleo Instructions 后：
     *   input r0 as u32.public;   // a=10 → 全网可见
     *   input r1 as u32.private;  // b=20 → 链上存为 ciphertext
     *   output r2 as u32.private; // 结果 → 链上存为 ciphertext
     *
     * 在浏览器 Explorer 里你只能看到 ciphertext1q... 密文。
     * 只有持有 View Key 的本地代码能解密看到明文 "30u32"。
     *
     * run(proveExecution=false) 在本地用 WASM 执行合约，不生成 proof，
     * 速度很快（~1s），直接返回明文输出。用于在执行链上交易前验证逻辑。
     *
     * 为什么本地的结果可以信任？
     * Aleo 程序是确定性的——同样的代码 + 同样的输入 = 同样的输出。
     * 链上执行 buildExecutionTransaction 时，ZK proof 数学上保证了
     * 计算过程正确。链上 Status: accepted = 全网节点已验证 proof 通过。
     */
    console.log("\n[0] Local dry-run...");
    const localResult = await programManager.run(
        program,
        functionName,
        inputs,
        false, // proveExecution=false → 不需要密钥，快速执行
    );
    const localOutputs = localResult.getOutputs(); // 明文输出，如 ["30u32"]
    console.log(`  Expected: ${expectedOutput}`);
    console.log(`  Got:      ${localOutputs[0]}`);
    if (localOutputs[0] !== expectedOutput) {
        console.error("  Output mismatch!");
        process.exit(1);
    }
    console.log("  OK");

    /*
     * ── [Step 1] 构建链上执行交易 ────────────────────────────
     *
     * buildExecutionTransaction 内部做了这些事：
     *   1. 从 API 获取合约源码
     *   2. 从 parameters.provable.com 获取 credits.aleo fee 密钥
     *   3. 查找/合成本合约的函数密钥（首次 ~20-40s，CPU 密集型）
     *   4. 生成 Authorization（签名授权）
     *   5. 生成 ZK proof（CPU 密集型）
     *   6. 构建完整交易
     *
     * 返回的 tx 是一个 WASM Transaction 对象，可以直接传给 submitTransaction。
     */
    console.log("\n[1] Building execution transaction (ZK proving)...");
    const startTime = Date.now();
    const tx = await programManager.buildExecutionTransaction({
        programName,
        functionName,
        priorityFee: 0.001,    // 优先费（单位：Aleo credits）
        privateFee: false,     // false=用 public balance 付手续费
        inputs,
        keySearchParams: { cacheKey },
    });
    console.log(`  Done in ${((Date.now() - startTime) / 1000).toFixed(1)}s`);

    /*
     * ── [Step 2] 提交交易到 Aleo 测试网 ──────────────────────
     *
     * submitTransaction 接受 Transaction 对象或字符串。
     * 官方文档直接传对象，保持一致。
     */
    console.log("[2] Submitting...");
    const txId = await programManager.networkClient.submitTransaction(tx);

    /*
     * ── [Step 3] 等待确认 + 获取链上详情 ──────────────────────
     *
     * 交易提交后需要 1-3 个区块（约 3-9 秒）才能被确认。
     * waitForConfirmation 轮询 /transaction/confirmed/{txId} 直到状态变为 accepted。
     * getTransaction 获取已确认交易的详情（type、execution 等）。
     */
    console.log("[3] Waiting for confirmation...");
    const status = await waitForConfirmation(programManager, txId);
    const transaction = await programManager.networkClient.getTransaction(txId);

    // ── 结果汇总 ─────────────────────────────────────────────
    console.log(`\nTransaction: ${txId}`);
    console.log(`Status:     ${status}`);
    console.log(`Type:       ${transaction.type}`);
    console.log(`Result:     ${localOutputs[0]}`);
    console.log(`Explorer:   https://testnet.explorer.provable.com/transaction/${txId}`);
}

main();
