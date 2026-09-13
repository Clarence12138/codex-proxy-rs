<script setup lang="ts">
import type { PortalKey } from '@/api/modules/portal'

import { onMounted, shallowRef } from 'vue'
import {
  createPortalKey,
  deletePortalKey,
  listPortalKeys,
  revealPortalKey,
  setPortalKeyEnabled,
  updatePortalKey,
} from '@/api/modules/portal'
import BaseButton from '@/components/base/BaseButton.vue'
import BaseConfirmModal from '@/components/base/BaseConfirmModal.vue'
import BaseInput from '@/components/base/BaseInput.vue'
import BasePageHeader from '@/components/base/BasePageHeader.vue'
import { useDownload } from '@/composables/useDownload'
import { buildCodexConfigFiles } from '@/views/api-keys/utils/codexConfig'

const keys = shallowRef<PortalKey[]>([])
const name = shallowRef('默认密钥')
const dailyLimitUsd = shallowRef('0')
const weeklyLimitUsd = shallowRef('0')
const maxConcurrency = shallowRef('0')
const requestsPerMinute = shallowRef('0')
const editing = shallowRef<PortalKey | null>(null)
const created = shallowRef<string | null>(null)
const pendingDeleteId = shallowRef<string | null>(null)
const error = shallowRef<string | null>(null)
const { downloadJson } = useDownload()

async function reload() {
  keys.value = await listPortalKeys()
}

onMounted(() => {
  void reload().catch(() => {
    error.value = '无法加载密钥'
  })
})

function payload() {
  return {
    name: name.value.trim() || '默认密钥',
    maxConcurrency: Number(maxConcurrency.value) || 0,
    requestsPerMinute: Number(requestsPerMinute.value) || 0,
    dailyLimitUsd: dailyLimitUsd.value,
    weeklyLimitUsd: weeklyLimitUsd.value,
  }
}

async function create() {
  error.value = null
  try {
    const key = await createPortalKey(payload())
    created.value = key.plaintext ?? null
    editing.value = null
    await reload()
  }
  catch {
    error.value = '创建失败，请确认已开通套餐且限额不超过套餐'
  }
}

async function save() {
  if (!editing.value)
    return
  error.value = null
  try {
    await updatePortalKey({ id: editing.value.id, ...payload() })
    editing.value = null
    await reload()
  }
  catch {
    error.value = '更新失败'
  }
}

function edit(key: PortalKey) {
  editing.value = key
  name.value = key.name
  dailyLimitUsd.value = key.dailyLimitUsd
  weeklyLimitUsd.value = key.weeklyLimitUsd
  maxConcurrency.value = String(key.maxConcurrency)
  requestsPerMinute.value = String(key.requestsPerMinute)
}

async function reveal(id: string) {
  const result = await revealPortalKey(id)
  created.value = result.plaintext
}

async function toggle(key: PortalKey) {
  await setPortalKeyEnabled(key.id, !key.enabled)
  await reload()
}

async function confirmDelete() {
  if (!pendingDeleteId.value)
    return
  await deletePortalKey(pendingDeleteId.value)
  pendingDeleteId.value = null
  await reload()
}

function exportClientConfig() {
  if (!created.value)
    return
  const files = buildCodexConfigFiles({
    apiKey: created.value,
    baseUrl: window.location.origin,
  })
  void downloadJson({
    'auth.json': files.auth,
    'config.toml': files.configToml,
  }, 'portal-codex-config.json')
}

function exportKeys() {
  void downloadJson(
    keys.value.map(key => ({
      id: key.id,
      name: key.name,
      prefix: key.prefix,
      enabled: key.enabled,
      maxConcurrency: key.maxConcurrency,
      requestsPerMinute: key.requestsPerMinute,
      dailyLimitUsd: key.dailyLimitUsd,
      weeklyLimitUsd: key.weeklyLimitUsd,
    })),
    'portal-keys.json',
  )
}
</script>

<template>
  <div class="grid gap-4">
    <BasePageHeader title="我的密钥" description="分组由套餐决定，不能选择全部账号；Key 限额不能高于套餐" />
    <p v-if="error" class="text-cp-error">
      {{ error }}
    </p>
    <div class="grid max-w-3xl gap-2 sm:grid-cols-2">
      <BaseInput v-model="name" placeholder="密钥名称" />
      <BaseInput v-model="maxConcurrency" placeholder="并发（0 不额外限制）" />
      <BaseInput v-model="dailyLimitUsd" placeholder="日限额 USD" />
      <BaseInput v-model="weeklyLimitUsd" placeholder="周限额 USD" />
      <BaseInput v-model="requestsPerMinute" placeholder="RPM（0 不额外限制）" />
      <div class="flex gap-2">
        <BaseButton @click="editing ? save() : create()">
          {{ editing ? '保存' : '创建' }}
        </BaseButton>
        <BaseButton variant="secondary" @click="exportKeys">
          导出清单
        </BaseButton>
      </div>
    </div>
    <p v-if="created" class="break-all font-mono text-cp-sm">
      明文（只显示一次）：{{ created }}
    </p>
    <BaseButton v-if="created" size="sm" @click="exportClientConfig">
      下载客户端配置
    </BaseButton>
    <ul class="grid gap-2">
      <li v-for="key in keys" :key="key.id" class="flex flex-wrap items-center justify-between gap-2 rounded-md bg-cp-bg-container px-3 py-2">
        <span>{{ key.name }} · {{ key.prefix }}… · 日 {{ key.dailyUsedUsd }}/{{ key.dailyLimitUsd }} · {{ key.enabled ? '启用' : '停用' }}</span>
        <span class="flex gap-2">
          <BaseButton size="sm" @click="edit(key)">编辑</BaseButton>
          <BaseButton size="sm" @click="reveal(key.id)">揭示</BaseButton>
          <BaseButton size="sm" @click="toggle(key)">{{ key.enabled ? '停用' : '启用' }}</BaseButton>
          <BaseButton size="sm" @click="pendingDeleteId = key.id">删除</BaseButton>
        </span>
      </li>
    </ul>
    <BaseConfirmModal
      :model-value="Boolean(pendingDeleteId)"
      title="删除密钥"
      description="删除后无法恢复明文。已产生的用户用量不会清零。"
      destructive
      @confirm="confirmDelete"
      @update:model-value="pendingDeleteId = $event ? pendingDeleteId : null"
    />
  </div>
</template>
