# aleo-hello

Aleo 测试网上的最小程序示例 — 配备完整的 TypeScript SDK 工作流、本地试运行验证、以及详细的踩坑排查记录。

## 项目做了什么

部署并执行一个 Leo 加法合约：

```leo
// src/main.leo
program hello_paxon_2026.aleo {
    fn main(public a: u32, b: u32) -> u32 {
        let c: u32 = a + b;
        return c;
    }
}
```

TypeScript 客户端 (`client-ts/`) 在链上执行该合约，并在**本地解密明文输出** — 因为 Aleo 链上所有私有数据都以密文形式存储。

## 项目结构

```
├── src/main.leo             # Leo 合约源码
├── program.json             # Leo 项目配置
├── tests/                   # Leo 单元测试
├── client-ts/               # TypeScript SDK 客户端
│   ├── index.ts             # 4 步执行流程（有详细注释）
│   ├── TROUBLESHOOTING.md   # 5 个实战坑点及解决方案
│   └── CLAUDE.md            # 项目约定
```

## 快速开始

### 1. 部署合约（Leo CLI）

```bash
leo build
leo deploy --network testnet
```

### 2. 通过 TypeScript SDK 执行

```bash
cd client-ts
pnpm install
pnpm start
```

需要在项目根目录的 `.env` 文件中配置 `PRIVATE_KEY=...`。

### 输出示例

```
[0] Local dry-run...               ~1s
  Expected: 30u32
  Got:      30u32
  OK

[1] Building execution (ZK proving)...  ~30s 首次运行
[2] Submitting...
[3] Waiting for confirmation...
  Waiting...

Transaction: at1...
Status:     accepted
Type:       execute
Result:     30u32
```

## 执行流程说明

Aleo 的 ZK 隐私模型使得链上所有私有输入和输出都以密文（ciphertext）形式存储。在浏览器 Explorer 中只能看到 `ciphertext1q...`，看不到实际数字。

这个项目的关键设计是 **Step 0：本地试运行**：

```
run(proveExecution=false)  → 本地 WASM 执行，不生成 proof
                           → 用 View Key 解密输出，得到明文 "30u32"
                           → 快速 (~1s)，用于验证逻辑正确性

buildExecutionTransaction  → 同样的计算 + 生成 ZK proof
                           → 提交到链上
                           → Status: accepted = 全网节点验证 proof 通过
                           → 数学保证：proof 通过 → 计算结果一定是 30
```

## 为什么这个项目有价值

虽然合约本身只有两数相加，但项目展示了完整的 Aleo 开发工作流：

- **Leo 合约** → 编译 → 部署 → 链上执行
- **ZK 隐私模型** — 为什么浏览器看到密文，本地代码能看到明文
- **实战踩坑** — Bun WASM 不兼容、确认轮询 404 噪音、密钥合成性能、VSCode 类型配置、如何从零知识链获取明文结果

详见 `client-ts/TROUBLESHOOTING.md` 完整排查记录。

## 环境要求

- [Leo](https://developer.aleo.org/leo)（编译 Leo 合约）
- Node.js v24+
- pnpm

## 许可

MIT
