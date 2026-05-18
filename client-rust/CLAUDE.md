---
description: Aleo Rust 客户端 — snarkVM v4.6.4，本地 ZK 证明生成 + 链上广播。广播成功但链上确认调试中。
globs: "*.rs, Cargo.toml"
alwaysApply: false
---

## 运行环境

- **Rust 1.95+**
- **snarkVM v4.6.4**（固定 tag，非 staging 分支）
- 使用 `.env` 加载私钥和 API 配置

## 启动

```sh
cd client-rust && cargo run
```

## 执行流程

```
Phase 1: Authorize        → 本地授权
Phase 2: Local execution  → 明文输出 (30u32)
Phase 3: ZK proving       → exec proof + fee proof + local verification
Phase 4: Broadcast        → POST to /v2/testnet/transaction/broadcast
Phase 5: Confirm          → 轮询 /v2/testnet/transaction/{id}
```

## 关键配置

- `ConsensusVersion::V14` — 当前测试网高度 ~16.5M
- `VarunaVersion::V1`
- `base_fee = 1_327u64` — 合约精确约束价格
- `priority_fee = 1_000u64`
- locator 格式: `"{program_id}/{function_name}"` (如 `"hello_paxon_2026.aleo/main"`)

## API 路由

| 用途 | URL |
|------|-----|
| snarkVM Query | `https://api.provable.com/v2`（纯净 base） |
| reqwest HTTP | `https://api.provable.com/v2/testnet` |

snarkVM 会自动拼接 `/testnet/...`，传双重路径会出错。

## 已知问题

- Broadcast 返回 200 + txID，但交易无法在链上找到
- Proofs 本地验证通过，排除 proof 生成错误
- 最可能原因：状态根过期（prepare→broadcast 间隔 ~35s）
