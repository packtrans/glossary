import { ResultsTable } from "@/components/ResultsTable";
import { SearchForm } from "@/components/SearchForm";
import { StatusAlert } from "@/components/StatusAlert";
import { useGlossarySearch } from "@/hooks/useGlossarySearch";
import { DEMO_LANG } from "@/types/glossary";

export default function App() {
  const { status, error, hits, lastQuery, search } = useGlossarySearch();
  const busy = status === "loading-index" || status === "searching";

  return (
    <div className="mx-auto flex min-h-svh w-full max-w-5xl flex-col gap-6 px-4 py-10">
      <header className="space-y-2">
        <p className="text-sm font-medium text-muted-foreground">PackTrans Glossary</p>
        <h1 className="text-3xl font-semibold tracking-tight">WebAssembly demo</h1>
        <p className="max-w-2xl text-muted-foreground">
          Static React app backed by `packtrans-glossary-wasm`. This build is hardcoded to{" "}
          <code className="rounded bg-muted px-1 py-0.5 text-sm">{DEMO_LANG}</code> and runs
          entirely in the browser.
        </p>
      </header>

      <StatusAlert status={status} error={error} />
      <SearchForm disabled={busy || status === "error"} onSearch={search} />
      <ResultsTable hits={hits} query={lastQuery} />
    </div>
  );
}
