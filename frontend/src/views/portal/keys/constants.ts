import type { PortalKey } from '@/api/modules/portal'
import { defineTableColumns } from '@/components/base/BaseTable/columns'

export type PortalKeyRow = PortalKey & {
  createdAtDisplay: string
}

export const portalKeyColumns = defineTableColumns<PortalKeyRow>([
  { key: 'selection', kind: 'selection' },
  { key: 'identity', label: '名称', kind: 'identity', size: 'xl', sortable: 'name' },
  { key: 'prefix', label: '密钥前缀', kind: 'mono', size: '2xl' },
  { key: 'enabled', label: '状态', kind: 'status', sortable: true },
  { key: 'scope', label: '分组', kind: 'status', size: 'lg' },
  { key: 'budget', label: '已用 / 限额', kind: 'text', size: 'xl' },
  { key: 'limits', label: '并发 / RPM', kind: 'text', size: 'md' },
  { key: 'createdAtDisplay', label: '创建时间', kind: 'datetime', sortable: 'createdAt' },
  {
    key: 'lastUsedAt',
    label: '最后使用',
    kind: 'datetime',
    sortable: true,
    emptyText: '',
  },
  { key: 'actions', label: '操作', kind: 'actions', size: 'xl' },
])
