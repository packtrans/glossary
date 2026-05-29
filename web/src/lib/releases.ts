import { indexAssetPrefix } from "./cdn";

const GLOSSARY_INDEXES_LATEST =
  "https://api.github.com/repos/packtrans/glossary-indexes/releases/latest";

const LANGUAGES_JSON_BASE =
  "https://raw.githubusercontent.com/packtrans/glossary-indexes/refs/tags";

export type GlossaryRelease = {
  tagName: string;
  indexAssets: Map<string, string>;
};

export async function fetchLatestGlossaryRelease(): Promise<GlossaryRelease> {
  const response = await fetch(GLOSSARY_INDEXES_LATEST);
  if (!response.ok) {
    throw new Error(`Failed to fetch glossary release (${response.status})`);
  }
  const body = (await response.json()) as {
    tag_name?: string;
    assets?: Array<{ name?: string }>;
  };
  const tagName = body.tag_name;
  if (!tagName) {
    throw new Error("Release response did not include tag_name");
  }

  const indexAssets = new Map<string, string>();
  for (const asset of body.assets ?? []) {
    const name = asset.name;
    if (!name?.endsWith(".zip")) {
      continue;
    }
    for (const lang of ["ja_jp", "ko_kr", "zh_cn", "lzh", "en_us"]) {
      if (name.startsWith(indexAssetPrefix(lang))) {
        indexAssets.set(lang, name);
        break;
      }
    }
    if (name.startsWith("packtrans-glossary-index-")) {
      const rest = name.slice("packtrans-glossary-index-".length);
      const lang = rest.replace(/-\d{8}\.zip$/, "");
      if (lang) {
        indexAssets.set(lang, name);
      }
    }
  }

  return { tagName, indexAssets };
}

export async function fetchAvailableLanguages(tag: string): Promise<string[]> {
  const url = `${LANGUAGES_JSON_BASE}/${encodeURIComponent(tag)}/languages.json`;
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch languages.json (${response.status})`);
  }
  const value = (await response.json()) as unknown;
  if (Array.isArray(value)) {
    return value.filter((item): item is string => typeof item === "string");
  }
  if (value && typeof value === "object" && "languages" in value) {
    const langs = (value as { languages?: unknown }).languages;
    if (Array.isArray(langs)) {
      return langs.filter((item): item is string => typeof item === "string");
    }
  }
  throw new Error("languages.json did not contain a language list");
}
