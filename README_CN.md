# aleo-hello

Aleo 测试网最小示例 — TypeScript 和 Rust 双客户端，本地试运行验证 + 完整踩坑排查记录。

## 功能

部署并执行一个两数相加的 Leo 合约：

```leo
// src/main.leo
program hello_paxon_2026.aleo {
    fn main(public a: u32, b: u32) -> u32 {
        let c: u32 = a + b;
        return c;
    }
}
```

Aleo 链上所有私有数据都是密文。两个客户端均在**本地解密并验证明文输出**（30u32），但实现方式不同。

## 项目结构

```
├── src/main.leo              # Leo 合约源码
├── program.json              # Leo 项目配置
├── tests/                    # Leo 测试
├── client-ts/                # TypeScript SDK 客户端（可用）
│   ├── index.ts              # 4 步执行流程
│   ├── TROUBLESHOOTING.md    # 5 个踩坑记录
│   └── CLAUDE.md
├── client-rust/              # Rust snarkVM 客户端（本地可用，广播调试中）
│   ├── src/main.rs           # 完整 ZK 证明流水线
│   ├── TROUBLESHOOTING.md    # 7 个踩坑记录
```

## 快速开始

### TypeScript 客户端

```bash
cd client-ts
pnpm install
pnpm start
```

### Rust 客户端

```bash
cd client-rust
cargo run
```

均需在项目根目录的 `.env` 中配置 `PRIVATE_KEY=...`。

## 当前状态

| 客户端 | 本地执行 | ZK 证明 | 广播 | 链上确认 |
|--------|:---:|:---:|:---:|:---:|
| TypeScript | ✅ | ✅ | ✅ | ✅ |
| Rust | ✅ | ✅ | ✅ | ❌（状态根过期） |

Rust 客户端：proof 本地验证通过，但链上确认失败 — 疑似 `prepare` 到 `broadcast` 间隔过长（~35s），状态根过期。详见 `client-rust/TROUBLESHOOTING.md`。

## 为什么有价值

- 完整 Aleo 工作流：Leo → 部署 → SDK 调用
- ZK 隐私模型：浏览器看密文、本地看明文的原理
- 实战踩坑：Bun WASM 不兼容、404 轮询噪音、密钥合成、VSCode tsconfig、snarkVM 版本对齐、共识版本匹配

## 环境要求

- [Leo](https://developer.aleo.org/leo)
- Node.js v24+ / pnpm
- Rust 1.95+

## 许可证

MIT
