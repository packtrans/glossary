import { useState } from 'react'
import { Search } from 'lucide-react'

import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'

type SearchFormProps = {
  disabled?: boolean
  onSearch: (query: string, limit: number) => void | Promise<void>
}

export function SearchForm({ disabled = false, onSearch }: SearchFormProps) {
  const [query, setQuery] = useState('Cooking Pot')
  const [limit, setLimit] = useState('10')

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
            event.preventDefault()
            const parsedLimit = Number.parseInt(limit, 10)
            void onSearch(query, Number.isFinite(parsedLimit) ? parsedLimit : 10)
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
              inputMode="numeric"
              min={1}
              max={50}
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
  )
}
