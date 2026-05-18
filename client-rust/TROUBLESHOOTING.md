# Aleo Rust Client 踩坑记录

## 1. execute_fee → execute 统一

早期 Aleo 设计中，业务计算与手续费在 VM 层被割裂为两个独立入口（execute_fee 与 execute）。
最新架构统一为 Authorization Flow，**取消 `execute_fee`，统一用 `.execute(...)` 方法**。

## 2. prove_fee / prove_execution 必须显式指定 VarunaVersion

```rust
use snarkvm::algorithms::snark::varuna::VarunaVersion;

// 错误：缺少 VarunaVersion 参数
trace.prove_fee::<AleoTestnetV0, _>(&mut rng)

// 正确
trace.prove_fee::<AleoTestnetV0, _>(VarunaVersion::V1, &mut rng)
```

## 3. 全局状态根（Global State Root）过期

### 病因
不能使用本地 Mock 沙盒 `BlockStore::open(0u16)`，会拿到空账本的过期状态根。
ZK proof 中的 global_state_root 必须与链上当前区块的默克尔树根严格一致。

### 修复
使用远程 REST Query 实时抓取链上状态根：
```rust
let uri = "https://api.provable.com/v2".parse::<http::Uri>()?;
let query = Query::<TestnetV0, BlockMemory<_>>::from(uri);
trace.prepare(&query)?;
```

## 4. 手续费精确对齐

Aleo 的手续费是"给多少扣多少，多不退、少拒收"。`base_fee` 必须与电路约束精确匹配。

- `base_fee`: 1,327u64（hello_paxon_2026.aleo/main 的精确约束价格）
- `priority_fee`: 可变（排序小费）

## 5. API 路由分离

snarkVM 内部会自动拼接 `/{network}/...` 路径。**必须把两层 URL 分开**：

| 用途 | URL |
|------|-----|
| snarkVM Query (内部自动加路径) | `https://api.provable.com/v2` |
| reqwest 手动 HTTP 请求 | `https://api.provable.com/v2/testnet` |

如果给 Query 传 `v2/testnet`，会导致路径变成 `v2/testnet/testnet/...`。

## 6. 异步编程：禁止 std::thread::sleep

Tokio 异步运行时中**严禁**使用 `std::thread::sleep`，会卡死整个工作线程。
必须用 `tokio::time::sleep(...)`。
