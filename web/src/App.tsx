import { Search } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { GlossaryEngine } from "@/wasm/packtrans_glossary_wasm";
import {
  ensureWasm,
  createEngine,
  getLinderaVersion,
  getTokenizerName,
  runQuery,
  type QueryHit,
} from "@/lib/glossary";
import {
  fetchAvailableLanguages,
  fetchLatestGlossaryRelease,
  type GlossaryRelease,
} from "@/lib/releases";

export default function App() {
  const [release, setRelease] = useState<GlossaryRelease | null>(null);
  const [languages, setLanguages] = useState<string[]>([]);
  const [lang, setLang] = useState("zh_cn");
  const [engine, setEngine] = useState<GlossaryEngine | null>(null);
  const [query, setQuery] = useState("Cooking Pot");
  const [limit, setLimit] = useState(10);
  const [inverse, setInverse] = useState(false);
  const [hits, setHits] = useState<QueryHit[]>([]);
  const [status, setStatus] = useState("Loading release metadata…");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const latest = await fetchLatestGlossaryRelease();
        await ensureWasm();
        const langs = await fetchAvailableLanguages(latest.tagName);
        if (cancelled) {
          return;
        }
        setRelease(latest);
        setLanguages(langs.length > 0 ? langs : [...latest.indexAssets.keys()]);
        if (langs.includes("zh_cn")) {
          setLang("zh_cn");
        } else if (langs[0]) {
          setLang(langs[0]);
        }
        setStatus(`Release ${latest.tagName} · Lindera ${getLinderaVersion()}`);
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const loadIndex = useCallback(async () => {
    if (!release) {
      return;
    }
    const filename = release.indexAssets.get(lang);
    if (!filename) {
      setError(`No index asset for ${lang} in release ${release.tagName}`);
      return;
    }

    setLoading(true);
    setError(null);
    setStatus(`Downloading index and dictionary for ${lang}…`);
    try {
      engine?.free();
      const next = await createEngine({
        lang,
        releaseTag: release.tagName,
        indexFilename: filename,
      });
      setEngine(next);
      setHits([]);
      setStatus(`Ready · ${lang} @ ${release.tagName}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setEngine(null);
    } finally {
      setLoading(false);
    }
  }, [engine, lang, release]);

  const onSearch = useCallback(() => {
    if (!engine || !query.trim()) {
      return;
    }
    setError(null);
    try {
      const results = runQuery(engine, query.trim(), limit, inverse);
      setHits(results);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [engine, inverse, limit, query]);

  const tokenizer = getTokenizerName(lang);
  const indexFile = release?.indexAssets.get(lang);

  return (
    <div className="min-h-screen bg-gradient-to-b from-background to-muted/30">
      <div className="mx-auto flex max-w-5xl flex-col gap-6 px-4 py-10">
        <header className="space-y-2">
          <h1 className="text-3xl font-bold tracking-tight">Packtrans Glossary</h1>
          <p className="text-muted-foreground max-w-2xl">
            Static client-side search over Minecraft mod translation glossaries. Indexes
            and Lindera dictionaries are loaded from the Packtrans CDN (same filenames as
            GitHub releases); queries run in WebAssembly via Tantivy.
          </p>
          <div className="flex flex-wrap gap-2">
            <Badge variant="secondary">{status}</Badge>
            {tokenizer !== "default" && (
              <Badge variant="outline">Tokenizer: {tokenizer}</Badge>
            )}
            {indexFile && <Badge variant="outline">{indexFile}</Badge>}
          </div>
        </header>

        <Card>
          <CardHeader>
            <CardTitle>Search</CardTitle>
            <CardDescription>
              Choose a target language, load the index once, then search source or target
              text.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
              <div className="space-y-2">
                <label className="text-sm font-medium" htmlFor="lang">
                  Language
                </label>
                <Select value={lang} onValueChange={setLang} disabled={loading}>
                  <SelectTrigger id="lang">
                    <SelectValue placeholder="Language" />
                  </SelectTrigger>
                  <SelectContent>
                    {languages.map((code) => (
                      <SelectItem key={code} value={code}>
                        {code}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium" htmlFor="limit">
                  Limit
                </label>
                <Input
                  id="limit"
                  type="number"
                  min={1}
                  max={100}
                  value={limit}
                  onChange={(e) => setLimit(Number(e.target.value) || 10)}
                />
              </div>
              <div className="flex items-end gap-2 sm:col-span-2">
                <Button onClick={loadIndex} disabled={loading || !release}>
                  {loading ? "Loading…" : "Load index"}
                </Button>
                <Button
                  variant="outline"
                  onClick={() => setInverse((v) => !v)}
                  type="button"
                >
                  {inverse ? "Inverse: on" : "Inverse: off"}
                </Button>
              </div>
            </div>

            <form
              className="flex flex-col gap-3 sm:flex-row"
              onSubmit={(e) => {
                e.preventDefault();
                onSearch();
              }}
            >
              <Input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={inverse ? "Search target language…" : "Search English…"}
                disabled={!engine}
              />
              <Button type="submit" disabled={!engine}>
                <Search className="h-4 w-4" />
                Query
              </Button>
            </form>

            {error && (
              <p className="text-sm text-red-600" role="alert">
                {error}
              </p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Results</CardTitle>
            <CardDescription>
              Sorted by Tantivy relevance score (same as the CLI).
            </CardDescription>
          </CardHeader>
          <CardContent>
            {loading && (
              <div className="space-y-2">
                <Skeleton className="h-10 w-full" />
                <Skeleton className="h-10 w-full" />
                <Skeleton className="h-10 w-full" />
              </div>
            )}
            {!loading && hits.length === 0 && (
              <p className="text-sm text-muted-foreground">
                {engine
                  ? "No results yet. Run a query above."
                  : "Load an index to start searching."}
              </p>
            )}
            {hits.length > 0 && (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Score</TableHead>
                    <TableHead>Mod</TableHead>
                    <TableHead>Key</TableHead>
                    <TableHead>Source</TableHead>
                    <TableHead>Target</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {hits.map((hit) => (
                    <TableRow key={`${hit.mod_id}-${hit.key}-${hit.confidence}`}>
                      <TableCell>{hit.confidence.toFixed(2)}</TableCell>
                      <TableCell className="font-mono text-xs">{hit.mod_id}</TableCell>
                      <TableCell className="font-mono text-xs max-w-[12rem] truncate">
                        {hit.key}
                      </TableCell>
                      <TableCell>{hit.source}</TableCell>
                      <TableCell>{hit.target}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
