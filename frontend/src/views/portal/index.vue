<script setup lang="ts">
import type { PortalMe } from '@/api/modules/portal'

import { onMounted, shallowRef } from 'vue'
import { getPortalMe } from '@/api/modules/portal'
import BaseCard from '@/components/base/BaseCard.vue'
import BasePageHeader from '@/components/base/BasePageHeader.vue'

const me = shallowRef<PortalMe | null>(null)
const error = shallowRef<string | null>(null)

onMounted(async () => {
  try {
    me.value = await getPortalMe()
  }
  catch {
    error.value = '无法加载订阅信息'
  }
})
</script>

<template>
  <div class="grid gap-4">
    <BasePageHeader title="用量概览" description="查看当前订阅与日/周已用金额" />
    <p v-if="error" class="text-cp-error">
      {{ error }}
    </p>
    <div v-else-if="!me" class="text-cp-text-tertiary">
      加载中…
    </div>
    <div v-else class="grid gap-4 sm:grid-cols-2">
      <BaseCard>
        <h2 class="mb-2 font-medium">
          订阅
        </h2>
        <p>{{ me.planName ?? '未开通' }}</p>
        <p class="text-cp-text-tertiary">
          {{ me.subscriptionEffective ? '生效中' : '未生效' }}
        </p>
      </BaseCard>
      <BaseCard>
        <h2 class="mb-2 font-medium">
          日限额
        </h2>
        <p>{{ me.dailyUsedUsd }} / {{ me.dailyLimitUsd }} USD</p>
      </BaseCard>
      <BaseCard>
        <h2 class="mb-2 font-medium">
          周限额
        </h2>
        <p>{{ me.weeklyUsedUsd }} / {{ me.weeklyLimitUsd }} USD</p>
      </BaseCard>
      <BaseCard>
        <h2 class="mb-2 font-medium">
          并发 / RPM
        </h2>
        <p>{{ me.maxConcurrency }} / {{ me.requestsPerMinute }}</p>
      </BaseCard>
    </div>
  </div>
</template>
