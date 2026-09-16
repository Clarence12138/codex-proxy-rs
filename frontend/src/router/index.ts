import { createRouter, createWebHistory } from 'vue-router'

import { useAuthStore } from '@/stores/modules/auth'
import { usePortalAuthStore } from '@/stores/modules/portal-auth'
import { routes } from './routes'

export const router = createRouter({ history: createWebHistory('/'), routes })

router.beforeEach(async (to) => {
  // 主动登录不被其他权限域已有的 Cookie 带走。
  if (to.path === '/login')
    return
  const portal = usePortalAuthStore()
  const auth = useAuthStore()
  try {
    if (to.path === '/portal' || to.path.startsWith('/portal/')) {
      if (!portal.sessionChecked)
        await portal.checkAuth()
      return portal.isAuthenticated ? undefined : '/login'
    }
    if (!auth.sessionChecked)
      await auth.checkAuth()
    if (to.path === '/key-usage')
      return auth.session?.role === 'key' ? undefined : '/login'
    if (auth.isAdmin)
      return
    // 只有根地址跨域恢复，Key 会话绝不放行管理员布局。
    if (to.path === '/') {
      if (!portal.sessionChecked)
        await portal.checkAuth()
      if (portal.isAuthenticated)
        return '/portal'
      if (auth.session?.role === 'key')
        return '/key-usage'
    }
  }
  catch {
    // 暂时无法确认身份时拒绝进入受保护页面。
  }
  return '/login'
})
