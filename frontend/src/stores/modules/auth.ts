import type { RequestOptions } from '@/api/request'
import { defineStore } from 'pinia'
import { ref } from 'vue'

import { login as apiLogin, logout as apiLogout, getAuthStatus } from '@/api'
import { resetUnauthorizedHandling } from '@/api/request'

export const useAuthStore = defineStore('auth', () => {
  const isAuthenticated = ref(false)
  const sessionChecked = ref(false)
  const loading = ref(false)

  async function checkAuth() {
    try {
      const status = await getAuthStatus({ silent: true })
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

  async function login(payload: Parameters<typeof apiLogin>[0], options: RequestOptions = {}) {
    try {
      loading.value = true
      await apiLogin(payload, options)

      isAuthenticated.value = true
      sessionChecked.value = true
      resetUnauthorizedHandling()

      return { success: true } as const
    }
    catch (cause: unknown) {
      // 登录失败不会撤销服务端已有 Cookie，不在这里使旧会话失效。
      return { success: false, cause } as const
    }
    finally {
      loading.value = false
    }
  }

  async function logout() {
    try {
      await apiLogout({ silent: true })
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
  }

  return {
    isAuthenticated,
    sessionChecked,
    loading,
    checkAuth,
    login,
    logout,
    invalidateSession,
  }
})
