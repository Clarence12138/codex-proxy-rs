<script setup lang="ts">
import { KeyRound, LayoutDashboard, LogOut, ScrollText, UserRound } from '@lucide/vue'
import { shallowRef } from 'vue'
import { useRouter } from 'vue-router'
import { toast } from '@/components/base/BaseToast'
import { usePortalAuthStore } from '@/stores/modules/portal-auth'
import { useThemeStore } from '@/stores/modules/theme'

import { errorMessage } from '@/utils/async'

const router = useRouter()
const auth = usePortalAuthStore()
const theme = useThemeStore()
const loggingOut = shallowRef(false)
const nav = [
  { label: '概览', path: '/portal', icon: LayoutDashboard },
  { label: '我的密钥', path: '/portal/keys', icon: KeyRound },
  { label: '使用记录', path: '/portal/usage', icon: ScrollText },
  { label: '我的', path: '/portal/me', icon: UserRound },
]

async function logout() {
  if (loggingOut.value)
    return
  loggingOut.value = true
  try {
    await auth.logout()
    await router.replace('/login')
  }
  catch (cause) {
    toast.error(errorMessage(cause, '退出失败，请重试'))
  }
  finally {
    loggingOut.value = false
  }
}
</script>

<template>
  <div class="flex min-h-dvh flex-col bg-cp-bg-layout md:flex-row">
    <aside aria-label="用户导航" class="flex shrink-0 flex-wrap items-center gap-2 p-4 md:w-56 md:flex-col md:items-stretch">
      <strong class="w-full px-2 py-3 text-cp-lg">用户面板</strong>
      <router-link
        v-for="item in nav"
        :key="item.path"
        :to="item.path"
        class="flex items-center gap-2 rounded-md px-3 py-2 text-cp-sm text-cp-text-secondary hover:bg-cp-fill-tertiary"
        exact-active-class="!bg-cp-control-item-bg-active !text-cp-text"
      >
        <component :is="item.icon" class="size-4" />
        {{ item.label }}
      </router-link>
      <button type="button" class="px-3 py-2 text-left text-cp-sm md:mt-auto" @click="theme.toggleTheme($event)">
        切换{{ theme.effectiveTheme === 'dark' ? '浅色' : '深色' }}主题
      </button>
      <button type="button" :disabled="loggingOut" class="flex items-center gap-2 px-3 py-2 text-cp-sm" @click="logout">
        <LogOut class="size-4" />
        退出
      </button>
    </aside>
    <main class="min-w-0 flex-1 p-4 md:p-6">
      <RouterView />
    </main>
  </div>
</template>
