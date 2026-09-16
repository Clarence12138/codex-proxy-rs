import type { RequestOptions } from '@/api/request'
import { defineStore } from 'pinia'
import { ref } from 'vue'

import { portalAuthStatus, portalLogin, portalLogout } from '@/api/modules/portal'
import { resetUnauthorizedHandling } from '@/api/request'

export const usePortalAuthStore = defineStore('portalAuth', () => {
  const isAuthenticated = ref(false)
  const sessionChecked = ref(false)
  const loading = ref(false)
  let revision = 0
  let pendingCheck: Promise<boolean> | undefined

  function checkAuth(): Promise<boolean> {
    if (pendingCheck)
      return pendingCheck
    const current = revision
    const check = portalAuthStatus({ silent: true }).then((status) => {
      if (current === revision) {
        isAuthenticated.value = status.authenticated
        sessionChecked.value = true
        if (status.authenticated)
          resetUnauthorizedHandling()
      }
      return isAuthenticated.value
    }).finally(() => {
      if (pendingCheck === check)
        pendingCheck = undefined
    })
    pendingCheck = check
    return check
  }

  async function login(payload: Parameters<typeof portalLogin>[0], options: RequestOptions = {}) {
    revision += 1
    pendingCheck = undefined
    try {
      loading.value = true
      await portalLogin(payload, options)
      revision += 1
      isAuthenticated.value = true
      sessionChecked.value = true
      resetUnauthorizedHandling()
      return { success: true } as const
    }
    catch (cause: unknown) {
      // 凭据不匹配不撤销已有服务端会话。
      return { success: false, cause } as const
    }
    finally {
      loading.value = false
    }
  }

  async function logout() {
    await portalLogout({ silent: true })
    // 只有服务端确认撤销才清理身份，避免刷新又恢复未撤销的会话。
    invalidateSession()
  }

  function invalidateSession() {
    revision += 1
    pendingCheck = undefined
    isAuthenticated.value = false
    sessionChecked.value = true
    loading.value = false
    resetUnauthorizedHandling()
  }

  return { isAuthenticated, sessionChecked, loading, checkAuth, login, logout, invalidateSession }
})
