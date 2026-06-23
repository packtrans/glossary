import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { basename, join, relative } from "node:path";
import { homedir } from "node:os";
import { zipSync } from "fflate";

const TEST_LANG = "zh_cn";

const DICTIONARY_BY_LANG: Record<string, string> = {
  lzh: "lindera-jieba",
  zh_cn: "lindera-jieba",
  ja_jp: "lindera-ipadic",
  ko_kr: "lindera-ko-dic",
};

function resolveDataDir(): string {
  if (process.env.XDG_DATA_HOME) {
    return process.env.XDG_DATA_HOME;
  }
  if (process.platform === "darwin") {
    return join(homedir(), "Library", "Application Support");
  }
  if (process.platform === "win32") {
    const localAppData = process.env.LOCALAPPDATA;
    if (!localAppData) {
      throw new Error("LOCALAPPDATA is not set");
    }
    return localAppData;
  }
  return join(homedir(), ".local", "share");
}

function indexesRoot(): string {
  return join(resolveDataDir(), "packtrans-glossary", "indexes");
}

function dictionariesRoot(): string {
  return join(resolveDataDir(), "packtrans-glossary", "dictionaries");
}

function dictionaryNameForLang(lang: string): string {
  if (lang === "lzh" || lang.startsWith("zh")) {
    return "lindera-jieba";
  }
  if (lang.startsWith("ja")) {
    return "lindera-ipadic";
  }
  if (lang.startsWith("ko")) {
    return "lindera-ko-dic";
  }
  throw new Error(`no Lindera dictionary mapping for language: ${lang}`);
}

function listVersionDirs(root: string): string[] {
  if (!existsSync(root)) {
    return [];
  }
  return readdirSync(root)
    .filter((entry) => {
      const path = join(root, entry);
      return statSync(path).isDirectory();
    })
    .sort()
    .reverse();
}

export function requireCliIndexDir(lang: string = TEST_LANG): string {
  const root = indexesRoot();
  const metaPath = join(root, "meta.json");

  const candidates: string[] = [];
  if (existsSync(metaPath)) {
    const meta = JSON.parse(readFileSync(metaPath, "utf8")) as {
      current_version?: string;
    };
    if (meta.current_version) {
      candidates.push(join(root, meta.current_version, lang));
    }
  }

  for (const version of listVersionDirs(root)) {
    candidates.push(join(root, version, lang));
  }

  for (const path of candidates) {
    if (existsSync(path) && statSync(path).isDirectory()) {
      return path;
    }
  }

  throw new Error(
    [
      `CLI index cache not found for '${lang}'.`,
      `Expected a directory under ${root}/<version>/${lang}.`,
      "Download it first:",
      `  packtrans-glossary index download --lang ${lang}`,
    ].join("\n"),
  );
}

export function requireCliDictionaryDir(lang: string = TEST_LANG): string {
  const dictName = dictionaryNameForLang(lang);
  const root = dictionariesRoot();

  for (const version of listVersionDirs(root)) {
    const path = join(root, version, dictName);
    if (existsSync(path) && statSync(path).isDirectory()) {
      return path;
    }
  }

  throw new Error(
    [
      `CLI dictionary cache not found for '${dictName}' (lang: ${lang}).`,
      `Expected a directory under ${root}/<version>/${dictName}.`,
      "Download it first:",
      `  packtrans-glossary dict download ${dictName}`,
    ].join("\n"),
  );
}

function shouldSkipZipEntry(name: string): boolean {
  return name.endsWith(".lock");
}

export function zipDirectory(dir: string, entryPrefix: string): Uint8Array {
  const files: Record<string, Uint8Array> = {};

  function walk(current: string): void {
    for (const entry of readdirSync(current)) {
      const fullPath = join(current, entry);
      const relPath = join(entryPrefix, relative(dir, fullPath)).replace(/\\/g, "/");
      if (statSync(fullPath).isDirectory()) {
        walk(fullPath);
        continue;
      }
      if (shouldSkipZipEntry(entry)) {
        continue;
      }
      files[relPath] = readFileSync(fullPath);
    }
  }

  walk(dir);
  return zipSync(files);
}

export function loadCliIndexZip(lang: string = TEST_LANG): Uint8Array {
  const indexDir = requireCliIndexDir(lang);
  return zipDirectory(indexDir, lang);
}

export function loadCliDictionaryZip(lang: string = TEST_LANG): Uint8Array {
  const dictDir = requireCliDictionaryDir(lang);
  const dictName = basename(dictDir);
  const version = basename(join(dictDir, ".."));
  return zipDirectory(dictDir, `${dictName}-${version}`);
}

export { TEST_LANG, DICTIONARY_BY_LANG };
