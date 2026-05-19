# Aleo Rust Client 踩坑记录

## 1. execute_fee → execute 统一

早期 Aleo 设计中，业务计算与手续费在 VM 层被割裂为两个独立入口（`execute_fee` 与 `execute`）。
最新架构统一为 Authorization Flow，**取消 `execute_fee`，统一用 `.execute(...)`**。

## 2. prove_fee / prove_execution 必须显式指定 VarunaVersion

```rust
use snarkvm::algorithms::snark::varuna::VarunaVersion;

// 错误
trace.prove_fee::<AleoTestnetV0, _>(&mut rng)

// 正确
trace.prove_fee::<AleoTestnetV0, _>(VarunaVersion::V2, &mut rng)
```

## 3. 全局状态根过期（BlockStore 空沙盒）

`BlockStore::open(0u16)` 在本地开启空账本，`prepare` 读取空账本派生的虚假状态根。
必须使用远程 REST Query 或自定义 Query 注入真实状态根。

## 4. 手续费精确对齐

Aleo 手续费是"给多少扣多少，多不退、少拒收"。

- `base_fee`: 1,327u64（hello_paxon_2026.aleo/main 的精确值）
- `priority_fee`: 1,000u64+（小费，可变，拥堵时建议 100,000+）

## 5. API 路由分离

snarkVM 内部自动拼接 `/{network}/...`。**必须分开两层 URL**：

| 用途 | URL |
|------|-----|
| snarkVM Query | `https://api.provable.com/v2`（纯净 base，snarkVM 自行加路径） |
| reqwest HTTP 请求 | `https://api.provable.com/v2/testnet` |

传 `v2/testnet` 给 Query 会导致路径变成 `v2/testnet/testnet/...`。

## 6. 异步编程：禁止 std::thread::sleep

Tokio 中**严禁** `std::thread::sleep`。用 `tokio::time::sleep(...)`。

## 7. ConsensusVersion 匹配区块高度

TestnetV0 共识版本按高度分界（当前高度 ~16.5M → V14）。

## 8. snarkVM 分支必须是 testnet（不是 staging 或 mainnet tag）

**病因**：v4.6.4 tag 是 mainnet，testnet 有独立分支。凭证格式/参数不同。

**修复**：`branch = "testnet"`（snarkVM v4.6.3）。

## 9. Cloudflare WAF 阻断 snarkVM 内部 ureq

**病因**：snarkVM 的 `Query::from(uri)` 用 `ureq::get()` 发请求，User-Agent 是 `ureq/3.x`，被 Cloudflare 当机器人拦截。

**修复**：实现自定义 `QueryTrait`（`FixedStateRootQuery`），用浏览器伪装的 `reqwest::Client` 自行抓取状态根，绕过 ureq。

## 10. VarunaVersion 必须是 V2（不是 V1）

**病因**：测试网已升级到 Varuna V2。用 V1 生成的 proof 链上验证失败。

**关键线索**：`?check_transaction=true` 参数返回精确错误：
- V1 → `Fee verification failed - verify_batch failed`
- 切到 V2（仅 fee）→ 报错从 Fee 变成 Execution
- Execution 也切 V2 → 通过！

## 11. Fee keys 必须用 V0 版本

**病因**：`hello_paxon_2026.aleo` 是 edition 0 程序，对应 V0 credits 体系。标准 `FeePublicProver` 的 key 不匹配。

**修复**：
```rust
use snarkvm::parameters::testnet::{FeePublicV0Prover, FeePublicV0Verifier};
use snarkvm::synthesizer::snark::{ProvingKey, VerifyingKey};
use snarkvm::prelude::FromBytes as _;

let fee_pk = ProvingKey::<TestnetV0>::from_bytes_le(&FeePublicV0Prover::load_bytes()?)?;
let fee_vk = VerifyingKey::<TestnetV0>::from_bytes_le(&FeePublicV0Verifier::load_bytes()?)?;

let credits_id = credits_program.id();
let fee_fn = Identifier::<TestnetV0>::from_str("fee_public")?;
process.insert_proving_key(credits_id, &fee_fn, fee_pk)?;
process.insert_verifying_key(credits_id, &fee_fn, fee_vk)?;
```

`load_bytes()` 内部会自动从 `parameters.provable.com/testnet` 下载并缓存到 `~/.aleo/resources/`。

## 12. Broadcast 加 `?check_transaction=true` 诊断

不加这个参数，验证失败只返回 200 然后交易消失。加上后返回精确的错误信息，直接定位到 `verify_batch failed`。

---

## 最终可用配置

```toml
# Cargo.toml
snarkvm = { git = "https://github.com/ProvableHQ/snarkVM.git", branch = "testnet", features = ["synthesizer", "ledger"] }
```

```rust
// 关键参数
let mut process = Process::<TestnetV0>::load()?;
let credits_program = Program::<TestnetV0>::credits()?;
process.add_program(&credits_program)?;

// 注入 V0 fee keys
let fee_pk = ProvingKey::<TestnetV0>::from_bytes_le(&FeePublicV0Prover::load_bytes()?)?;
let fee_vk = VerifyingKey::<TestnetV0>::from_bytes_le(&FeePublicV0Verifier::load_bytes()?)?;
process.insert_proving_key(credits_id, &fee_fn, fee_pk)?;
process.insert_verifying_key(credits_id, &fee_fn, fee_vk)?;

// 证明
let execution = trace.prove_execution::<AleoTestnetV0, _>(&locator, VarunaVersion::V2, &mut rng)?;
let fee = fee_trace.prove_fee::<AleoTestnetV0, _>(VarunaVersion::V2, &mut rng)?;

// 广播（带诊断参数）
let broadcast_url = format!("{}/transaction/broadcast?check_transaction=true", node_url);
```
