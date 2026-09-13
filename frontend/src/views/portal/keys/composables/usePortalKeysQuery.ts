import type { PortalKeyRow } from '../constants'
import type { BaseTableSort } from '@/components/base/BaseTable/columns'
import { computed, onMounted, shallowRef, watch } from 'vue'
import { listPortalKeys } from '@/api/modules/portal'
import { useRequestState } from '@/composables/useRequestState'
import { formatDateTime } from '@/utils/date'

// 门户列表接口返回当前用户全量 Key，搜索、排序和分页都在本地完成。
export function usePortalKeysQuery() {
  const searchQuery = shallowRef('')
  const sort = shallowRef<BaseTableSort>()
  const page = shallowRef(1)
  const pageSize = shallowRef(20)
  const allKeys = shallowRef<PortalKeyRow[]>([])
  const request = useRequestState()
  const { loading, error } = request

  const filteredKeys = computed(() => {
    const keyword = searchQuery.value.trim().toLowerCase()
    const items = keyword
      ? allKeys.value.filter((key) => {
          return key.name.toLowerCase().includes(keyword)
            || (key.label?.toLowerCase().includes(keyword) ?? false)
        })
      : allKeys.value

    const currentSort = sort.value
    if (!currentSort)
      return items

    return [...items].sort((left, right) => comparePortalKeys(left, right, currentSort))
  })

  const apiKeyPagination = computed(() => ({
    currentPage: page.value,
    pageSize: pageSize.value,
    total: filteredKeys.value.length,
  }))

  const apiKeys = computed(() => {
    const start = (page.value - 1) * pageSize.value
    return filteredKeys.value.slice(start, start + pageSize.value)
  })

  function clampPage(total = filteredKeys.value.length) {
    const maxPage = Math.max(1, Math.ceil(total / pageSize.value) || 1)
    if (page.value > maxPage)
      page.value = maxPage
  }

  async function loadPortalKeys() {
    const requestId = request.start()
    const signal = request.signal

    try {
      const items = await listPortalKeys({ signal })
      if (!request.isCurrent(requestId))
        return false

      allKeys.value = items.map(item => ({
        ...item,
        createdAtDisplay: formatDateTime(item.createdAt),
      }))
      clampPage()
      return true
    }
    catch (cause) {
      request.fail(requestId, cause)
      return false
    }
    finally {
      request.finish(requestId)
    }
  }

  function handlePageChange(nextPage: number) {
    page.value = Math.max(1, nextPage)
  }

  function handlePageSizeChange(nextPageSize: number) {
    pageSize.value = nextPageSize
    page.value = 1
  }

  function handleSortChange(nextSort: BaseTableSort | undefined) {
    sort.value = nextSort
    page.value = 1
  }

  watch(searchQuery, () => {
    page.value = 1
  })

  watch([filteredKeys, pageSize], () => {
    clampPage()
  })

  onMounted(() => {
    void loadPortalKeys()
  })

  return {
    loading,
    loadError: error,
    apiKeys,
    loadPortalKeys,
    searchQuery,
    sort,
    apiKeyPagination,
    handlePageChange,
    handlePageSizeChange,
    handleSortChange,
  }
}

function comparePortalKeys(left: PortalKeyRow, right: PortalKeyRow, sort: BaseTableSort) {
  const direction = sort.direction === 'asc' ? 1 : -1
  const [leftValue, rightValue] = sortValues(left, right, sort.key)
  if (leftValue < rightValue)
    return -1 * direction
  if (leftValue > rightValue)
    return 1 * direction
  return 0
}

function sortValues(left: PortalKeyRow, right: PortalKeyRow, key: string): [string | number, string | number] {
  switch (key) {
    case 'name':
      return [left.name, right.name]
    case 'enabled':
      return [Number(left.enabled), Number(right.enabled)]
    case 'createdAt':
      return [left.createdAt, right.createdAt]
    case 'lastUsedAt':
      return [left.lastUsedAt ?? '', right.lastUsedAt ?? '']
    default:
      return ['', '']
  }
}
