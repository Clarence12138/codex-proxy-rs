import type { AdminPortalUser } from '@/api/modules/portal'
import { watchDebounced } from '@vueuse/core'
import { computed, onMounted, shallowRef, watch } from 'vue'
import { listAdminPortalUsers } from '@/api/modules/portal'
import { useRequestState } from '@/composables/useRequestState'

export function usePortalUsersQuery() {
  const users = shallowRef<AdminPortalUser[]>([])
  const search = shallowRef('')
  const page = shallowRef(1)
  const pageSize = shallowRef(20)
  const total = shallowRef(0)
  const request = useRequestState()
  const pagination = computed(() => ({ currentPage: page.value, pageSize: pageSize.value, total: total.value }))

  async function reload(targetPage = page.value) {
    const id = request.start()
    const params = { page: Math.max(1, targetPage), pageSize: pageSize.value, search: search.value.trim() || undefined }
    try {
      const result = await listAdminPortalUsers(params, { signal: request.signal })
      if (!request.isCurrent(id))
        return false
      const lastPage = Math.max(1, Math.ceil(result.total / params.pageSize))
      if (params.page > lastPage)
        return reload(lastPage)
      users.value = result.items
      total.value = result.total
      page.value = params.page
      return true
    }
    catch (cause) {
      request.fail(id, cause)
      return false
    }
    finally {
      request.finish(id)
    }
  }

  function changePage(next: number) {
    void reload(next)
  }

  function changePageSize(next: number) {
    pageSize.value = next
    page.value = 1
    void reload(1)
  }

  // 输入变化立即撤销旧查询，不能等防抖结束才阻止旧搜索结果回写。
  watch(search, () => {
    request.invalidate()
    page.value = 1
    users.value = []
    total.value = 0
  }, { flush: 'sync' })
  watchDebounced(search, () => void reload(1), { debounce: 250 })
  onMounted(() => void reload())

  return { users, search, pagination, loading: request.loading, error: request.error, reload, changePage, changePageSize }
}
