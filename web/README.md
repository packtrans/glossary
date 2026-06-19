# PackTrans Glossary Web

Static React demo for `packtrans-glossary-wasm`.

## Prerequisites

- Node.js 22+
- Rust 1.85+ and `wasm-pack`

## Commands

```sh
cd web
npm install
npm run dev
```

`predev` builds the WASM package and ensures `public/indexes/zh_cn.zip` exists (copies the committed sample index when missing).

```sh
# Optional: replace the bundled index with the latest release asset
npm run download-index

# Regenerate the committed sample index from Rust test fixtures
npm run generate-sample-index
```

```sh
npm run build   # static output in dist/
npm run preview # serve dist/ locally
```

## Demo scope

- Hardcoded `zh_cn` language
- Source-to-target search only (inverse CJK queries are not supported in WASM)
- Pure static site: index zip is served from `public/` and loaded into WASM memory in the browser
