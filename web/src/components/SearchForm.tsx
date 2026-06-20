import { useState } from "react";
import { Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";

type SearchFormProps = {
  disabled?: boolean;
  onSearch: (query: string, limit: number) => void | Promise<void>;
};

const MIN_LIMIT = 1;
const MAX_LIMIT = 50;
const DEFAULT_LIMIT = 10;

function clampLimit(value: number): number {
  return Math.min(MAX_LIMIT, Math.max(MIN_LIMIT, value));
}

export function SearchForm({ disabled = false, onSearch }: SearchFormProps) {
  const [query, setQuery] = useState("Cooking Pot");
  const [limit, setLimit] = useState("10");

  return (
    <Card>
      <CardHeader>
        <CardTitle>Search glossary</CardTitle>
        <CardDescription>
          English source text to Simplified Chinese (`zh_cn`) translations.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form
          className="flex flex-col gap-4 sm:flex-row sm:items-end"
          onSubmit={(event) => {
            event.preventDefault();
            const parsedLimit = Number.parseInt(limit, 10);
            const boundedLimit = Number.isFinite(parsedLimit)
              ? clampLimit(parsedLimit)
              : DEFAULT_LIMIT;
            void onSearch(query, boundedLimit);
          }}
        >
          <div className="grid flex-1 gap-2">
            <label className="text-sm font-medium" htmlFor="query">
              Query
            </label>
            <Input
              id="query"
              placeholder='Try "Cooking Pot"'
              value={query}
              disabled={disabled}
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
          <div className="grid w-full gap-2 sm:w-28">
            <label className="text-sm font-medium" htmlFor="limit">
              Limit
            </label>
            <Input
              id="limit"
              type="number"
              inputMode="numeric"
              min={MIN_LIMIT}
              max={MAX_LIMIT}
              value={limit}
              disabled={disabled}
              onChange={(event) => setLimit(event.target.value)}
            />
          </div>
          <Button type="submit" disabled={disabled} className="sm:w-auto">
            <Search />
            Search
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}
