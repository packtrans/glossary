# Packtrans Glossary Web

Static single-page app to query glossary indexes in the browser. No SSR and no backend API: assets are fetched from the Packtrans CDN and search runs in WebAssembly (`packtrans-glossary-wasm`).

## CDN layout

Same filenames as [packtrans/glossary-indexes](https://github.com/packtrans/glossary-indexes) and [Lindera releases](https://github.com/lindera/lindera/releases):

- Index: `https://cdn.packtrans.download/glossary/index/{releaseTag}/packtrans-glossary-index-{lang}-{date}.zip`
- Dictionary: `https://cdn.packtrans.download/glossary/dict/{linderaVersion}/lindera-{dict}-{linderaVersion}.zip`

Release metadata (tag, asset names, languages) is read from GitHub when the app loads.

## Development

```sh
# From repo root — refresh WASM after Rust changes
cd crates/packtrans-glossary-wasm
wasm-pack build --target web --out-dir ../../web/src/wasm --release

cd ../../web
npm install
npm run dev
```

## Production build

```sh
npm run build
```

Deploy the contents of `web/dist/` to any static host.
