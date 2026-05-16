---
description: Aleo 测试网程序执行客户端。pnpm + Node.js + tsx。
globs: "*.ts, package.json, tsconfig.json"
alwaysApply: false
---

## 运行环境

- 使用 **pnpm** 管理依赖，**Node.js v24** + **tsx** 运行 TypeScript
- **不使用 Bun**：Aleo SDK (`@provablehq/sdk`) 的 WASM 线程池 (`initThreadPool`) 与 Bun Web Worker 不兼容，会永久 hang

## 启动命令

```sh
cd client-ts && pnpm start
```

等价于：

```sh
node --env-file=../.env --import tsx index.ts
```

`.env` 位于项目根目录 (`../.env`)，Node.js v21+ 原生支持 `--env-file`，无需 dotenv。

## 依赖

| 包 | 用途 |
|---|------|
| `@provablehq/sdk` | Aleo SDK（testnet 版） |
| `tsx` | Node.js TypeScript 运行时 |
| `typescript` | 类型检查 (`pnpm exec tsc --noEmit`) |
| `@types/node` | Node.js API 类型定义 |

## API Endpoint

- 使用 `https://api.provable.com/v2`（官方文档一致）
- Explorer 仅用于浏览链上数据：`https://testnet.explorer.provable.com`

## 执行流程 (4 步)

```
[0] programManager.run(proveExecution=false)  本地试运行，验证输出 (~1s)
[1] buildExecutionTransaction()               首次 ~30-50s，同进程 ~5s
[2] submitTransaction(tx)                     ~1s
[3] waitForConfirmation() + getTransaction()  轮询确认 + 获取详情
```

### 为什么要本地试运行？

Aleo 链上所有私有数据都是密文（浏览器里看到的都是 `ciphertext1q...`）。想看到明文结果（如 `30u32`），只能在本地用 `programManager.run()` 执行并解密。

- `run(program, fn, inputs, false)` — 不生成 proof，快速得到明文输出
- 将本地输出与预期值比对，确保合约逻辑正确
- 链上 `Status: accepted` 即表示 ZK 证明验证通过，结果与本地一致

### `verifyExecution` 不适用

`verifyExecution` 只用于 `run(proveExecution=true)` 本地离线执行的 proof 验证，**不能**用于链上交易。链上 proof 由 Aleo 验证节点校验。

## VSCode 设置

- TypeScript 版本使用项目 workspace 版本（`pnpm exec tsc --noEmit` 验证）
- 如果 VSCode 没有自动加载 workspace TypeScript，`.vscode/settings.json` 中设置：
  ```json
  { "typescript.tsdk": "node_modules/typescript/lib" }
  ```

## 文件结构

```
client-ts/
├── index.ts           # 主入口
├── package.json       # pnpm 配置
├── pnpm-lock.yaml     # 依赖锁定
├── tsconfig.json      # TypeScript 配置 (Node16)
├── TROUBLESHOOTING.md # 问题排查记录
├── CLAUDE.md          # 本文件
└── node_modules/
```
