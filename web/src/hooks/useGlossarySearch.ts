import { useCallback, useEffect, useRef, useState } from "react";

import type { GlossaryIndex } from "../../pkg/packtrans_glossary_wasm.js";
import { DEMO_LANG, INDEX_URL, type GlossaryStatus, type QueryHit } from "@/types/glossary";

type WasmModule = typeof import("../../pkg/packtrans_glossary_wasm.js");

export function useGlossarySearch() {
  const indexRef = useRef<GlossaryIndex | null>(null);
  const wasmRef = useRef<WasmModule | null>(null);
  const [status, setStatus] = useState<GlossaryStatus>("loading-index");
  const [error, setError] = useState<string | null>(null);
  const [hits, setHits] = useState<QueryHit[]>([]);
  const [lastQuery, setLastQuery] = useState("");

  useEffect(() => {
    let cancelled = false;

    async function loadIndex() {
      try {
        const wasm = await import("../../pkg/packtrans_glossary_wasm.js");
        if (cancelled) {
          return;
        }

        wasmRef.current = wasm;
        const response = await fetch(INDEX_URL);
        if (!response.ok) {
          throw new Error(`Failed to load index (${response.status})`);
        }

        const bytes = new Uint8Array(await response.arrayBuffer());
        const index = new wasm.GlossaryIndex(bytes, DEMO_LANG);
        if (cancelled) {
          index.free();
          return;
        }

        indexRef.current = index;
        setStatus("ready");
        setError(null);
      } catch (loadError) {
        if (!cancelled) {
          setStatus("error");
          setError(loadError instanceof Error ? loadError.message : String(loadError));
        }
      }
    }

    void loadIndex();

    return () => {
      cancelled = true;
      indexRef.current?.free();
      indexRef.current = null;
      wasmRef.current = null;
    };
  }, []);

  const search = useCallback(async (query: string, limit: number) => {
    const trimmed = query.trim();
    if (!trimmed) {
      setError("Enter a search term.");
      setHits([]);
      return;
    }

    const index = indexRef.current;
    if (!index) {
      setError("Glossary index is not ready yet.");
      return;
    }

    setStatus("searching");
    setError(null);
    setLastQuery(trimmed);

    try {
      const result = index.query(trimmed, limit, false) as QueryHit[];
      setHits(result);
      setStatus("ready");
    } catch (searchError) {
      setHits([]);
      setStatus("error");
      setError(searchError instanceof Error ? searchError.message : String(searchError));
    }
  }, []);

  return {
    status,
    error,
    hits,
    lastQuery,
    search,
  };
}
