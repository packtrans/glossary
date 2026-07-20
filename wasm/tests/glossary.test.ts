import { createRequire } from "node:module";
import { beforeAll, describe, expect, it } from "vitest";

import {
  loadCliDictionaryZip,
  loadCliIndexZip,
  requireCliDictionaryDir,
  requireCliIndexDir,
  TEST_LANG,
} from "./cli-cache.js";

const require = createRequire(import.meta.url);
const { GlossaryIndex, lindera_version, query } = require("../pkg/packtrans_glossary_wasm.js");

interface QueryHit {
  confidence: number;
  mod_id: string;
  key: string;
  source: string;
  source_lang: string;
  target_lang: string;
  target: string;
}

describe("packtrans-glossary-wasm", () => {
  let indexZip: Uint8Array;
  let dictZip: Uint8Array;

  beforeAll(() => {
    requireCliIndexDir(TEST_LANG);
    requireCliDictionaryDir(TEST_LANG);
    indexZip = loadCliIndexZip(TEST_LANG);
    dictZip = loadCliDictionaryZip(TEST_LANG);
  });

  it("exports the Lindera version for dictionary zip downloads", () => {
    expect(lindera_version()).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it("runs a forward query on the CLI-managed zh_cn index", () => {
    const index = new GlossaryIndex(indexZip, TEST_LANG);

    const hits = index.query("Cooking Pot", 10, false) as QueryHit[];

    expect(hits.length).toBeGreaterThan(0);
    const hit = hits.find((entry) => entry.key.includes("cooking_pot"));
    expect(hit).toBeDefined();
    expect(hit?.source).toBe("Cooking Pot");
    expect(hit?.target.length).toBeGreaterThan(0);
  });

  it("runs an inverse query with the CLI dictionary cache", () => {
    const index = new GlossaryIndex(indexZip, TEST_LANG, dictZip);

    const hits = index.query("厨锅", 10, true) as QueryHit[];

    expect(hits.length).toBeGreaterThan(0);
    const hit = hits.find((entry) => entry.source.includes("厨锅"));
    expect(hit).toBeDefined();
    expect(hit?.target).toMatch(/Cooking Pot/i);
  });

  it("supports the one-shot query helper", () => {
    const hits = query(indexZip, TEST_LANG, "Cooking Pot", 10, false) as QueryHit[];

    expect(hits.length).toBeGreaterThan(0);
    expect(hits.some((entry) => entry.source === "Cooking Pot")).toBe(true);
  });

  it("runs a forward regex query on the CLI-managed zh_cn index", () => {
    const index = new GlossaryIndex(indexZip, TEST_LANG);

    const hits = index.query("cook.*", 10, false, true) as QueryHit[];

    expect(hits.length).toBeGreaterThan(0);
    const hit = hits.find((entry) => entry.key.includes("cooking_pot"));
    expect(hit).toBeDefined();
    expect(hit?.source).toBe("Cooking Pot");
  });

  it("runs an inverse regex query with the CLI dictionary cache", () => {
    const index = new GlossaryIndex(indexZip, TEST_LANG, dictZip);

    const hits = index.query("锅", 50, true, true) as QueryHit[];

    expect(hits.length).toBeGreaterThan(0);
    const hit = hits.find(
      (entry) => entry.key.includes("cooking_pot") && entry.source === "厨锅",
    );
    expect(hit).toBeDefined();
    expect(hit?.target).toMatch(/Cooking Pot/i);
  });

  it("rejects invalid dictionary zip bytes at construction", () => {
    expect(() => new GlossaryIndex(indexZip, TEST_LANG, new Uint8Array([1, 2, 3]))).toThrow(
      /dictionary/i,
    );
  });
});
