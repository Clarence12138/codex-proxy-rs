import type { Plugin } from 'vue'

import { setUnauthorizedHandler } from '@/api/request'
import { router } from '@/router'
import { pinia } from '@/stores'
import { useAuthStore } from '@/stores/modules/auth'
import { usePortalAuthStore } from '@/stores/modules/portal-auth'

export const authPlugin: Plugin = {
  install() {
    const authStore = useAuthStore(pinia)
    const portalAuth = usePortalAuthStore(pinia)

    setUnauthorizedHandler(async (url) => {
      const isPortalApi = Boolean(url?.includes('/api/portal'))
      if (isPortalApi)
        portalAuth.invalidateSession()
      else
        authStore.invalidateSession()
      if (router.currentRoute.value.path !== '/login')
        await router.replace({ name: 'login' })
    })
  },
}
