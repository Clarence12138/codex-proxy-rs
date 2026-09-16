import type { PortalUsageItem, PortalUsageOverview } from '@/api/modules/portal'
import { refDebounced, useDocumentVisibility, useTimeoutPoll } from '@vueuse/core'
import dayjs from 'dayjs'
import { computed, shallowRef, watch } from 'vue'
import { getPortalUsageOverview, listPortalUsage } from '@/api/modules/portal'
import { useRequestState } from '@/composables/useRequestState'

export function usePortalOverview() {
  const period = shallowRef('today')
  const model = shallowRef('')
  const selectedModel = refDebounced(model, 300)
  const refreshInterval = shallowRef('30')
  const overview = shallowRef<PortalUsageOverview>()
  const records = shallowRef<PortalUsageItem[]>([])
  const state = useRequestState()

  async function load(clear = false) {
    const id = state.start()
    if (clear) {
      overview.value = undefined
      records.value = []
    }
    const end = dayjs()
    const days = period.value === '7d' ? 6 : period.value === '30d' ? 29 : 0
    const start = end.tz('Asia/Shanghai').subtract(days, 'day').startOf('day')
    const filter = { startTime: start.toISOString(), endTime: end.toISOString(), model: selectedModel.value.trim() || undefined }
    try {
      const options = { signal: state.signal, silent: true }
      const [data, recent] = await Promise.all([
        getPortalUsageOverview(filter, options),
        listPortalUsage({ start: filter.startTime, end: filter.endTime, model: filter.model, pageSize: 10 }, options),
      ])
      if (state.isCurrent(id)) {
        overview.value = data
        records.value = recent.items
      }
    }
    catch (cause) {
      state.fail(id, cause)
    }
    finally {
      state.finish(id)
    }
  }
  async function refresh() {
    if (!state.loading.value)
      await load()
  }
  watch([period, selectedModel], () => {
    void load(true)
  }, { immediate: true })
  const visibility = useDocumentVisibility()
  const poll = useTimeoutPoll(refresh, computed(() => Math.max(1, Number(refreshInterval.value)) * 1000))
  watch([visibility, refreshInterval], ([visible, interval]) => {
    if (visible === 'visible' && interval !== '0')
      poll.resume()
    else
      poll.pause()
  }, { immediate: true })
  return { period, model, refreshInterval, overview, records, loading: state.loading, error: state.error, refresh }
}
