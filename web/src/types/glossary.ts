export interface QueryHit {
  confidence: number;
  mod_id: string;
  key: string;
  source: string;
  source_lang: string;
  target_lang: string;
  target: string;
}

export type GlossaryStatus = "loading-index" | "ready" | "searching" | "error";

export const DEMO_LANG = "zh_cn" as const;

export const INDEX_CDN_URL =
  "https://cdn.packtrans.download/glossary/packtrans-glossary-index-zh_cn-20260601.zip";

export const INDEX_URL = INDEX_CDN_URL;
