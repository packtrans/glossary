import initWasm, {
  GlossaryEngine,
  lindera_version,
  target_tokenizer_name,
} from "@/wasm/packtrans_glossary_wasm";
import { dictZipUrl, indexZipUrl } from "./cdn";

export type QueryHit = {
  confidence: number;
  mod_id: string;
  key: string;
  source: string;
  source_lang: string;
  target_lang: string;
  target: string;
};

let wasmReady = false;

export async function ensureWasm(): Promise<void> {
  if (wasmReady) {
    return;
  }
  await initWasm();
  wasmReady = true;
}

export function getLinderaVersion(): string {
  return lindera_version();
}

export function getTokenizerName(lang: string): string {
  return target_tokenizer_name(lang);
}

async function fetchZip(url: string): Promise<Uint8Array> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Download failed (${response.status}): ${url}`);
  }
  const buffer = await response.arrayBuffer();
  return new Uint8Array(buffer);
}

export async function createEngine(options: {
  lang: string;
  releaseTag: string;
  indexFilename: string;
}): Promise<GlossaryEngine> {
  await ensureWasm();
  const indexBytes = await fetchZip(
    indexZipUrl(options.releaseTag, options.indexFilename),
  );

  const tokenizer = getTokenizerName(options.lang);
  let dictBytes: Uint8Array | null = null;
  if (tokenizer !== "default") {
    const version = getLinderaVersion();
    dictBytes = await fetchZip(dictZipUrl(version, tokenizer));
  }

  return new GlossaryEngine(indexBytes, options.lang, dictBytes);
}

export function runQuery(
  engine: GlossaryEngine,
  text: string,
  limit: number,
  inverse: boolean,
): QueryHit[] {
  return engine.query(text, limit, inverse) as QueryHit[];
}
