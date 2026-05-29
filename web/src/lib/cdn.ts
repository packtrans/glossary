/** CDN root for glossary release assets (mirrors GitHub release filenames). */
export const CDN_BASE = "https://cdn.packtrans.download/glossary";

/** Index zip: `packtrans-glossary-index-{lang}-{date}.zip` */
export function indexZipUrl(version: string, filename: string): string {
  return `${CDN_BASE}/index/${encodeURIComponent(version)}/${encodeURIComponent(filename)}`;
}

/** Dictionary zip: `lindera-{name}-{linderaVersion}.zip` (same as Lindera GitHub release). */
export function dictZipUrl(linderaVersion: string, dictName: string): string {
  const filename = `${dictName}-${linderaVersion}.zip`;
  return `${CDN_BASE}/dict/${encodeURIComponent(linderaVersion)}/${encodeURIComponent(filename)}`;
}

export function indexAssetPrefix(lang: string): string {
  return `packtrans-glossary-index-${lang}-`;
}
