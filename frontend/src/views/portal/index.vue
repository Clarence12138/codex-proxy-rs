<script setup lang="ts">
import BaseButton from '@/components/base/BaseButton.vue'
import BaseCard from '@/components/base/BaseCard.vue'
import BaseInput from '@/components/base/BaseInput.vue'
import BasePageHeader from '@/components/base/BasePageHeader.vue'
import BaseSelect from '@/components/base/BaseSelect.vue'
import UsageBudget from '@/components/usage/UsageBudget.vue'
import UsageSkeleton from '@/components/usage/UsageSkeleton.vue'
import UsageSummary from '@/components/usage/UsageSummary.vue'
import UsageTrend from '@/components/usage/UsageTrend.vue'
import { formatDateTime } from '@/utils/date'
import RequestHealthTimelineCard from '@/views/dashboard/components/RequestHealthTimelineCard.vue'
import UsageBillingCell from '@/views/usage/components/UsageBillingCell.vue'
import UsageTokenCell from '@/views/usage/components/UsageTokenCell.vue'
import { usePortalOverview } from './composables/usePortalOverview'
import { outcomeClass, outcomeText } from './utils/usage'

const { period, model, refreshInterval, overview, records, loading, error, refresh } = usePortalOverview()
</script>

<template>
  <div class="grid min-w-0 gap-5">
    <BasePageHeader title="用量概览" description="当前用户全部 API Key 的汇总；日/周额度由所有 Key 共享">
      <template #actions>
        <div class="flex flex-wrap gap-2">
          <BaseSelect v-model="period" aria-label="统计时间范围" class="w-29" :options="[{ label: '今天', value: 'today' }, { label: '近 7 天', value: '7d' }, { label: '近 30 天', value: '30d' }]" />
          <BaseSelect v-model="refreshInterval" aria-label="自动刷新频率" class="w-30" :options="[{ label: '30 秒刷新', value: '30' }, { label: '60 秒刷新', value: '60' }, { label: '暂停刷新', value: '0' }]" />
          <BaseButton :loading="loading" @click="refresh">
            刷新
          </BaseButton>
        </div>
      </template>
    </BasePageHeader>
    <BaseInput v-model="model" class="w-60 max-w-full" placeholder="输入完整模型名称" aria-label="筛选用量模型" :maxlength="128" />
    <p v-if="error" role="alert" class="rounded-cp-lg bg-cp-error-container p-3 text-cp-error-text">
      {{ error }}{{ overview ? '，暂时保留上次结果。' : '，请刷新重试。' }}
    </p>
    <template v-if="overview">
      <BaseCard title="用户与订阅">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div>
            <strong>{{ overview.user.username }}</strong>
            <span class="ml-3">{{ overview.user.planName ?? '未开通套餐' }}</span>
            <span class="ml-3" :class="overview.user.subscriptionEffective ? 'text-cp-success' : 'text-cp-warning'">{{ overview.user.subscriptionEffective ? '生效中' : '未生效' }}</span>
          </div>
          <span class="text-cp-text-secondary">到期：{{ overview.user.subscriptionEndsAt ? formatDateTime(overview.user.subscriptionEndsAt) : overview.user.planName ? '无到期时间' : '—' }}</span>
        </div>
        <p class="mt-2 text-cp-xs text-cp-text-tertiary">
          更新于 {{ formatDateTime(overview.asOf) }}。订阅未生效时可查看历史，但不能调用模型。
        </p>
      </BaseCard>
      <UsageSummary :summary="overview.summary" />
      <div class="grid min-w-0 gap-5 xl:grid-cols-[minmax(0,1.4fr)_minmax(400px,1fr)]">
        <UsageTrend class="min-w-0" :points="overview.trend" />
        <UsageBudget :budget="overview.budget" />
      </div>
      <RequestHealthTimelineCard :timeline="overview.healthTimeline" />
      <BaseCard title="最近请求">
        <div class="mb-3 flex justify-end">
          <RouterLink to="/portal/usage" class="text-cp-primary-text">
            查看完整记录与详情 →
          </RouterLink>
        </div>
        <div class="overflow-x-auto">
          <table class="w-full text-left text-cp-sm">
            <thead class="text-cp-text-secondary">
              <tr>
                <th class="p-2">
                  密钥 / 时间
                </th><th class="p-2">
                  模型
                </th><th class="p-2">
                  Token
                </th><th class="p-2">
                  费用
                </th><th class="p-2">
                  状态
                </th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="record in records" :key="record.id">
                <td class="p-2">
                  <div>{{ record.keyName ?? record.keyId }} <span v-if="record.keyPrefix" class="font-mono text-cp-text-tertiary">{{ record.keyPrefix }}…</span></div><div class="text-cp-xs text-cp-text-tertiary">
                    {{ formatDateTime(record.startedAt) }}
                  </div>
                </td>
                <td class="p-2 font-mono">
                  {{ record.model ?? '—' }}
                </td>
                <td class="p-2">
                  <UsageTokenCell :record="record" />
                </td>
                <td class="p-2">
                  <UsageBillingCell :record="record" />
                </td>
                <td class="p-2">
                  <span class="rounded-cp px-2 py-1 text-cp-xs" :class="outcomeClass(record.outcome)">{{ outcomeText(record.outcome) }}</span>
                </td>
              </tr>
            </tbody>
          </table>
          <p v-if="!records.length" class="p-4 text-center text-cp-text-tertiary">
            所选范围暂无请求
          </p>
        </div>
      </BaseCard>
    </template>
    <UsageSkeleton v-else-if="loading" />
  </div>
</template>
