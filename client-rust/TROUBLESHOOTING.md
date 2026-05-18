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
trace.prove_fee::<AleoTestnetV0, _>(VarunaVersion::V1, &mut rng)
```

## 3. 全局状态根过期（BlockStore 空沙盒）

### 病因
`BlockStore::open(0u16)` 在本地内存中开启高度为 0 的空账本，`prepare` 读取空账本派生的虚假状态根。
ZK proof 中的 `global_state_root` 必须与链上当前区块的默克尔树根一致。

### 修复
使用远程 REST Query：
```rust
let uri = "https://api.provable.com/v2".parse::<http::Uri>()?;
let query = Query::<TestnetV0, BlockMemory<_>>::from(uri);
trace.prepare(&query)?;
```

## 4. 手续费精确对齐

Aleo 的手续费机制是"给多少扣多少，多不退、少拒收"。`base_fee` 必须与电路约束精确匹配。

- `base_fee`: 1,327u64（hello_paxon_2026.aleo/main 的精确约束价格）
- `priority_fee`: 1,000u64（小费，可变）

## 5. API 路由分离（避免路径复读）

snarkVM 内部自动拼接 `/{network}/...`。**必须分开两层 URL**：

| 用途 | URL |
|------|-----|
| snarkVM Query | `https://api.provable.com/v2`（纯净，snarkVM 自行加路径） |
| reqwest HTTP 请求 | `https://api.provable.com/v2/testnet`（显式带路径） |

传 `v2/testnet` 给 Query 会导致路径变成 `v2/testnet/testnet/...`。

## 6. 异步编程：禁止 std::thread::sleep

Tokio 异步运行时中**严禁** `std::thread::sleep`，会卡死工作线程。用 `tokio::time::sleep(...)`。

## 7. 状态根时间窗口过期（当前未解决）

### 现象
Phase 4 broadcast 返回 200 + tx ID，但 Phase 5 轮询永远找不到交易（404）。链上 Explorer 也查不到。

### 分析
从 `prepare`（拉取状态根）到 `broadcast` 总耗时 ~35 秒（exec proving 11s + fee proving 24s）。
测试网每 ~3 秒出一个块，状态根可能已推进 11+ 个区块。
Aleo 验证节点在校验 proof 时对比状态根，过期的状态根会导致交易被静默丢弃。

### 可能方向
- 在 prepare 后立即证明并广播（减小窗口）
- 如果确认超时，用新鲜状态根重新 prepare + prove + broadcast（加重试循环）
- TypeScript SDK 没有这个问题是因为 buildExecutionTransaction 内部 prepare + prove 紧密打包
