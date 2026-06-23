import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const { GlossaryIndex, query } = require("../pkg/packtrans_glossary_wasm.js");

const testDir = dirname(fileURLToPath(import.meta.url));
const fixturesDir = join(testDir, "..", "fixtures");

interface QueryHit {
  confidence: number;
  mod_id: string;
  key: string;
  source: string;
  source_lang: string;
  target_lang: string;
  target: string;
}

function loadFixture(lang: string): Uint8Array {
  return new Uint8Array(readFileSync(join(fixturesDir, `${lang}.zip`)));
}

describe("packtrans-glossary-wasm", () => {
  it("runs a forward query on a French index", () => {
    const indexZip = loadFixture("fr_fr");
    const index = new GlossaryIndex(indexZip, "fr_fr");

    const hits = index.query("Cooking Pot", 10, false) as QueryHit[];

    expect(hits).toHaveLength(1);
    expect(hits[0].mod_id).toBe("farmersdelight");
    expect(hits[0].key).toBe("block.farmersdelight.cooking_pot");
    expect(hits[0].source).toBe("Cooking Pot");
    expect(hits[0].target).toBe("Marmite");
  });

  it("runs an inverse query on a French index", () => {
    const indexZip = loadFixture("fr_fr");
    const index = new GlossaryIndex(indexZip, "fr_fr");

    const hits = index.query("Marmite", 10, true) as QueryHit[];

    expect(hits).toHaveLength(1);
    expect(hits[0].source).toBe("Marmite");
    expect(hits[0].source_lang).toBe("fr_fr");
    expect(hits[0].target).toBe("Cooking Pot");
    expect(hits[0].target_lang).toBe("en_us");
  });

  it("runs an inverse query on a Chinese index without a dictionary", () => {
    const indexZip = loadFixture("zh_cn");
    const index = new GlossaryIndex(indexZip, "zh_cn");

    const hits = index.query("厨锅", 10, true) as QueryHit[];

    expect(hits).toHaveLength(1);
    expect(hits[0].source).toBe("厨锅");
    expect(hits[0].target).toBe("Cooking Pot");
  });

  it("supports the one-shot query helper", () => {
    const indexZip = loadFixture("fr_fr");

    const hits = query(indexZip, "fr_fr", "Cooking Pot", 10, false) as QueryHit[];

    expect(hits).toHaveLength(1);
    expect(hits[0].target).toBe("Marmite");
  });

  it("rejects invalid dictionary zip bytes at construction", () => {
    const indexZip = loadFixture("zh_cn");

    expect(() => new GlossaryIndex(indexZip, "zh_cn", new Uint8Array([1, 2, 3]))).toThrow(
      /dictionary/i,
    );
  });
});
