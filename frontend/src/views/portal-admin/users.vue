<script setup lang="ts">
import type { AdminPortalUser } from '@/api/modules/portal'

import { onMounted, shallowRef } from 'vue'
import {
  assignAdminPortalSubscription,
  createAdminPortalUser,
  disableAdminPortalSubscription,
  listAdminPortalPlans,
  resetAdminPortalPassword,
  setAdminPortalUserEnabled,
} from '@/api/modules/portal'
import BaseButton from '@/components/base/BaseButton.vue'
import BaseConfirmModal from '@/components/base/BaseConfirmModal.vue'
import BaseInput from '@/components/base/BaseInput.vue'
import BasePageHeader from '@/components/base/BasePageHeader.vue'
import BaseTablePagination from '@/components/base/BaseTable/BaseTablePagination.vue'
import { usePortalUsersQuery } from './usePortalUsersQuery'

const { users, search, pagination, loading, error: queryError, reload: reloadUsers, changePage, changePageSize } = usePortalUsersQuery()
const plans = shallowRef<Array<{ id: string, name: string }>>([])
const username = shallowRef('')
const password = shallowRef('')
const resetUserId = shallowRef<string | null>(null)
const resetPassword = shallowRef('')
const pendingDisableId = shallowRef<string | null>(null)
const error = shallowRef<string | null>(null)
const endsAt = shallowRef('')
const pending = shallowRef(false)

async function loadPlans() {
  plans.value = (await listAdminPortalPlans()).items
}

async function reload() {
  error.value = null
  await Promise.all([reloadUsers(), loadPlans().catch(() => {
    error.value = '无法加载订阅，请重试'
  })])
}

onMounted(() => {
  void loadPlans().catch(() => {
    error.value = '无法加载订阅，请重试'
  })
})

async function create() {
  error.value = null
  pending.value = true
  try {
    await createAdminPortalUser({ username: username.value.trim(), password: password.value })
    username.value = ''
    password.value = ''
    await reloadUsers(1)
  }
  catch {
    error.value = '创建用户失败'
  }
  finally {
    pending.value = false
  }
}

async function assign(userId: string, planId: string) {
  if (!planId)
    return
  error.value = null
  pending.value = true
  try {
    await assignAdminPortalSubscription({
      userId,
      planId,
      startsAt: new Date().toISOString(),
      endsAt: endsAt.value.trim() || null,
    })
    endsAt.value = ''
    await reload()
  }
  catch {
    error.value = '开通订阅失败'
  }
  finally {
    pending.value = false
  }
}

async function toggle(user: AdminPortalUser) {
  if (user.status === 'active') {
    pendingDisableId.value = user.id
    return
  }
  error.value = null
  try {
    await setAdminPortalUserEnabled(user.id, true)
    await reload()
  }
  catch {
    error.value = '启用用户失败'
  }
}

async function confirmDisable() {
  if (!pendingDisableId.value)
    return
  error.value = null
  try {
    await setAdminPortalUserEnabled(pendingDisableId.value, false)
    pendingDisableId.value = null
    await reload()
  }
  catch {
    error.value = '停用用户失败'
  }
}

async function reset() {
  if (!resetUserId.value)
    return
  error.value = null
  try {
    await resetAdminPortalPassword({ userId: resetUserId.value, password: resetPassword.value })
    resetUserId.value = null
    resetPassword.value = ''
    await reloadUsers()
  }
  catch {
    error.value = '重置密码失败'
  }
}

async function expire(userId: string) {
  error.value = null
  try {
    await disableAdminPortalSubscription(userId)
    await reload()
  }
  catch {
    error.value = '停用订阅失败'
  }
}

function subscriptionLabel(user: AdminPortalUser) {
  if (!user.planName)
    return '未开通'
  const end = user.subscriptionEndsAt ? ` · 到期 ${user.subscriptionEndsAt}` : ' · 无到期'
  return `${user.planName}${end}`
}
</script>

<template>
  <div class="grid gap-4">
    <BasePageHeader title="用户" description="创建拼车用户、开通或续期订阅，并重置密码" />
    <div v-if="error || queryError" role="alert" class="flex items-center gap-2 text-cp-error">
      <span>{{ error || queryError }}</span>
      <BaseButton :loading="loading" @click="reload">
        重试
      </BaseButton>
    </div>
    <BaseInput v-model="search" aria-label="搜索用户" placeholder="搜索用户名" class="max-w-sm" />
    <p v-if="loading" role="status" class="text-cp-text-tertiary">
      加载中…
    </p>
    <p v-else-if="!queryError && !users.length" role="status" class="text-cp-text-tertiary">
      {{ search.trim() ? '没有匹配的用户' : '暂无用户' }}
    </p>
    <div class="grid max-w-3xl gap-2 sm:grid-cols-4">
      <BaseInput v-model="username" placeholder="用户名" />
      <BaseInput v-model="password" type="password" placeholder="密码（至少 12 位）" />
      <BaseInput v-model="endsAt" placeholder="到期时间 RFC3339（可选）" />
      <BaseButton variant="primary" :loading="pending" @click="create">
        创建
      </BaseButton>
    </div>
    <ul class="grid gap-2">
      <li v-for="user in users" :key="user.id" class="flex flex-wrap items-center justify-between gap-2 rounded-md bg-cp-bg-container px-3 py-2">
        <span>{{ user.username }} · {{ user.status }} · {{ subscriptionLabel(user) }}</span>
        <span class="flex flex-wrap gap-2">
          <select class="rounded-md px-2 py-1" aria-label="开通或续期订阅" @change="assign(user.id, ($event.target as HTMLSelectElement).value)">
            <option value="">
              开通 / 续期
            </option>
            <option v-for="plan in plans" :key="plan.id" :value="plan.id">
              {{ plan.name }}
            </option>
          </select>
          <BaseButton size="sm" @click="expire(user.id)">
            停用订阅
          </BaseButton>
          <BaseButton size="sm" @click="toggle(user)">
            {{ user.status === 'active' ? '停用用户' : '启用用户' }}
          </BaseButton>
          <BaseButton size="sm" @click="resetUserId = user.id">
            改密
          </BaseButton>
        </span>
      </li>
    </ul>
    <BaseTablePagination
      :pagination="pagination"
      :loading="loading"
      @page-change="changePage"
      @page-size-change="changePageSize"
    />
    <BaseConfirmModal
      :model-value="Boolean(pendingDisableId)"
      title="停用用户"
      description="停用后立即拒绝登录与 /v1 准入，不会删除用量。"
      destructive
      @confirm="confirmDisable"
      @update:model-value="pendingDisableId = $event ? pendingDisableId : null"
    />
    <BaseConfirmModal
      :model-value="Boolean(resetUserId)"
      title="重置密码"
      description="重置后现有会话立即失效。"
      confirm-text="重置"
      @confirm="reset"
      @update:model-value="resetUserId = $event ? resetUserId : null; resetPassword = ''"
    >
      <BaseInput v-model="resetPassword" type="password" placeholder="新密码（至少 12 位）" />
    </BaseConfirmModal>
  </div>
</template>
