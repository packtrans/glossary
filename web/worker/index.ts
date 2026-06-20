import { INDEX_CDN_URL } from "../src/glossary-index-source.ts";

interface Env {
  ASSETS: { fetch: typeof fetch };
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname === "/glossary-index.zip") {
      const upstream = await fetch(INDEX_CDN_URL);
      if (!upstream.ok) {
        return new Response(`Failed to fetch glossary index (${upstream.status})`, {
          status: upstream.status,
        });
      }

      return new Response(upstream.body, {
        headers: {
          "Content-Type": "application/zip",
          "Cache-Control": "public, max-age=3600",
        },
      });
    }

    return env.ASSETS.fetch(request);
  },
};
