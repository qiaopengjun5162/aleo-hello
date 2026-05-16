# 问题排查记录：Aleo SDK 执行程序

## 问题 1：程序一直卡住

### 现象

`bun run start` 执行后，日志只输出到 `Spawning 12 threads`，之后进程一直 hang。

### 诊断

```sh
# Bun — hang
bun -e "import { initThreadPool } from '@provablehq/sdk'; await initThreadPool(); console.log('OK')"

# Node.js — 正常
node --import tsx -e "import { initThreadPool } from '@provablehq/sdk'; await initThreadPool(); console.log('OK')"
```

| 运行时 | `initThreadPool()` | 结果 |
|--------|-------------------|------|
| Bun 1.2.17 | hang | WASM Worker 无法初始化 |
| Node.js v24 | 正常 | 12 线程启动 |

### 根因

Aleo SDK 的 `@provablehq/wasm` 使用 Web Workers 做并行 ZK 计算。Bun 的 Worker 实现与 WASM 线程模型不兼容，Worker 被创建后无法正确回传就绪信号，导致 `initThreadPool()` Promise 永不 resolve。

### 解决

移除 Bun，改用 Node.js v24 + tsx。

---

## 问题 2：VSCode 代码爆红

### 现象

`index.ts` 中 `console`、`process` 报红，`tsconfig.json` 有配置错误。

### 根因

1. `tsconfig.json` 中 `module: "ESNext"` 与 `moduleResolution: "Node16"` 不兼容
2. 未安装 `@types/node`，Node.js 内置 API（`process`、`console`）类型缺失
3. `verbatimModuleSyntax: true` 与 SDK 导出方式不兼容

### 解决

1. 修正 `tsconfig.json`：
   - `"module": "Node16"` （匹配 `moduleResolution`）
   - `"types": ["node"]` （引入 Node.js API 类型）
   - 移除 `verbatimModuleSyntax` 和 `jsx`（不需要）
2. 安装依赖：`pnpm add -D typescript @types/node`
3. 运行 `pnpm exec tsc --noEmit` 验证零错误

---

## 问题 3：waitForTransactionConfirmation 输出大量 404

### 现象

交易提交成功后，控制台刷出一堆 404 错误：

```
Response text from server: {"statusCode":404...
Non-OK response (retrying): 404 ...
```

### 根因

SDK 的 `waitForTransactionConfirmation` 内部用 `console.warn` 输出每次轮询失败的错误响应。交易刚提交时未出块，`/transaction/confirmed/{id}` 返回 404，SDK 2 秒轮询一次，每次都打印 — 这是正常行为，但日志很脏。

### 解决

改为自定义 `waitForConfirmation` 函数：
- 调用 `getConfirmedTransaction()`（返回 `ConfirmedTransactionJSON`，有 `status` 字段）
- 404 时静默重试，只打印简洁的 `Waiting....` 进度点
- 非 404 错误才输出 console.warn

**输出效果：**

```
[3/3] Waiting for confirmation...
  Waiting...
Transaction: at1...
Status:     accepted
```

---

## 问题 4：密钥每次运行都重新合成

### 现象

进程重启后，每次运行都是 ~30-50s 的密钥合成时间。

### 根因

`AleoKeyProvider` 的内存缓存在进程退出后丢失。WASM 内部合成的密钥不会传回 JS 层，无法持久化到磁盘。

### 状态

暂时接受。首次运行 ~30-50s。`synthesizeKeys` 可以单独获取密钥并缓存到磁盘，但需要确保程序源码与链上完全一致（含 constructor），否则密钥不匹配导致 proving 失败。

---

## 参考资料

- [Aleo SDK 官方文档 - Executing Programs](https://developer.aleo.org/sdk/guides/execute_programs)
- [ProvableHQ SDK GitHub](https://github.com/ProvableHQ/sdk)
- [Aleo 101 Bootcamp](https://github.com/openbuildxyz/Aleo-101-Bootcamp)
- [Provable Explorer (Testnet)](https://testnet.explorer.provable.com)
