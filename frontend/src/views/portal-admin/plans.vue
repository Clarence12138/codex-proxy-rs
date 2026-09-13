<script setup lang="ts">
import type { AdminPortalPlan } from '@/api/modules/portal'

import { onMounted, shallowRef } from 'vue'
import { createAdminPortalPlan, listAdminPortalPlans, updateAdminPortalPlan } from '@/api/modules/portal'
import AccountGroupCheckboxGrid from '@/components/AccountGroupCheckboxGrid.vue'
import BaseButton from '@/components/base/BaseButton.vue'
import BaseInput from '@/components/base/BaseInput.vue'
import BasePageHeader from '@/components/base/BasePageHeader.vue'
import BaseSwitch from '@/components/base/BaseSwitch.vue'
import { useAccountGroupCatalog } from '@/composables/useAccountGroupCatalog'

const plans = shallowRef<AdminPortalPlan[]>([])
const name = shallowRef('')
const daily = shallowRef('0')
const weekly = shallowRef('0')
const maxConcurrency = shallowRef('0')
const requestsPerMinute = shallowRef('0')
const maxKeys = shallowRef('0')
const groupIds = shallowRef<string[]>([])
const editingId = shallowRef<string | null>(null)
const enabled = shallowRef(true)
const pending = shallowRef(false)
const error = shallowRef<string | null>(null)
const { groups, loadGroups } = useAccountGroupCatalog({ immediate: false })

async function reload() {
  plans.value = (await listAdminPortalPlans()).items
}

onMounted(async () => {
  await loadGroups().catch(() => undefined)
  await reload().catch(() => {
    error.value = '无法加载套餐'
  })
})

function fill(plan: AdminPortalPlan) {
  editingId.value = plan.id
  name.value = plan.name
  daily.value = plan.dailyLimitUsd
  weekly.value = plan.weeklyLimitUsd
  maxConcurrency.value = String(plan.maxConcurrency)
  requestsPerMinute.value = String(plan.requestsPerMinute)
  maxKeys.value = String(plan.maxKeys)
  groupIds.value = [...plan.groupIds]
  enabled.value = plan.enabled
}

async function save() {
  error.value = null
  if (!groupIds.value.length) {
    error.value = '必须选择账号分组'
    return
  }
  const payload = {
    name: name.value.trim(),
    dailyLimitUsd: daily.value,
    weeklyLimitUsd: weekly.value,
    maxConcurrency: Number(maxConcurrency.value) || 0,
    requestsPerMinute: Number(requestsPerMinute.value) || 0,
    maxKeys: Number(maxKeys.value) || 0,
    groupIds: groupIds.value,
  }
  try {
    pending.value = true
    if (editingId.value) {
      await updateAdminPortalPlan({ id: editingId.value, ...payload, enabled: enabled.value })
    }
    else {
      await createAdminPortalPlan(payload)
    }
    editingId.value = null
    name.value = ''
    await reload()
  }
  catch {
    error.value = '保存套餐失败'
  }
  finally {
    pending.value = false
  }
}
</script>

<template>
  <div class="grid gap-4">
    <BasePageHeader title="套餐" description="日/周 USD、并发和 RPM 都是用户合计上限；0 表示该维度不额外限制" />
    <p v-if="error" class="text-cp-error">
      {{ error }}
    </p>
    <div class="grid max-w-3xl gap-2 sm:grid-cols-2">
      <BaseInput v-model="name" placeholder="套餐名称" />
      <BaseInput v-model="maxKeys" placeholder="最多 Key 数（0 不限制）" />
      <BaseInput v-model="daily" placeholder="日限额 USD" />
      <BaseInput v-model="weekly" placeholder="周限额 USD" />
      <BaseInput v-model="maxConcurrency" placeholder="用户合计并发（0 不限制）" />
      <BaseInput v-model="requestsPerMinute" placeholder="用户合计 RPM（0 不限制）" />
      <div class="sm:col-span-2">
        <AccountGroupCheckboxGrid v-model="groupIds" :groups="groups" />
      </div>
      <BaseSwitch v-if="editingId" v-model="enabled" label="启用套餐" show-label />
      <BaseButton class="sm:col-span-2" variant="primary" :loading="pending" @click="save">
        {{ editingId ? '保存套餐' : '创建套餐' }}
      </BaseButton>
    </div>
    <ul class="grid gap-2">
      <li v-for="plan in plans" :key="plan.id" class="flex items-center justify-between rounded-md bg-cp-bg-container px-3 py-2">
        <span>
          {{ plan.name }} · 日 {{ plan.dailyLimitUsd }} · 并发 {{ plan.maxConcurrency }} · RPM {{ plan.requestsPerMinute }} · Key {{ plan.maxKeys }}
        </span>
        <BaseButton size="sm" @click="fill(plan)">
          编辑
        </BaseButton>
      </li>
    </ul>
  </div>
</template>
