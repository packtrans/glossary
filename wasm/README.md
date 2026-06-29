# WASM Node.js tests

Vitest integration tests for the `packtrans-glossary-wasm` build.

Tests read the same on-disk index and dictionary caches as the query CLI. They do not generate fixtures locally; if the cache is missing, tests fail with instructions to download via the CLI.

## Prerequisites

- Rust toolchain with `wasm32-unknown-unknown` target
- `wasm-pack` (`cargo install wasm-pack`)
- CLI index and dictionary caches for `zh_cn`:

```bash
cargo build --release --bin packtrans-glossary
./target/release/packtrans-glossary index download --lang zh_cn
./target/release/packtrans-glossary dict download lindera-jieba
```

On Unix (including macOS) the default cache root is `~/.local/share/packtrans-glossary/` (`$XDG_DATA_HOME/packtrans-glossary/` when set).

## Commands

```bash
cd wasm
pnpm install
pnpm test
```

`pnpm test` runs `wasm-pack build --target nodejs` into `wasm/pkg/` before executing Vitest.
