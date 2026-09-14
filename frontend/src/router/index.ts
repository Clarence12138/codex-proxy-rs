import { createRouter, createWebHistory } from 'vue-router'

import { useAuthStore } from '@/stores/modules/auth'
import { usePortalAuthStore } from '@/stores/modules/portal-auth'
import { routes } from './routes'

export const router = createRouter({
  history: createWebHistory('/'),
  routes,
})

// 登录页允许主动重新登录，不被另一权限域的旧会话立即带走。
router.beforeEach(async (to) => {
  if (to.path === '/login')
    return

  const isPortal = to.path === '/portal' || to.path.startsWith('/portal/')
  if (isPortal) {
    const portalAuth = usePortalAuthStore()
    if (!portalAuth.isAuthenticated && !portalAuth.sessionChecked)
      await portalAuth.checkAuth()
    if (!portalAuth.isAuthenticated)
      return '/login'
    return
  }

  const authStore = useAuthStore()
  if (!authStore.isAuthenticated && !authStore.sessionChecked)
    await authStore.checkAuth()
  if (authStore.isAuthenticated)
    return

  // 仅首页自动分流；其他管理页面不能凭用户会话放行。
  if (to.path === '/') {
    const portalAuth = usePortalAuthStore()
    if (!portalAuth.isAuthenticated && !portalAuth.sessionChecked)
      await portalAuth.checkAuth()
    if (portalAuth.isAuthenticated)
      return '/portal'
  }
  return '/login'
})
