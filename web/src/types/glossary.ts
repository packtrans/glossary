export interface QueryHit {
  confidence: number
  mod_id: string
  key: string
  source: string
  source_lang: string
  target_lang: string
  target: string
}

export type GlossaryStatus =
  | 'loading-index'
  | 'ready'
  | 'searching'
  | 'error'

export const DEMO_LANG = 'zh_cn' as const
export const INDEX_URL = `/indexes/${DEMO_LANG}.zip`
