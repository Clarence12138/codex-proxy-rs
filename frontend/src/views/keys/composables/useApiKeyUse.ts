import type { Ref, ShallowRef } from 'vue'
import { computed, shallowRef, watch } from 'vue'

import { API_BASE_URL } from '@/api/constants'
import { buildCodexCcSwitchImportDeeplink } from '../utils/ccswitchImport'

export interface ApiKeyUseTarget {
  id: string
  name: string
  prefix: string
  key?: string
}

// 密钥使用与 CCSwitch 导入编排：服务根地址推导、deeplink 跳转、
// “使用密钥”弹窗的明文补全与打开。只依赖 id/name/prefix 与揭示函数。
export function useApiKeyUse<T extends ApiKeyUseTarget>(options: {
  createdKey: Readonly<Ref<string>>
  createdKeyName: Readonly<Ref<string>>
  revealPlaintextKey: (apiKey: T) => Promise<string | undefined>
}) {
  const showUseKeyModal = shallowRef(false)
  const selectedUseKey: ShallowRef<(T & { key: string }) | null> = shallowRef(null)

  const serviceRootUrl = computed(() => resolveServiceRootUrl())
  const openAiBaseUrl = computed(() => `${serviceRootUrl.value}/v1`)

  function resolveServiceRootUrl() {
    const normalizedApiBase = API_BASE_URL.trim().replace(/\/+$/, '')

    if (/^https?:\/\//i.test(normalizedApiBase)) {
      return normalizedApiBase
    }

    if (typeof window === 'undefined') {
      return normalizedApiBase
    }

    const origin = window.location.origin.replace(/\/+$/, '')
    if (!normalizedApiBase) {
      return origin
    }

    return `${origin}${normalizedApiBase.startsWith('/') ? normalizedApiBase : `/${normalizedApiBase}`}`
  }

  function importCreatedKeyToCcs() {
    if (!options.createdKey.value)
      return

    window.location.href = buildCodexCcSwitchImportDeeplink({
      apiKey: options.createdKey.value,
      baseUrl: openAiBaseUrl.value,
      providerName: options.createdKeyName.value || 'codex-proxy-rs',
    })
  }

  async function openUseKeyModal(apiKey: T) {
    const key = await options.revealPlaintextKey(apiKey)
    if (!key)
      return
    selectedUseKey.value = { ...apiKey, key }
    showUseKeyModal.value = true
  }

  async function importToCcs(apiKey: T) {
    const key = await options.revealPlaintextKey(apiKey)
    if (!key)
      return
    window.location.href = buildCodexCcSwitchImportDeeplink({
      apiKey: key,
      baseUrl: openAiBaseUrl.value,
      providerName: apiKey.name || apiKey.prefix || 'codex-proxy-rs',
    })
  }

  watch(showUseKeyModal, (open) => {
    if (!open)
      selectedUseKey.value = null
  })

  return {
    showUseKeyModal,
    selectedUseKey,
    serviceRootUrl,
    openAiBaseUrl,
    importCreatedKeyToCcs,
    openUseKeyModal,
    importToCcs,
  }
}
