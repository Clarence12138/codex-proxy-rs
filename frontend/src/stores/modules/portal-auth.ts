import { defineStore } from 'pinia'
import { ref } from 'vue'

import { portalAuthStatus, portalLogin, portalLogout } from '@/api/modules/portal'
import { resetUnauthorizedHandling } from '@/api/request'
import { errorMessage } from '@/utils/async'

export const usePortalAuthStore = defineStore('portalAuth', () => {
  const isAuthenticated = ref(false)
  const sessionChecked = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function checkAuth() {
    try {
      const status = await portalAuthStatus()
      isAuthenticated.value = status.authenticated
      if (status.authenticated)
        resetUnauthorizedHandling()
      return status.authenticated
    }
    catch {
      isAuthenticated.value = false
      return false
    }
    finally {
      sessionChecked.value = true
    }
  }

  async function login(payload: { username: string, password: string }) {
    try {
      loading.value = true
      error.value = null
      await portalLogin(payload)
      isAuthenticated.value = true
      sessionChecked.value = true
      resetUnauthorizedHandling()
      return true
    }
    catch (cause: unknown) {
      error.value = errorMessage(cause, '登录失败')
      isAuthenticated.value = false
      return false
    }
    finally {
      loading.value = false
    }
  }

  async function logout() {
    try {
      await portalLogout()
    }
    catch {
      // 忽略登出错误
    }
    finally {
      isAuthenticated.value = false
      sessionChecked.value = true
    }
  }

  function invalidateSession() {
    isAuthenticated.value = false
    sessionChecked.value = true
    loading.value = false
    error.value = null
  }

  return {
    isAuthenticated,
    sessionChecked,
    loading,
    error,
    checkAuth,
    login,
    logout,
    invalidateSession,
  }
})
