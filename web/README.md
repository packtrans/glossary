# PackTrans Glossary Web

Static React demo for `packtrans-glossary-wasm`, powered by [Vite+](https://viteplus.dev/).

## Prerequisites

- [`vp` CLI](https://viteplus.dev/) (install: `curl -fsSL https://vite.plus | bash`)
- Rust 1.85+ and `wasm-pack`

Vite+ manages the Node.js runtime and pnpm version for this project.

## Commands

Use `vp` for day-to-day work:

```sh
cd web
vp install   # install dependencies (pnpm)
vp dev       # dev server (runs WASM prebuild via predev)
vp check     # format + lint + type-check
vp build     # production build (runs WASM prebuild via prebuild)
vp preview   # serve dist/ locally
```

Rebuild the WASM bindings manually when needed:

```sh
vp run build:wasm
```

`predev` / `prebuild` run `vp run build:wasm` automatically.

## Demo scope

- Hardcoded `zh_cn` language
- Source-to-target search only (inverse CJK queries are not supported in WASM)
- Pure static site: the glossary index zip is fetched at runtime from PackTrans CDN (via a same-origin proxy) and loaded into WASM memory in the browser

CDN source (proxied at `/glossary-index.zip`):

`https://cdn.packtrans.download/glossary/packtrans-glossary-index-zh_cn-20260601.zip`

## Tooling notes

- Lint/format config lives in `vite.config.ts` (`lint`, `fmt` blocks) — ESLint was removed during Vite+ migration
- `vp check` replaces separate `eslint` + `tsc` runs for local validation
- `vp test` is available when you add `*.test.ts(x)` files; there are none in this demo yet

## Deploy to Cloudflare Workers

The app is a static SPA served from `dist/` via [Workers Static Assets](https://developers.cloudflare.com/workers/static-assets/). Configuration is in `wrangler.jsonc`.

### Temporary preview deploy (no Cloudflare account)

For agent/CI-less previews, use Wrangler’s temporary account flow ([docs](https://developers.cloudflare.com/workers/platform/claim-deployments/)):

```sh
cd web
vp run deploy:temporary
```

This runs `vp build`, then `wrangler deploy --temporary`. Wrangler prints:

- A `workers.dev` URL for the deployment
- A **claim URL** (valid ~60 minutes) to transfer the preview account to your Cloudflare account

**Important:** open the claim URL before it expires if you want to keep the deployment. After claiming, run `wrangler login` and use `vp run deploy` (without `--temporary`) for permanent deploys.

Requirements: Wrangler **4.102.0+** (included as a dev dependency).

### Permanent deploy

After `wrangler login` (or with `CLOUDFLARE_API_TOKEN` in CI):

```sh
cd web
vp run deploy
```

### Local Workers preview

Serve the built `dist/` through the Workers runtime locally:

```sh
cd web
vp build
vp exec wrangler dev
```
