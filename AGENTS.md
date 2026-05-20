# AGENTS.md

## Cursor Cloud specific instructions

### Project overview

Rust CLI workspace for indexing/querying Minecraft mod translation glossaries. Three crates under `crates/`: `packtrans-glossary-core` (library), `packtrans-glossary` (query CLI), `packtrans-glossary-builder` (index builder CLI).

### Toolchain requirement

Requires Rust **1.85+** (Edition 2024, resolver "3"). The update script runs `rustup update stable && rustup default stable` to ensure this.

### Build / Lint / Test

Standard Cargo commands from workspace root:

```sh
cargo build          # Build all crates
cargo test           # Run tests (currently no unit tests exist)
cargo clippy         # Lint
cargo fmt -- --check # Format check (note: existing code has minor formatting diffs)
```

### Running the CLIs

Both binaries need sample translation data arranged as `<scan-dir>/<modid>/<lang>.json`. See README for details.

```sh
# Build an index
cargo run --bin packtrans-glossary-builder -- --index-path indexes index --scan-dir res --lang zh_cn

# Query an index
cargo run --bin packtrans-glossary -- --index-path indexes query --lang zh_cn "Cooking Pot" --limit 10
```

### Notes

- No external services, databases, or Docker required. This is a pure offline CLI.
- The `indexes/` and `res/` directories are git-ignored.
- The CurseForge `create-mod-list` subcommand requires a `CURSEFORGE_API_KEY` env var; the Modrinth subcommand works without keys.
- Lindera dictionary data is downloaded on first use (requires internet for initial setup during index build with CJK languages).
