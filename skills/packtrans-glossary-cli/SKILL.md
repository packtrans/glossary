---
name: packtrans-glossary-cli
description: Installs the packtrans-glossary query binary from GitHub releases into bin/, runs translation queries, and manages release indexes and Lindera dictionaries. Use when querying Minecraft mod glossaries, downloading the CLI without building, index dict/serve workflows, or when the user mentions packtrans-glossary, bin/packtrans-glossary, or release indexes.
---

# Packtrans Glossary CLI

Query-only workflow using the **prebuilt** `packtrans-glossary` binary (not `packtrans-glossary-builder`). Builder/index-from-source workflows stay in [AGENTS.md](../../AGENTS.md).

## Install binary (project `bin/`)

From the repository root:

```bash
bash skills/packtrans-glossary-cli/scripts/install-cli.sh
```

- Writes `bin/packtrans-glossary` (or `bin/packtrans-glossary.exe` on Windows).
- Pulls [packtrans/glossary](https://github.com/packtrans/glossary) **latest** release; archives also contain `packtrans-glossary-builder` — the script copies **only** the query binary.
- Pin a version: `VERSION=v0.0.6 bash skills/packtrans-glossary-cli/scripts/install-cli.sh`
- `bin/` is git-ignored; do not commit binaries.

**Supported release targets:** `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`. Other hosts: `cargo build --release -p packtrans-glossary` and copy from `target/release/`.

Set `CLI` for commands below:

```bash
CLI=./bin/packtrans-glossary   # Unix
# CLI=./bin/packtrans-glossary.exe  # Windows
```

## Query translations

**Release-managed index** (downloads to the default data dir on first use; needs network):

```bash
$CLI query --lang zh_cn "Cooking Pot" --limit 10
$CLI query --lang zh_cn "厨锅" --limit 10 --inverse
$CLI query --lang zh_cn "Cooking Pot" --json
```

**Local index** (`--index-dir` is the index **root**; language is appended):

```bash
$CLI query --index-dir indexes --lang zh_cn "Cooking Pot" --limit 20
```

| Flag | Notes |
|------|--------|
| `--lang` | Required (e.g. `zh_cn`, `ja_jp`) |
| `--limit` | Default `10` |
| `--inverse` | Search target text, return source |
| `--json` | JSON array (same shape as `serve`) |
| `--dict-path` | Override Lindera dictionary location |

Default data dir: `~/.local/share/packtrans-glossary/indexes/` (Linux), `~/Library/Application Support/packtrans-glossary/indexes/` (macOS), `%LOCALAPPDATA%\packtrans-glossary\indexes\` (Windows).

## Manage release indexes

```bash
$CLI index download --lang zh_cn
$CLI index upgrade --lang zh_cn
$CLI index ls
$CLI index delete --lang zh_cn
$CLI index clean
```

Layout: `{data_dir}/meta.json` and `{data_dir}/{version}/{lang}/`. Version checks run at most once per 24h during query.

## Dictionaries (CJK tokenization)

Required on first index build/query for languages like `zh_cn`, `ja_jp`, `ko_kr`, `lzh`:

```bash
$CLI dict download
$CLI dict download lindera-jieba
$CLI dict ls
$CLI dict delete lindera-jieba
$CLI dict clean
```

Names: `lindera-ipadic`, `lindera-ko-dic`, `lindera-jieba`.

## HTTP API (local only)

```bash
$CLI serve
# GET /query?lang=zh_cn&q=Cooking+Pot&limit=20&inverse=false
curl 'http://127.0.0.1:8080/query?lang=zh_cn&q=Cooking+Pot&limit=10'
```

Experimental; default bind `127.0.0.1:8080`. Use `--index-dir` for local indexes.

## Agent checklist

1. If `bin/packtrans-glossary` is missing, run `install-cli.sh` (or build with cargo).
2. For release queries without `--index-dir`, ensure network; run `index download --lang …` if you need the index before querying.
3. For local `indexes/` trees, pass `--index-dir indexes` (root, not `indexes/zh_cn`).
4. Prefer `$CLI` over `cargo run` when validating release behavior or avoiding compile time.

## Source build (fallback)

```bash
cargo build --release -p packtrans-glossary
cp target/release/packtrans-glossary bin/
```
