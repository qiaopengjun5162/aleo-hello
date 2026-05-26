# 从零到上链：Aleo Rust SDK 客户端实战之旅

## 前言

本文记录了我用纯 Rust（snarkVM）写一个 Aleo 测试网程序执行客户端的全过程。
从"代码编译通过但一直卡住"到最终链上确认，跨越了 12 个技术坑，涉及 ZK 密码学、网络层 WAF 对抗、以及 snarkVM 底层源码阅读。

**结论先行**：Rust 客户端已确认上链，完整流水线可用。

---

## 项目背景

目标：用 Rust 调用 Aleo 测试网上已部署的一个简单合约（两数相加），生成 ZK 证明并广播交易。

```
合约: hello_paxon_2026.aleo
函数: main(public a: u32, b: u32) -> u32
输入: 10u32, 20u32
预期: 30u32
```

## 里程碑总览

```
[❌] Day 1: 程序卡住，initThreadPool() 无响应
[✅] Day 1: 切换到 Node.js 运行时
[✅] Day 2: 本地执行成功，拿到明文 30u32
[✅] Day 3: ZK 证明生成 + 本地验证通过
[❌] Day 4-6: 广播成功但链上永远找不到交易
[🔍] Day 7: 加 ?check_transaction=true 发现真正的错误
[✅] Day 7: VarunaVersion::V2 + V0 Fee Keys → 上链成功
```

---

## 核心踩坑与解决

### 坑 1：Bun 运行时与 WASM 线程池不兼容

**现象**：`bun run` 执行后，卡在 `initThreadPool()`，无任何输出。

**根因**：Aleo SDK 的 WASM 线程池使用 Web Workers，Bun 的实现不兼容。

**解决**：切换到 Node.js v24 + tsx（后来 Rust 客户端也避开了此问题）。

---

### 坑 2：snarkVM 分支选错——staging / mainnet ≠ testnet

**现象**：本地证明验证通过，广播返回 200，但链上找不到交易（一直 404）。

**根因**：snarkVM 的 staging 分支和 v4.6.4 tag 都是 mainnet 版本。测试网有独立的 `testnet` 分支。

**解决**：
```toml
snarkvm = { git = "https://github.com/ProvableHQ/snarkVM.git", branch = "testnet", ... }
```

---

### 坑 3：Cloudflare WAF 阻断 snarkVM 内部 ureq

**现象**：`trace.prepare(&query)` 报 `tls handshake eof` 或 `unexpected end of file`。

**根因**：snarkVM 内部使用 `ureq::get()` 发 HTTP 请求，User-Agent 是 `ureq/3.x`，被 Cloudflare 当机器人直接阻断。外部 reqwest 加浏览器头没用，因为 snarkVM 的 ureq 是内部硬编码的。

**解决**：实现自定义 `QueryTrait`（`FixedStateRootQuery`），用浏览器伪装的 reqwest 自取状态根，绕过 snarkVM 的 ureq。

```rust
struct FixedStateRootQuery<N: Network> {
    state_root: N::StateRoot,
    block_height: u32,
}
// 实现全部 8 个 trait 方法（sync + async）
```

---

### 坑 4：VarunaVersion::V1 → V2

**现象**：加了 `?check_transaction=true` 后，广播返回明确的错误：
```
Fee verification failed - verify_batch failed
```

**根因**：测试网已升级到 Varuna V2 证明系统。用 V1 生成的 proof，本地验证通过（因为我们本地也用 V1），但链上验证节点用 V2——验证失败。

**关键诊断手段**：在广播 URL 后加 `?check_transaction=true`：
```rust
let url = format!("{}/transaction/broadcast?check_transaction=true", node_url);
```

不加这个参数，交易只会静默消失（返回 200 然后被验证节点丢弃）。

**解决**：所有 `prove_execution` 和 `prove_fee` 改用 `VarunaVersion::V2`。

---

### 坑 5：V0 Fee Keys vs 标准 Fee Keys

**现象**：切到 V2 后，Fee proof 通过了但 Execution proof 报错。Execution 也切 V2 后，Fee proof 又报错（来回反复）。

**根因**：`hello_paxon_2026.aleo` 的 constructor 里有 `assert.eq edition 0u16`——这是个 edition 0 程序，对应 **V0 credits 体系**。标准 `FeePublicProver` 加载的 key 是给新版 credits 用的，V0 版需要 `FeePublicV0Prover`。

**解决**：
```rust
use snarkvm::parameters::testnet::{FeePublicV0Prover, FeePublicV0Verifier};

let fee_pk = ProvingKey::<TestnetV0>::from_bytes_le(
    &FeePublicV0Prover::load_bytes()?
)?;
let fee_vk = VerifyingKey::<TestnetV0>::from_bytes_le(
    &FeePublicV0Verifier::load_bytes()?
)?;
process.insert_proving_key(credits_id, &fee_fn, fee_pk)?;
process.insert_verifying_key(credits_id, &fee_fn, fee_vk)?;
```

`load_bytes()` 内部会自动从 `parameters.provable.com/testnet` 下载，缓存到 `~/.aleo/resources/`。

---

### 坑 6：本地旧密钥缓存污染

**现象**：清除了所有缓存后，`snarkVM` 自动从 `parameters.provable.com/testnet` 重新下载了正确的 key 文件。

**根因**：之前用 mainnet 分支运行时，`~/.aleo/resources/` 里缓存了旧版本的参数文件。切到 testnet 分支后，snarkVM 检测到本地已有文件就不再下载，直接用旧的。

**解决**：
```bash
rm -rf ~/.aleo/resources
```
让 snarkVM 触发自动下载。

---

### 坑 7：ConsensusVersion 必须匹配区块高度

**现象**：本地验证传错版本会导致误判。

**根因**：TestnetV0 的共识版本按区块高度分界（当前 ~16.5M → V14）。

**解决**：`verify_execution` 和 `verify_fee` 传 `ConsensusVersion::V14`。

---

### 其他坑点速览

| # | 问题 | 解决 |
|---|------|------|
| 7 | `Process::lock()` vs 新版 API | v4.6.4+ 移除 lock()，直接 `mut process` |
| 8 | `execute_fee` vs `execute` | 新版统一为 `.execute(...)` |
| 9 | locator 格式 | `"{program_id}/{function_name}"` |
| 10 | 手续费精确对齐 | base_fee 必须是 1,327（该程序的精确约束值） |
| 11 | API 路由分离 | Query 用 `v2`（纯净），reqwest 用 `v2/testnet` |
| 12 | 异步 sleep | 必须用 `tokio::time::sleep`，不能用 `std::thread::sleep` |

---

## 最终可用配置

```rust
// Cargo.toml
snarkvm = { git = "https://github.com/ProvableHQ/snarkVM.git", branch = "testnet", features = ["synthesizer", "ledger"] }

// 关键参数
Process::<TestnetV0>::load()           // 非 load_v0()
VarunaVersion::V2                      // 非 V1
FeePublicV0Prover / FeePublicV0Verifier // V0 版 fee keys
ConsensusVersion::V14                  // 当前高度对应
?check_transaction=true                // 广播诊断
```

## 代码架构

```
client-rust/src/
├── main.rs      CLI + 流程编排 (80行)
├── core.rs      Engine: 初始化/V0注入/证明/验证 (130行)
├── network.rs   HTTP客户端/WAF伪装/广播/确认 (90行)
└── query.rs     FixedStateRootQuery (绕过ureq) (55行)
```

## 经验总结

1. **`?check_transaction=true` 是调试神器**：不加它，交易静默消失没有任何线索
2. **snarkVM 分支要对齐**：testnet 网络必须用 testnet 分支，mainnet tag 不兼容
3. **VarunaVersion 不是装饰**：V1/V2 差别巨大，必须匹配当前网络版本
4. **Fee key 版本由程序 edition 决定**：edition 0 → V0 keys
5. **WAF 在密码学层之下**：Cloudflare 不管你的 ZK proof 多漂亮，User-Agent 不对就直接掐
6. **本地验证通过 ≠ 链上会过**：版本错配时本地验证用错参数也能过，必须加 `?check_transaction=true`

---

## 参考资源

- [ProvableHQ/snarkVM](https://github.com/ProvableHQ/snarkVM)
- [ProvableHQ/python-sdk](https://github.com/ProvableHQ/python-sdk)
- [Provable API v2 Docs](https://docs.explorer.provable.com/docs/api/v2/intro)
- [Aleo SDK Docs](https://developer.aleo.org/sdk)
- 本项目仓库：https://github.com/qiaopengjun5162/aleo-hello
