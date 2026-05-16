# aleo-hello

A minimal Aleo program on testnet — with a production-grade TypeScript SDK workflow, local dry-run verification, and detailed troubleshooting notes.

## What it does

Deploys and executes a Leo program that adds two numbers:

```leo
// src/main.leo
program hello_paxon_2026.aleo {
    fn main(public a: u32, b: u32) -> u32 {
        let c: u32 = a + b;
        return c;
    }
}
```

The TypeScript client (`client-ts/`) executes this on-chain and **verifies the plaintext output locally** — since Aleo stores all private data as ciphertext on-chain.

## Project structure

```
├── src/main.leo         # Leo program source
├── program.json         # Leo project config
├── tests/               # Leo tests
├── client-ts/           # TypeScript SDK client
│   ├── index.ts         # 4-step execution workflow
│   ├── TROUBLESHOOTING.md  # 5 real-world issues & solutions
│   └── CLAUDE.md        # Project conventions
```

## Quick start

### 1. Deploy the program (Leo CLI)

```bash
leo build
leo deploy --network testnet
```

### 2. Execute via TypeScript SDK

```bash
cd client-ts
pnpm install
pnpm start
```

Requires a `.env` file in the project root with `PRIVATE_KEY=...`.

Output:

```
[0] Local dry-run...            ~1s
  Expected: 30u32
  Got:      30u32
  OK

[1] Building execution (ZK proving)...  ~30s first run
[2] Submitting...
[3] Waiting for confirmation...
  Waiting...

Transaction: at1...
Status:     accepted
Type:       execute
Result:     30u32
```

## Why this is useful

This project demonstrates the complete Aleo developer workflow:

- **Leo program** → compile → deploy → execute on-chain
- **ZK privacy model** — why the explorer shows ciphertext but your code sees plaintext
- **Real troubleshooting** — Bun WASM incompatibility, 404 noise during confirmation, key synthesis performance, VSCode tsconfig issues, and how to get plaintext results from a zero-knowledge chain

See `client-ts/TROUBLESHOOTING.md` for the full debug log.

## Requirements

- [Leo](https://developer.aleo.org/leo) (for building Leo programs)
- Node.js v24+
- pnpm

## License

MIT
