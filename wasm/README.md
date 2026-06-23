# WASM Node.js tests

Vitest integration tests for the `packtrans-glossary-wasm` build.

## Prerequisites

- Rust toolchain with `wasm32-unknown-unknown` target
- `wasm-pack` (`cargo install wasm-pack`)

## Commands

```bash
cd wasm
npm install
npm run generate:fixtures   # refresh fixtures/fr_fr.zip and fixtures/zh_cn.zip
npm test                    # builds wasm (nodejs target) then runs vitest
```

`npm test` runs `wasm-pack build --target nodejs` into `wasm/pkg/` before executing tests.

## Fixtures

Index zip fixtures under `fixtures/` are generated from the Rust crate via:

```bash
cargo run -p packtrans-glossary-wasm --features export-fixtures --bin export-fixtures
```

Regenerate them after changing the shared test index builder in `crates/packtrans-glossary-wasm/src/test_fixtures.rs`.
