---
name: packtrans-glossary-cli
description: Installs the packtrans-glossary query binary from GitHub releases into bin/, runs translation queries, and manages release indexes and Lindera dictionaries. Use when querying Minecraft mod glossaries, downloading the CLI without building, index dict/serve workflows.
---

# Packtrans Glossary CLI

Query workflow using the **prebuilt** `packtrans-glossary` binary.

## Install binary (skill `bin/`)

`<skill-dir>` is the directory containing this `SKILL.md` (e.g. `skills/packtrans-glossary-cli` or `.agents/skills/packtrans-glossary-cli`).

```bash
bash <skill-dir>/scripts/install-cli.sh
```

Installs to `<skill-dir>/bin/` (same directory as this skill).

**Supported release targets:** `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.

Set `CLI` for commands below (use the same `<skill-dir>`):

```bash
CLI=<skill-dir>/bin/packtrans-glossary   # Unix
# CLI=<skill-dir>/bin/packtrans-glossary.exe  # Windows
```

## Query translations

**Release-managed index** (automatically download from GitHub release; needs network):

```bash
$CLI query --lang zh_cn "Cooking Pot" --limit 10
$CLI query --lang zh_cn "厨锅" --limit 10 --inverse
$CLI query --lang zh_cn "Cooking Pot" --json
```

**Local index** (`--index-dir` is the index **root**; language is appended):

```bash
$CLI query --index-dir indexes --lang zh_cn "Cooking Pot" --limit 20
```


| Flag        | Notes                             |
| ----------- | --------------------------------- |
| `--lang`    | Required (e.g. `zh_cn`, `ja_jp`)  |
| `--limit`   | Default `10`                      |
| `--inverse` | Search target text, return source |
| `--json`    | JSON array (same shape as `serve`) |

**Global flag** (place before the subcommand, e.g. `$CLI --dict-path /path query …`):

| Flag          | Notes                                |
| ------------- | ------------------------------------ |
| `--dict-path` | Override Lindera dictionary location |

Default data dir: `~/.local/share/packtrans-glossary/indexes/` (Linux), `~/Library/Application Support/packtrans-glossary/indexes/` (macOS), `%LOCALAPPDATA%\packtrans-glossary\indexes\` (Windows).

## Manage release indexes

Will be automatically downloaded on first index query for corresponding languages.

```bash
$CLI index download --lang zh_cn
$CLI index upgrade --lang zh_cn
$CLI index ls
$CLI index delete --lang zh_cn
$CLI index clean
```

Layout: `{data_dir}/meta.json` and `{data_dir}/{version}/{lang}/`.

## Dictionaries (CJK tokenization)

Will be automatically downloaded on first index query for languages like `zh_cn`, `ja_jp`, `ko_kr`, `lzh`:

```bash
$CLI dict download
$CLI dict download lindera-jieba
$CLI dict ls
$CLI dict delete lindera-jieba
$CLI dict clean
```

Names: `lindera-ipadic`, `lindera-ko-dic`, `lindera-jieba`.

## Agent checklist

1. If `<skill-dir>/bin/packtrans-glossary` is missing, run `<skill-dir>/scripts/install-cli.sh`.
2. For release queries without `--index-dir`, ensure network; run `index download --lang …` if you need the index before querying.
3. For local `indexes/` trees, pass `--index-dir indexes` (root, not `indexes/zh_cn`).
4. For any other commands not mentioned in this file, view help info with `$CLI --help`.
