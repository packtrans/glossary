import { AlertCircle, LoaderCircle } from 'lucide-react'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import type { GlossaryStatus } from '@/types/glossary'

type StatusAlertProps = {
  status: GlossaryStatus
  error: string | null
}

export function StatusAlert({ status, error }: StatusAlertProps) {
  if (status === 'loading-index') {
    return (
      <Alert>
        <LoaderCircle className="animate-spin" />
        <AlertTitle>Loading glossary index</AlertTitle>
        <AlertDescription>
          Fetching the bundled `zh_cn` index into WebAssembly memory.
        </AlertDescription>
      </Alert>
    )
  }

  if (status === 'searching') {
    return (
      <Alert>
        <LoaderCircle className="animate-spin" />
        <AlertTitle>Searching</AlertTitle>
        <AlertDescription>Running the query in the WASM module.</AlertDescription>
      </Alert>
    )
  }

  if (error) {
    return (
      <Alert variant="destructive">
        <AlertCircle />
        <AlertTitle>Something went wrong</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    )
  }

  return null
}
