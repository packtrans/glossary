# Packtrans Glossary

Rust-based CLI tools for indexing and querying Minecraft mod glossary translations. Uses [Tantivy](https://github.com/quickwit-oss/tantivy) for fast full-text search and relevance scoring.

We have also built a web App based on this repository at [packtrans/glossary-web](https://github.com/packtrans/glossary-web), try it on [https://glossary.packtrans.download](https://glossary.packtrans.download).


## Installation

Download pre-built binary from [Releases](https://github.com/packtrans/glossary/releases).

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
- Download and query release-managed glossary indexes from [packtrans/glossary-indexes](https://github.com/packtrans/glossary-indexes).
- HTTP API via `serve` for programmatic queries (JSON responses).

## Project Structure

```text
crates/
├── packtrans-glossary-core       # Shared core library (schema, indexing, querying, tokenizers, utilities)
├── packtrans-glossary            # End-user query CLI
├── packtrans-glossary-builder    # Index builder CLI (index, create-mod-list, download)
└── packtrans-glossary-wasm       # WASM bindings for browser use (@packtrans/glossary)
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
packtrans-glossary-builder index \
  --scan-dir res \
  --lang zh_cn \
  --out indexes
```

Writes the Tantivy index to `indexes/zh_cn/` (`--out` is an index root; `--lang` is appended).

**Options:**

- `--scan-dir`, `--lang`, and `--out` are required.
- `--dict-path` optionally overrides the dictionary storage location (global builder flag).
- Source language is always `en_us`.
- Scans all direct child directories under `--scan-dir`; each is treated as one mod.
- Skips mods with missing source or target language files (no error; summary reports total mods and mods with both language files).
- Malformed language JSON is skipped with a warning; indexing still completes successfully.
- Fails if the index already exists.
- Never writes to the system data directory; use `--out` for the destination.

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
# Release-managed index (downloads to the default data dir when needed)
packtrans-glossary query --lang zh_cn "Cooking Pot" --limit 20

# Local index built with the builder
packtrans-glossary query --index-dir indexes --lang zh_cn "Cooking Pot" --limit 20
```

**Options:**

- `--lang` and query text are required.
- `--index-dir` is an index root; the index at `{index-dir}/{lang}` is used (same layout as `index --out`). When omitted, a release index is downloaded or opened from the default data directory.
- `--limit` is optional; defaults to `10`.
- `--inverse` searches target-language text and returns the source translation.
- `--dict-path` optionally overrides the dictionary storage location (global query flag).
- `--json` prints results as a JSON array (same shape as the `serve` HTTP API).

### HTTP Server

> The `serve` command is intended for ad-hoc queries on your own machine (default bind `127.0.0.1`). It is not designed for production use or a large number of parallel requests. For heavy or concurrent workloads, use the `query` CLI instead.

Start an HTTP server that exposes glossary search as JSON:

```bash
# Default: bind 127.0.0.1:8080, use release-managed indexes
packtrans-glossary serve

# Custom host/port and local index root
packtrans-glossary serve --host 0.0.0.0 --port 3000 --index-dir indexes
```

**Endpoint:** `GET /query`

| Parameter      | Required | Default | Description                         |
| -------------- | -------- | ------- | ----------------------------------- |
| `lang`         | yes      | —       | Target language code (e.g. `zh_cn`) |
| `q` or `query` | yes      | —       | Search text                         |
| `limit`        | no       | `10`    | Maximum results (max `50`)          |
| `inverse`      | no       | `false` | Search target text, return source   |

Example:

```bash
curl 'http://127.0.0.1:8080/query?lang=zh_cn&q=Cooking+Pot&limit=20&inverse=false'
```

Returns a JSON array of hits with `confidence`, `mod_id`, `key`, `source`, `source_lang`, `target_lang`, and `target`. Errors return `{ "error": "..." }` with HTTP 400 (bad request) or 500 (internal error).

**Options:**

- `--host` defaults to `127.0.0.1`.
- `--port` defaults to `8080`.
- `--index-dir` is an index root (same layout as `query --index-dir`). When omitted, release indexes are used from the default data directory.
- `--dict-path` optionally overrides the dictionary storage location (global query flag).

### Managing Release Indexes

```bash
# Download the latest release index for a language
packtrans-glossary index download --lang zh_cn

# Upgrade (download latest and remove older versions)
packtrans-glossary index upgrade --lang zh_cn

# List installed indexes
packtrans-glossary index ls

# Delete an index
packtrans-glossary index delete --lang zh_cn

# Remove old version directories
packtrans-glossary index clean
```

Release indexes are stored under the default data directory:

```text
index-root/
├── meta.json
└── {version}/{lang}/
```

Version checks run at most once per 24 hours during query resolution.

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

## WASM (`@packtrans/glossary`)

The `packtrans-glossary-wasm` crate is published as [`@packtrans/glossary`](https://github.com/packtrans/glossary/pkgs/npm/glossary) for browser use. JavaScript fetches index zip bytes (and optionally a Lindera dictionary zip) and passes them into the WASM bindings.

```ts
import init, { GlossaryIndex, lindera_version } from "@packtrans/glossary";

await init();

const indexZip = await fetch(indexUrl).then((r) => r.arrayBuffer());

// Forward query (default tokenizer)
const index = new GlossaryIndex(new Uint8Array(indexZip), "zh_cn");
const hits = index.query("Cooking Pot", 10, false);

// Inverse query with a Lindera dictionary fetched by JS
const dictUrl = `https://github.com/lindera/lindera/releases/download/v${lindera_version()}/lindera-jieba-${lindera_version()}.zip`;
const dictZip = await fetch(dictUrl).then((r) => r.arrayBuffer());
const inverseIndex = new GlossaryIndex(
  new Uint8Array(indexZip),
  "zh_cn",
  new Uint8Array(dictZip),
);
const inverseHits = inverseIndex.query("厨锅", 10, true);

// Regex query (matches indexed terms; set the 4th argument to true)
const regexHits = index.query("cook.*", 10, false, true);
const inverseRegexHits = inverseIndex.query("锅", 50, true, true);
```

- `dictZip` is optional. When provided at construction, WASM loads the dictionary and registers the matching Lindera tokenizer. When omitted, the default tokenizer is used.
- Use `lindera_version()` to build dictionary download URLs that match the Lindera release this WASM build expects.
- Dictionary release archives use the same Lindera URLs as `packtrans-glossary dict download`.
- `inverse=true` searches target-language text and returns source-language results (same semantics as the CLI `--inverse` flag).
- `regex=true` (optional 4th argument to `query`) treats the query string as a [Rust regular expression](https://docs.rs/regex/latest/regex/) matched against indexed terms in the selected search field. Regex queries match tokenized terms, not raw stored text; use a Lindera dictionary for CJK inverse queries.

Node.js integration tests live in [`wasm/`](wasm/README.md) (Vitest + `wasm-pack` nodejs target). They read the CLI-managed index and dictionary caches; download `zh_cn` and `lindera-jieba` before running `pnpm test`.

## Query Output

Results are displayed in a tab-separated format sorted by confidence (Tantivy score):

```text
confidence  mod_id          key                                  source       source_lang  target_lang  target
12.94       farmersdelight  block.farmersdelight.cooking_pot     Cooking Pot  en_us        zh_cn        厨锅
```

When `--inverse` is used, the `source` and `target` columns swap semantics (source becomes the target-language text you searched, target becomes the English translation).

## Tantivy Schema

Each indexed document contains the following fields:

| Field         | Type             | Description                            |
| ------------- | ---------------- | -------------------------------------- |
| `mod_id`      | Stored string    | Mod identifier                         |
| `key`         | Stored string    | Translation key                        |
| `source_lang` | Stored string    | Source language code                   |
| `source_text` | Indexed + stored | Source text (searched)                 |
| `target_lang` | Stored string    | Target language code                   |
| `target_text` | Indexed + stored | Target text (searched in inverse mode) |

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

# Build a local index
cargo run --bin packtrans-glossary-builder -- index \
  --scan-dir res --lang zh_cn --out indexes

# Query the local index
cargo run --bin packtrans-glossary -- query \
  --index-dir indexes --lang zh_cn "Cooking Pot" --limit 10

# Or query a release-managed index (no --index-dir)
cargo run --bin packtrans-glossary -- query --lang zh_cn "Cooking Pot" --limit 10

# Inverse query (search by target language)
cargo run --bin packtrans-glossary -- query \
  --index-dir indexes --lang zh_cn "厨锅" --limit 10 --inverse

# HTTP server (then query via curl)
cargo run --bin packtrans-glossary -- serve --index-dir indexes
# curl 'http://127.0.0.1:8080/query?lang=zh_cn&q=Cooking+Pot&limit=10'
```

## Environment Variables

| Variable             | Required                | Description                       |
| -------------------- | ----------------------- | --------------------------------- |
| `CURSEFORGE_API_KEY` | For CurseForge commands | API key for CurseForge API access |

## Future Enhancements

- `--force` flag to rebuild existing indexes
- Fuzzy match
- Batch indexing of multiple target languages

## License

[MIT](LICENSE)
