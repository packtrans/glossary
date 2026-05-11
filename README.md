# Packtrans Glossary

Rust-based CLI tools for indexing and querying Minecraft mod glossary translations. Uses [Tantivy](https://github.com/quickwit-oss/tantivy) for fast full-text search and relevance scoring.

## Overview

This workspace provides two command-line utilities:

- **`packtrans_glossary`** – End-user tool for querying translations.
- **`packtrans_glossary_builder`** – Index builder for creating searchable translation databases.

## Features

- Index language JSON files from multiple Minecraft mods.
- Query English/source text to retrieve target-language translations.
- Results sorted by Tantivy relevance score.
- Supports indexing multiple mods into a single target-language database.

## Project Structure

```text
crates/
├── packtrans-glossary-core       # Shared core library
├── packtrans-glossary            # End-user query CLI
└── packtrans-glossary-builder    # Index builder CLI
```

## Resource Layout

Language files must be arranged as:

```text
<scan-dir>/<modid>/<lang>.json
```

Example:

```text
res/farmersdelight/en_us.json
res/farmersdelight/zh_cn.json
res/farmersdelight/ja_jp.json
```

The `modid` is derived from the direct child directory name under `<scan-dir>`.

## Usage

### Building an Index

```bash
packtrans_glossary_builder index \
  --scan-dir res \
  --source en_us \
  --target zh_cn \
  --index-db indexes/zh_cn
```

**Requirements:**
- `--scan-dir`, `--source`, `--target`, and `--index-db` are all required.
- Scans all direct child directories under `--scan-dir`; each is treated as one mod.
- Skips mods with missing source or target language files (prints a warning).
- Fails if `--index-db` already exists (unless a future `--force` option is used).

### Querying Translations

```bash
packtrans_glossary query "Cooking Pot" \
  --index-db indexes/zh_cn \
  --limit 20
```

**Requirements:**
- Query text and `--index-db` are required.
- `--limit` is optional; defaults to `20`.
- No need to specify source/target language or mod ID—these are stored in the index.

## Query Output

Results are displayed in a table format sorted by confidence (Tantivy score):

```text
confidence  mod_id          key                                  target_lang  target
12.94       farmersdelight  block.farmersdelight.cooking_pot     zh_cn        厨锅
```

## Tantivy Schema

Each indexed document contains the following fields:

| Field        | Type           | Description               |
|--------------|----------------|---------------------------|
| `mod_id`     | Stored string  | Mod identifier            |
| `key`        | Stored string  | Translation key           |
| `source_lang`| Stored string  | Source language code      |
| `source_text`| Indexed + stored | Source text (searched)   |
| `target_lang`| Stored string  | Target language code      |
| `target_text`| Stored string  | Translated text           |

## Development

### Format and Check

```bash
cargo fmt
cargo check
```

### Example Workflow

```bash
# Build index
cargo run --bin packtrans_glossary_builder -- index \
  --scan-dir res \
  --source en_us \
  --target zh_cn \
  --index-db indexes/zh_cn

# Query index
cargo run --bin packtrans_glossary -- query "Cooking Pot" \
  --index-db indexes/zh_cn \
  --limit 10

cargo run --bin packtrans_glossary -- query "Stove" \
  --index-db indexes/zh_cn \
  --limit 10
```

## Future Enhancements

- `--force` flag to rebuild existing indexes
- JSON output mode
- Fuzzy match
- Batch indexing of multiple target languages

## License

[MIT](LICENSE)
