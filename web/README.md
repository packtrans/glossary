# PackTrans Glossary Web

Static React demo for `packtrans-glossary-wasm`, powered by [Vite+](https://viteplus.dev/).

## Prerequisites

- [`vp` CLI](https://viteplus.dev/) (install: `curl -fsSL https://vite.plus | bash`)
- Rust 1.85+ and `wasm-pack`

Vite+ manages the Node.js runtime and pnpm version for this project.

## Commands

From the repo root (pnpm workspace):

```sh
vp install   # install all workspace dependencies
pnpm build   # build web (WASM + Vite)
pnpm deploy  # build and wrangler deploy
```

From `web/`:

```sh
vp install   # install dependencies (pnpm)
vp dev       # dev server (runs WASM prebuild via predev)
vp check     # format + lint + type-check
vp build     # production build (runs WASM prebuild via prebuild)
vp preview   # preview via Workers runtime (Cloudflare Vite plugin)
```

Rebuild the WASM bindings manually when needed:

```sh
vp run build:wasm
```

`predev` / `prebuild` run `vp run build:wasm` automatically.

## Demo scope

- Hardcoded `zh_cn` language
- Source-to-target search only (inverse CJK queries are not supported in WASM)
- Pure static site: the glossary index zip is fetched cross-origin from PackTrans CDN (CORS-enabled) and loaded into WASM memory in the browser

CDN index URL (`src/types/glossary.ts`):

`https://cdn.packtrans.download/glossary/packtrans-glossary-index-zh_cn-20260601.zip`

Local dev uses `http://localhost:5173` (Vite `server.host`) so the browser origin matches CDN CORS. `127.0.0.1:5173` is a different origin and will be blocked unless you add it on the CDN too.

## Tooling notes

- Lint/format config lives in `vite.config.ts` (`lint`, `fmt` blocks) — ESLint was removed during Vite+ migration
- `vp check` replaces separate `eslint` + `tsc` runs for local validation
- `vp test` is available when you add `*.test.ts(x)` files; there are none in this demo yet

## Deploy to Cloudflare Workers

The app is a static SPA deployed with the [Cloudflare Vite plugin](https://developers.cloudflare.com/workers/vite-plugin/). Input configuration is in `wrangler.toml`; `vp build` emits client assets and an output `wrangler.json` under `dist/` for preview and deploy.

After `wrangler login` (or with `CLOUDFLARE_API_TOKEN` in CI):

```sh
# from repo root
pnpm deploy

# or from web/
vp run deploy
```

Requirements: Wrangler **4.102.0+** (included as a dev dependency).

### Local Workers preview

Preview the production build in the Workers runtime (matches deployed behavior):

```sh
cd web
vp build
vp preview
```
