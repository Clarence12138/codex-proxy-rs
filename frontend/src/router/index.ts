import { createRouter, createWebHistory } from 'vue-router'

import { useAuthStore } from '@/stores/modules/auth'
import { usePortalAuthStore } from '@/stores/modules/portal-auth'
import { routes } from './routes'

export const router = createRouter({
  history: createWebHistory('/'),
  routes,
})

// 路由守卫
router.beforeEach(async (to) => {
  const isPortal = to.path === '/portal' || to.path.startsWith('/portal/')
  if (isPortal) {
    const portalAuth = usePortalAuthStore()
    if (to.path === '/portal/login') {
      if (portalAuth.isAuthenticated)
        return '/portal'
      return
    }
    if (!portalAuth.isAuthenticated && !portalAuth.sessionChecked) {
      const isAuth = await portalAuth.checkAuth()
      if (!isAuth)
        return '/portal/login'
    }
    if (!portalAuth.isAuthenticated)
      return '/portal/login'
    return
  }

  const authStore = useAuthStore()
  if (to.path === '/login') {
    if (authStore.isAuthenticated)
      return '/'
    return
  }
  if (!authStore.isAuthenticated && !authStore.sessionChecked) {
    const isAuth = await authStore.checkAuth()
    if (!isAuth)
      return '/login'
  }
  if (!authStore.isAuthenticated)
    return '/login'
})
