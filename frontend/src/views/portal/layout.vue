<script setup lang="ts">
import { KeyRound, LayoutDashboard, LogOut, ScrollText } from '@lucide/vue'
import { useRouter } from 'vue-router'

import { usePortalAuthStore } from '@/stores/modules/portal-auth'

const router = useRouter()
const auth = usePortalAuthStore()
const nav = [
  { label: '概览', path: '/portal', icon: LayoutDashboard },
  { label: '我的密钥', path: '/portal/keys', icon: KeyRound },
  { label: '用量', path: '/portal/usage', icon: ScrollText },
]

async function logout() {
  await auth.logout()
  await router.replace('/portal/login')
}
</script>

<template>
  <div class="flex min-h-dvh bg-cp-bg-layout">
    <aside class="flex w-56 shrink-0 flex-col gap-2 p-4">
      <strong class="px-2 py-3 text-cp-lg">用户面板</strong>
      <router-link
        v-for="item in nav"
        :key="item.path"
        :to="item.path"
        class="flex items-center gap-2 rounded-md px-3 py-2 text-cp-sm text-cp-text-secondary hover:bg-cp-fill-tertiary"
        active-class="!bg-cp-control-item-bg-active !text-cp-text"
      >
        <component :is="item.icon" class="size-4" />
        {{ item.label }}
      </router-link>
      <button type="button" class="mt-auto flex items-center gap-2 px-3 py-2 text-cp-sm" @click="logout">
        <LogOut class="size-4" />
        退出
      </button>
    </aside>
    <main class="min-w-0 flex-1 p-6">
      <RouterView />
    </main>
  </div>
</template>
