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

## 执行流程

与官方文档一致的 3 步模式：

```
[1/3] buildExecutionTransaction()  首次 ~30-50s，后续同进程 ~5s
[2/3] submitTransaction(tx)        直接传 Transaction 对象（官方写法）
[3/3] waitForConfirmation()        自定义轮询 + getTransaction() 获取详情
```

- `submitTransaction` 接受 `Transaction | string`，官方传对象，我们也传对象
- 必须传 `keySearchParams: { cacheKey: "..." }` 启用密钥内存缓存
- 确认后调 `getTransaction(txId)` 获取链上交易详情

### `verifyExecution` 不适用

`verifyExecution` 只用于 `programManager.run()` 本地离线执行的 proof 验证，**不能**用于链上交易。链上交易的 proof 由 Aleo 验证节点在校验区块时完成，`Status: accepted` 即表示验证通过。

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
