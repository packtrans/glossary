# Packtrans Glossary

Rust-based CLI tools for indexing and querying Minecraft mod glossary translations. Uses [Tantivy](https://github.com/quickwit-oss/tantivy) for fast full-text search and relevance scoring.

## Overview

This workspace provides two command-line utilities:

- **`packtrans-glossary`** – End-user tool for querying translations and managing dictionaries.
- **`packtrans-glossary-builder`** – Index builder for creating searchable translation databases, with mod list generation and language file downloading.

## Features

- Index language JSON files from multiple Minecraft mods.
- Query English/source text to retrieve target-language translations.
- Inverse query mode: search by target-language text to find source text.
- Results sorted by Tantivy relevance score.
- Supports indexing multiple mods into a single target-language database.
- Download mod language files from Modrinth, CurseForge, or Minecraft directly.
- Generate mod lists from Modrinth or CurseForge APIs.
- CJK tokenizer support via Lindera (Japanese ipadic, Korean ko-dic, Chinese jieba).
- Manage downloadable Lindera dictionaries (`dict download`, `dict ls`, `dict delete`, `dict clean`).

## Project Structure

```text
crates/
├── packtrans-glossary-core       # Shared core library (schema, indexing, querying, tokenizers, utilities)
├── packtrans-glossary            # End-user query CLI
└── packtrans-glossary-builder    # Index builder CLI (index, create-mod-list, download)
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
packtrans-glossary-builder --index-path indexes index \
  --scan-dir res \
  --lang zh_cn
```

**Options:**
- `--scan-dir` and `--lang` are required. `--index-path` is optional and defaults to the system data directory when omitted.
- `--dict-path` optionally overrides the dictionary storage location.
- Source language is always `en_us`.
- Scans all direct child directories under `--scan-dir`; each is treated as one mod.
- Skips mods with missing source or target language files (no error; summary reports total mods and mods with both language files).
- Malformed language JSON is skipped with a warning; indexing still completes successfully.
- Fails if the index already exists.

### Downloading Mod Language Files

```bash
# Download from Modrinth using a mod list
packtrans-glossary-builder download --platform modrinth \
  --output res --list-file mods.json

# Download from CurseForge using a mod list
packtrans-glossary-builder download --platform curseforge \
  --output res --list-file mods.json

# Download Minecraft vanilla language files (no list file needed)
packtrans-glossary-builder download --platform minecraft --output res
```

**Options:**
- `--platform` (`modrinth`, `curseforge`, `minecraft`) and `--output` are required.
- `--list-file` / `-f` is required for modrinth and curseforge platforms (JSON array of mod entries with `id`, `slug`, `version_id` fields).
- `--temp-path` optionally specifies where to store temporary download files.
- Downloaded jars are cached in the temp directory; language files are extracted to `<output>/<platform>-<slug>/`.

### Creating a Mod List

```bash
# Fetch top 1000 Modrinth mods by download count
packtrans-glossary-builder create-mod-list --platform modrinth --output mods.json --count 1000

# Fetch top 500 CurseForge mods (requires CURSEFORGE_API_KEY env var)
packtrans-glossary-builder create-mod-list --platform curseforge --output mods.json --count 500
```

### Querying Translations

```bash
packtrans-glossary --index-path indexes query --lang zh_cn "Cooking Pot" \
  --limit 20
```

**Options:**
- `--index-path`, `--lang`, and query text are required.
- `--limit` is optional; defaults to `20`.
- `--inverse` searches target-language text and returns the source translation.
- `--dict-path` optionally overrides the dictionary storage location.

### Managing Dictionaries

```bash
# Download all Lindera dictionaries
packtrans-glossary dict download

# Download a specific dictionary
packtrans-glossary dict download lindera-ipadic

# List installed dictionaries
packtrans-glossary dict ls

# Delete a dictionary
packtrans-glossary dict delete lindera-ipadic

# Remove old dictionary versions
packtrans-glossary dict clean
```

Available dictionaries: `lindera-ipadic` (Japanese), `lindera-ko-dic` (Korean), `lindera-jieba` (Chinese).

## Query Output

Results are displayed in a tab-separated format sorted by confidence (Tantivy score):

```text
confidence  mod_id          key                                  source       source_lang  target_lang  target
12.94       farmersdelight  block.farmersdelight.cooking_pot     Cooking Pot  en_us        zh_cn        厨锅
```

When `--inverse` is used, the `source` and `target` columns swap semantics (source becomes the target-language text you searched, target becomes the English translation).

## Tantivy Schema

Each indexed document contains the following fields:

| Field        | Type           | Description               |
|--------------|----------------|---------------------------|
| `mod_id`     | Stored string  | Mod identifier            |
| `key`        | Stored string  | Translation key           |
| `source_lang`| Stored string  | Source language code      |
| `source_text`| Indexed + stored | Source text (searched)   |
| `target_lang`| Stored string  | Target language code      |
| `target_text`| Indexed + stored | Target text (searched in inverse mode) |

## Development

### Format and Check

```bash
cargo fmt
cargo clippy
cargo check
```

### Run Tests

```bash
cargo test
```

### Example Workflow

```bash
# Download Minecraft vanilla language files
cargo run --bin packtrans-glossary-builder -- download \
  --platform minecraft --output res

# Or use local files: place language JSON files under res/<modid>/

# Build index
cargo run --bin packtrans-glossary-builder -- --index-path indexes index \
  --scan-dir res \
  --lang zh_cn

# Query index
cargo run --bin packtrans-glossary -- --index-path indexes query --lang zh_cn "Cooking Pot" \
  --limit 10

# Inverse query (search by target language)
cargo run --bin packtrans-glossary -- --index-path indexes query --lang zh_cn "厨锅" \
  --limit 10 --inverse
```

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `CURSEFORGE_API_KEY` | For CurseForge commands | API key for CurseForge API access |

## Future Enhancements

- `--force` flag to rebuild existing indexes
- JSON output mode
- Fuzzy match
- Batch indexing of multiple target languages

## License

[MIT](LICENSE)
