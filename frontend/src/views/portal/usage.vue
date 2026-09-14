<script setup lang="ts">
import type { PortalUsageItem } from '@/api/modules/portal'
import type { BaseTableColumn } from '@/components/base/BaseTable/columns'
import { Activity, CircleDollarSign, FileText } from '@lucide/vue'
import { computed, onMounted, shallowRef } from 'vue'

import { getPortalUsageSummary, listPortalUsage } from '@/api/modules/portal'
import BaseButton from '@/components/base/BaseButton.vue'
import BaseCard from '@/components/base/BaseCard.vue'
import BaseMotionIcon from '@/components/base/BaseMotionIcon.vue'
import BasePageHeader from '@/components/base/BasePageHeader.vue'
import { defineTableColumns } from '@/components/base/BaseTable/columns'
import BaseTable from '@/components/base/BaseTable/index.vue'
import ProviderIconGroup from '@/components/ProviderIconGroup.vue'
import { formatDateTime } from '@/utils/date'
import { formatInteger } from '@/utils/number'
import UsageBillingCell from '@/views/usage/components/UsageBillingCell.vue'
import UsageClientKeyCell from '@/views/usage/components/UsageClientKeyCell.vue'
import UsageLatencyCell from '@/views/usage/components/UsageLatencyCell.vue'
import UsageModelCell from '@/views/usage/components/UsageModelCell.vue'
import UsageReasoningEffortCell from '@/views/usage/components/UsageReasoningEffortCell.vue'
import UsageTokenCell from '@/views/usage/components/UsageTokenCell.vue'
import UsageTransportBadge from '@/views/usage/components/UsageTransportBadge.vue'
import { formatUsd } from '@/views/usage/utils/format'

interface PortalUsageRow extends PortalUsageItem {
  clientApiKey: string
  requestedModel: string | null
  latencyDetails: Partial<import('@/api').UsageLatencyDetails>
  startedAtDisplay: string
}

interface PortalUsageSummary {
  requestCount: number
  totalTokens: number
  totalUsd: string
}

const usageColumns: BaseTableColumn<PortalUsageRow>[] = defineTableColumns<PortalUsageRow>([
  { key: 'clientApiKey', label: '密钥', kind: 'custom', size: 'xl' },
  { key: 'model', label: '模型', kind: 'mono', size: 'xl' },
  { key: 'reasoningEffort', label: '推理强度', kind: 'custom', size: 'lg' },
  { key: 'provider', label: '平台/类型', kind: 'custom', size: 'lg' },
  { key: 'clientTransport', label: '请求类型（接入）', kind: 'custom', size: 'xl' },
  { key: 'upstreamTransport', label: '请求类型（上游）', kind: 'custom', size: 'xl' },
  { key: 'tokenDetails', label: 'TOKEN', kind: 'numeric', size: 'xl' },
  { key: 'costUsd', label: '费用', kind: 'numeric', size: 'lg' },
  { key: 'latencyMs', label: '延迟', kind: 'custom', size: 'xl' },
  { key: 'outcome', label: '状态', kind: 'status', size: 'md' },
  { key: 'startedAtDisplay', label: '时间', kind: 'datetime' },
])

const items = shallowRef<PortalUsageItem[]>([])
const summary = shallowRef<PortalUsageSummary | null>(null)
const nextCursor = shallowRef<string | null>(null)
const loading = shallowRef(false)
const loadingMore = shallowRef(false)
const error = shallowRef<string | null>(null)

const rows = computed<PortalUsageRow[]>(() => items.value.map(item => ({
  ...item,
  clientApiKey: item.keyId,
  requestedModel: item.model,
  latencyDetails: {
    firstEventMs: item.firstEventMs ?? undefined,
    firstReasoningMs: item.firstReasoningMs ?? undefined,
    firstTextMs: item.firstTextMs ?? undefined,
    firstTokenMs: item.firstTokenLatencyMs ?? undefined,
  },
  startedAtDisplay: formatDateTime(item.startedAt),
})))

const summaryItems = computed(() => [
  {
    key: 'requests',
    label: '累计成功请求',
    value: summary.value ? formatInteger(summary.value.requestCount) : '—',
    detail: '完整交付的请求',
    icon: Activity,
    tone: 'bg-cp-blue-container text-cp-blue-on-container',
  },
  {
    key: 'tokens',
    label: '总 Token',
    value: summary.value ? formatInteger(summary.value.totalTokens) : '—',
    detail: '成功请求合计',
    icon: FileText,
    tone: 'bg-cp-green-container text-cp-green-on-container',
  },
  {
    key: 'cost',
    label: '累计费用',
    value: summary.value ? formatUsd(summary.value.totalUsd) : '—',
    detail: '已记录 USD 费用',
    icon: CircleDollarSign,
    tone: 'bg-cp-orange-container text-cp-orange-on-container',
  },
])

async function loadInitial() {
  if (loading.value || loadingMore.value)
    return

  loading.value = true
  error.value = null
  try {
    const [page, nextSummary] = await Promise.all([
      listPortalUsage({ pageSize: 50 }),
      getPortalUsageSummary(),
    ])
    items.value = page.items
    nextCursor.value = page.nextCursor
    summary.value = nextSummary
  }
  catch {
    error.value = '无法加载使用记录，请稍后重试'
  }
  finally {
    loading.value = false
  }
}

async function loadMore() {
  const cursor = nextCursor.value
  if (!cursor || loading.value || loadingMore.value)
    return

  loadingMore.value = true
  error.value = null
  try {
    const page = await listPortalUsage({ pageSize: 50, cursor })
    items.value = [...items.value, ...page.items]
    nextCursor.value = page.nextCursor
  }
  catch {
    error.value = '无法加载更多记录，请稍后重试'
  }
  finally {
    loadingMore.value = false
  }
}

function outcomeText(outcome: string) {
  const labels: Record<string, string> = {
    succeeded: '成功',
    failed: '失败',
    cancelled: '已取消',
    incomplete: '未完成',
    running: '进行中',
  }
  return labels[outcome] ?? '未知状态'
}

function outcomeClass(outcome: string) {
  if (outcome === 'succeeded')
    return 'bg-cp-success-container text-cp-success-on-container'
  if (outcome === 'failed')
    return 'bg-cp-error-container text-cp-error-on-container'
  if (outcome === 'running')
    return 'bg-cp-info-container text-cp-info-on-container'
  return 'bg-cp-warning-container text-cp-warning-on-container'
}

onMounted(loadInitial)
</script>

<template>
  <div class="w-full">
    <BasePageHeader title="使用记录">
      <template v-if="error" #actions>
        <BaseButton variant="secondary" :loading="loading" :disabled="loading || loadingMore" @click="loadInitial">
          重新加载
        </BaseButton>
      </template>
    </BasePageHeader>

    <section class="mt-5 grid grid-cols-1 gap-3 md:grid-cols-3" aria-label="用量概览">
      <BaseCard
        v-for="item in summaryItems"
        :key="item.key"
        as="article"
        padding="compact"
        class="grid min-h-23 grid-cols-[36px_minmax(0,1fr)] items-stretch gap-3"
      >
        <BaseMotionIcon class="inline-flex size-9 shrink-0 items-center justify-center rounded-cp" :class="item.tone">
          <component :is="item.icon" class="size-4.5" />
        </BaseMotionIcon>
        <div class="flex min-w-0 flex-col justify-between py-0.5">
          <span class="block text-cp-sm leading-none font-bold text-cp-text-quaternary">
            {{ item.label }}
          </span>
          <strong class="block truncate text-[22px] leading-none font-extrabold text-cp-text">
            {{ item.value }}
          </strong>
          <span class="block truncate text-cp-sm leading-none font-emphasis text-cp-text-secondary">
            {{ item.detail }}
          </span>
        </div>
      </BaseCard>
    </section>

    <BaseCard
      class="mt-5 flex min-h-112 flex-col"
      title="请求明细"
      description="查看每次请求的模型、传输、Token、费用与耗时"
    >
      <template #body>
        <p v-if="error && rows.length" class="mt-0 mb-3 text-cp-sm text-cp-error-text" role="alert">
          {{ error }}
        </p>
        <div class="flex min-h-80 min-w-0 flex-1 flex-col">
          <BaseTable
            class="min-h-0 flex-1"
            :columns="usageColumns"
            :rows="rows"
            :loading="loading"
            :empty-text="error ? '无法加载使用记录' : '暂无使用记录'"
          >
            <template #clientApiKey="{ row }">
              <UsageClientKeyCell
                :key-id="row.keyId"
                :key-name="row.keyName"
                :key-prefix="row.keyPrefix"
              />
            </template>

            <template #model="{ row }">
              <UsageModelCell :record="row" />
            </template>

            <template #reasoningEffort="{ row }">
              <UsageReasoningEffortCell :record="row" />
            </template>
            <template #provider="{ row }">
              <ProviderIconGroup :provider="row.provider || ''" :authentication-kind="row.authenticationKind" />
            </template>
            <template #clientTransport="{ row }">
              <UsageTransportBadge :transport="row.clientTransport" />
            </template>
            <template #upstreamTransport="{ row }">
              <UsageTransportBadge :transport="row.upstreamTransport" />
            </template>
            <template #latencyMs="{ row }">
              <UsageLatencyCell :record="row" />
            </template>

            <template #outcome="{ row }">
              <span
                class="inline-flex h-6 items-center rounded-cp px-2 text-cp-xs font-bold"
                :class="outcomeClass(row.outcome)"
                :title="outcomeText(row.outcome)"
              >
                {{ outcomeText(row.outcome) }}
              </span>
            </template>

            <template #tokenDetails="{ row }">
              <UsageTokenCell :record="row" />
            </template>

            <template #costUsd="{ row }">
              <UsageBillingCell :record="row" />
            </template>
          </BaseTable>

          <div v-if="nextCursor" class="flex shrink-0 justify-center pt-4">
            <BaseButton variant="secondary" :loading="loadingMore" :disabled="loading || loadingMore" @click="loadMore">
              加载更多
            </BaseButton>
          </div>
        </div>
      </template>
    </BaseCard>
  </div>
</template>
