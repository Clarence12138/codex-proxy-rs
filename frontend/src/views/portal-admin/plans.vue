<script setup lang="ts">
import type { AdminPortalPlan } from '@/api/modules/portal'

import { CreditCard, FolderTree, Gauge, KeyRound, Layers, Pencil, Plus } from '@lucide/vue'
import { onMounted, shallowRef } from 'vue'
import { createAdminPortalPlan, listAdminPortalPlans, updateAdminPortalPlan } from '@/api/modules/portal'
import AccountGroupCheckboxGrid from '@/components/AccountGroupCheckboxGrid.vue'
import BaseButton from '@/components/base/BaseButton.vue'
import BaseCard from '@/components/base/BaseCard.vue'
import BaseEmpty from '@/components/base/BaseEmpty.vue'
import BaseFormItem from '@/components/base/BaseForm/FormItem.vue'
import BaseInput from '@/components/base/BaseInput.vue'
import BaseModal from '@/components/base/BaseModal/index.vue'
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
const open = shallowRef(false)
const pending = shallowRef(false)
const loading = shallowRef(true)
const error = shallowRef<string | null>(null)
const loadError = shallowRef(false)
const { groups, loading: groupsLoading, loadGroups } = useAccountGroupCatalog({ immediate: false })

async function reload() {
  loading.value = true
  loadError.value = false
  try {
    plans.value = (await listAdminPortalPlans()).items
  }
  catch {
    loadError.value = true
  }
  finally {
    loading.value = false
  }
}

onMounted(() => {
  void loadGroups()
  void reload()
})

function create() {
  editingId.value = null
  name.value = ''
  daily.value = '0'
  weekly.value = '0'
  maxConcurrency.value = '0'
  requestsPerMinute.value = '0'
  maxKeys.value = '0'
  groupIds.value = []
  enabled.value = true
  error.value = null
  open.value = true
}

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
  error.value = null
  open.value = true
}

function limitLabel(value: string | number, unit: string) {
  return Number(value) === 0 ? '不限制' : `${value} ${unit}`
}

function groupLabel(id: string) {
  return groups.value.find(group => group.id === id)?.name ?? '未能加载的分组'
}

async function save() {
  if (pending.value)
    return
  error.value = null
  const planName = name.value.trim()
  if (!planName) {
    error.value = '请输入订阅名称'
    return
  }
  if (new TextEncoder().encode(planName).length > 64) {
    error.value = '订阅名称不能超过 64 个 UTF-8 字节'
    return
  }
  if (/\p{Cc}/u.test(planName)) {
    error.value = '订阅名称不能包含控制字符'
    return
  }
  if (!groupIds.value.length) {
    error.value = '必须选择账号分组'
    return
  }
  const payload = {
    name: planName,
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
    open.value = false
    await reload()
  }
  catch {
    error.value = '保存订阅失败，请检查填写内容后重试'
  }
  finally {
    pending.value = false
  }
}
</script>

<template>
  <div class="grid gap-6">
    <BasePageHeader title="订阅" description="管理用户可用的额度、请求上限与账号分组。">
      <template #actions>
        <BaseButton variant="primary" @click="create">
          <template #icon>
            <Plus :size="16" />
          </template>
          创建订阅
        </BaseButton>
      </template>
    </BasePageHeader>

    <div v-if="loadError" role="alert" class="flex flex-wrap items-center gap-3 rounded-cp bg-cp-error-container p-4 text-cp-error-on-container">
      <span>无法加载订阅，请重试。</span>
      <BaseButton size="sm" @click="reload">
        重新加载
      </BaseButton>
    </div>
    <p v-else-if="loading" role="status" class="py-8 text-center text-cp-text-secondary">
      正在加载订阅…
    </p>
    <BaseEmpty v-else-if="!plans.length" :icon="CreditCard" title="还没有订阅" description="创建订阅并设置额度，再到用户页面为用户开通。">
      <template #action>
        <BaseButton variant="primary" @click="create">
          创建订阅
        </BaseButton>
      </template>
    </BaseEmpty>
    <template v-else>
      <div class="flex flex-wrap items-center justify-between gap-2 text-cp-sm text-cp-text-secondary">
        <span>共 {{ plans.length }} 项订阅</span>
        <span>额度与请求上限按用户合计 · 0 显示为“不限制”</span>
      </div>
      <div class="grid min-w-0 gap-4 md:grid-cols-2 2xl:grid-cols-3">
        <BaseCard v-for="plan in plans" :key="plan.id" class="min-w-0">
          <div class="flex items-start gap-3">
            <span class="inline-grid size-11 shrink-0 place-items-center rounded-cp bg-cp-primary-container text-cp-primary-on-container"><CreditCard :size="22" /></span>
            <div class="min-w-0 flex-1">
              <h2 class="m-0 text-xl font-heavy break-words text-cp-text">
                {{ plan.name }}
              </h2>
              <span class="mt-2 inline-flex rounded-cp-sm px-2 py-1 text-cp-xs font-emphasis" :class="plan.enabled ? 'bg-cp-success-container text-cp-success-on-container' : 'bg-cp-fill-tertiary text-cp-text-secondary'">{{ plan.enabled ? '已启用' : '已停用' }}</span>
            </div>
            <BaseButton size="sm" :aria-label="`编辑订阅 ${plan.name}`" @click="fill(plan)">
              <template #icon>
                <Pencil :size="14" />
              </template>编辑
            </BaseButton>
          </div>
          <dl class="my-5 grid grid-cols-2 gap-3 rounded-cp bg-cp-fill-quaternary p-4">
            <div class="min-w-0">
              <dt class="text-cp-sm text-cp-text-secondary">
                每日额度
              </dt>
              <dd class="mt-2 ml-0 text-xl font-heavy break-words text-cp-text tabular-nums">
                {{ limitLabel(plan.dailyLimitUsd, 'USD') }}
              </dd>
            </div>
            <div class="min-w-0">
              <dt class="text-cp-sm text-cp-text-secondary">
                七天额度
              </dt>
              <dd class="mt-2 ml-0 text-xl font-heavy break-words text-cp-text tabular-nums">
                {{ limitLabel(plan.weeklyLimitUsd, 'USD') }}
              </dd>
            </div>
          </dl>
          <dl class="m-0 grid gap-3 text-cp-sm">
            <div class="flex items-center justify-between gap-3">
              <dt class="flex items-center gap-2 text-cp-text-secondary">
                <Layers :size="16" />并发请求
              </dt><dd class="m-0 font-emphasis text-cp-text">
                {{ limitLabel(plan.maxConcurrency, '个') }}
              </dd>
            </div>
            <div class="flex items-center justify-between gap-3">
              <dt class="flex items-center gap-2 text-cp-text-secondary">
                <Gauge :size="16" />每分钟请求（RPM）
              </dt><dd class="m-0 font-emphasis text-cp-text">
                {{ limitLabel(plan.requestsPerMinute, '次') }}
              </dd>
            </div>
            <div class="flex items-center justify-between gap-3">
              <dt class="flex items-center gap-2 text-cp-text-secondary">
                <KeyRound :size="16" />API 密钥数量
              </dt><dd class="m-0 font-emphasis text-cp-text">
                {{ limitLabel(plan.maxKeys, '个') }}
              </dd>
            </div>
          </dl>
          <div class="mt-5 grid gap-2">
            <p class="m-0 flex items-center gap-2 text-cp-sm text-cp-text-secondary">
              <FolderTree :size="16" />可用账号分组
            </p>
            <div class="flex flex-wrap gap-2">
              <span v-for="id in plan.groupIds" :key="id" class="max-w-full rounded-cp-sm bg-cp-fill-quaternary px-2.5 py-1 text-cp-xs break-words text-cp-text-secondary">{{ groupLabel(id) }}</span>
              <span v-if="!plan.groupIds.length" class="text-cp-sm text-cp-text-secondary">未配置分组</span>
            </div>
          </div>
        </BaseCard>
      </div>
    </template>

    <BaseModal v-model="open" :title="editingId ? '编辑订阅' : '创建订阅'" description="设置用户共享的使用额度与访问范围。" size="lg" :dismissible="!pending">
      <form id="subscription-form" class="grid gap-6" @submit.prevent="save">
        <fieldset :disabled="pending" class="m-0 grid min-w-0 gap-6 border-0 p-0">
          <BaseFormItem label="订阅名称" required>
            <BaseInput v-model="name" placeholder="例如：标准订阅、团队订阅" />
          </BaseFormItem>
          <section class="grid gap-4">
            <div>
              <h3 class="m-0 font-heavy text-cp-text">
                使用额度
              </h3><p class="mt-1 mb-0 text-cp-sm text-cp-text-secondary">
                所有 API 密钥按用户合计；填写 0 表示不额外限制。
              </p>
            </div>
            <div class="grid gap-4 sm:grid-cols-2">
              <BaseFormItem label="每日额度" description="每日可用金额，单位 USD">
                <BaseInput v-model="daily" type="number" min="0" step="any" required>
                  <template #suffix>
                    USD / 日
                  </template>
                </BaseInput>
              </BaseFormItem>
              <BaseFormItem label="七天额度" description="七天窗口内的可用金额，单位 USD">
                <BaseInput v-model="weekly" type="number" min="0" step="any" required>
                  <template #suffix>
                    USD / 7 天
                  </template>
                </BaseInput>
              </BaseFormItem>
            </div>
          </section>
          <section class="grid gap-4">
            <h3 class="m-0 font-heavy text-cp-text">
              请求与密钥
            </h3>
            <div class="grid gap-4 sm:grid-cols-3">
              <BaseFormItem label="并发请求上限" description="同时进行的请求数量">
                <BaseInput v-model="maxConcurrency" type="number" min="0" step="1" required>
                  <template #suffix>
                    个
                  </template>
                </BaseInput>
              </BaseFormItem>
              <BaseFormItem label="每分钟请求上限" description="RPM，每分钟的请求次数">
                <BaseInput v-model="requestsPerMinute" type="number" min="0" step="1" required>
                  <template #suffix>
                    次
                  </template>
                </BaseInput>
              </BaseFormItem>
              <BaseFormItem label="API 密钥数量上限" description="用户可创建的密钥数量">
                <BaseInput v-model="maxKeys" type="number" min="0" step="1" required>
                  <template #suffix>
                    个
                  </template>
                </BaseInput>
              </BaseFormItem>
            </div>
            <p class="m-0 text-cp-sm text-cp-text-secondary">
              以上三项填写 0 表示不额外限制。
            </p>
          </section>
          <section class="grid gap-3" aria-label="可用账号分组">
            <div>
              <h3 class="m-0 font-heavy text-cp-text">
                可用账号分组 <span class="text-cp-error">*</span>
              </h3><p class="mt-1 mb-0 text-cp-sm text-cp-text-secondary">
                至少选择一个分组，用户只能访问所选分组内的账号。
              </p>
            </div>
            <AccountGroupCheckboxGrid v-model="groupIds" :groups="groups" :loading="groupsLoading" :disabled="pending" />
          </section>
          <BaseSwitch v-if="editingId" v-model="enabled" label="启用订阅" show-label :disabled="pending" />
        </fieldset>
        <p v-if="error" role="alert" class="m-0 text-cp-sm text-cp-error">
          {{ error }}
        </p>
      </form>
      <template #footer>
        <BaseButton :disabled="pending" @click="open = false">
          取消
        </BaseButton>
        <BaseButton variant="primary" type="submit" form="subscription-form" :loading="pending">
          {{ editingId ? '保存订阅' : '创建订阅' }}
        </BaseButton>
      </template>
    </BaseModal>
  </div>
</template>
