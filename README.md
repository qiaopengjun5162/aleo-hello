# aleo-hello

A minimal Aleo program on testnet — with both TypeScript and Rust SDK clients, local dry-run verification, and detailed troubleshooting notes.

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

Aleo stores all private data as ciphertext on-chain. Both clients **verify the plaintext output locally** (30u32) using different approaches.

## Project structure

```
├── src/main.leo              # Leo program source
├── program.json              # Leo project config
├── tests/                    # Leo tests
├── client-ts/                # TypeScript SDK client (working)
│   ├── index.ts              # 4-step execution workflow
│   ├── TROUBLESHOOTING.md    # 5 issues & solutions
│   └── CLAUDE.md
├── client-rust/              # Rust snarkVM client (local ok, broadcast debugging)
│   ├── src/main.rs           # Full ZK proving pipeline
│   ├── TROUBLESHOOTING.md    # 7 issues & solutions
│   └── Cargo.toml
```

## Quick start

### TypeScript (pnpm + Node.js v24)

```bash
cd client-ts
pnpm install
pnpm start
```

### Rust (Cargo)

```bash
cd client-rust
cargo run
```

Both require a `.env` file in the project root with `PRIVATE_KEY=...` and `NODE_URL=https://api.provable.com/v2/testnet`.

## Status

| Client | Local execution | ZK proving | Broadcast | Chain confirm |
|--------|:---:|:---:|:---:|:---:|
| TypeScript | ✅ | ✅ | ✅ | ✅ |
| Rust | ✅ | ✅ | ✅ | ❌ (state root staleness) |

Rust client: proofs pass local verification but are rejected on-chain — likely due to state root expiring during proving (~35s gap). See `client-rust/TROUBLESHOOTING.md`.

## Why this is useful

- Complete Aleo workflow: Leo → deploy → SDK execution
- ZK privacy model explained — why explorer shows ciphertext, code sees plaintext
- Real troubleshooting: Bun WASM incompatibility, 404 noise, key synthesis, VSCode tsconfig, snarkVM version mismatch, consensus version alignment

## Requirements

- [Leo](https://developer.aleo.org/leo) (for building programs)
- Node.js v24+ / pnpm (for TS client)
- Rust 1.95+ (for Rust client)

## License

MIT
