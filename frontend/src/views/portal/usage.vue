<script setup lang="ts">
import { onMounted, shallowRef } from 'vue'

import { getPortalUsageSummary, listPortalUsage } from '@/api/modules/portal'
import BaseButton from '@/components/base/BaseButton.vue'
import BasePageHeader from '@/components/base/BasePageHeader.vue'

const items = shallowRef<Array<{
  id: string
  startedAt: string
  model: string | null
  outcome: string
  totalTokens: number | null
  costUsd: string | null
  keyName: string | null
}>>([])
const summary = shallowRef<{ requestCount: number, totalTokens: number, totalUsd: string } | null>(null)
const nextCursor = shallowRef<string | null>(null)
const loadingMore = shallowRef(false)
const error = shallowRef<string | null>(null)

async function load(cursor?: string) {
  const page = await listPortalUsage({ pageSize: 50, cursor })
  items.value = cursor ? [...items.value, ...page.items] : page.items
  nextCursor.value = page.nextCursor
}

onMounted(async () => {
  try {
    const [usage] = await Promise.all([
      listPortalUsage({ pageSize: 50 }),
      getPortalUsageSummary().then((value) => {
        summary.value = value
      }),
    ])
    items.value = usage.items
    nextCursor.value = usage.nextCursor
  }
  catch {
    error.value = '无法加载用量'
  }
})

async function loadMore() {
  if (!nextCursor.value)
    return
  loadingMore.value = true
  try {
    await load(nextCursor.value)
  }
  finally {
    loadingMore.value = false
  }
}
</script>

<template>
  <div class="grid gap-4">
    <BasePageHeader title="用量" description="只显示你自己的请求，不含上游账号信息" />
    <p v-if="error" class="text-cp-error">
      {{ error }}
    </p>
    <p v-else-if="summary" class="text-cp-text-secondary">
      成功请求 {{ summary.requestCount }} · Token {{ summary.totalTokens }} · USD {{ summary.totalUsd }}
    </p>
    <p v-else-if="!items.length" class="text-cp-text-tertiary">
      暂无请求
    </p>
    <table v-if="items.length" class="w-full text-left text-cp-sm">
      <thead>
        <tr>
          <th class="py-2">
            时间
          </th>
          <th>模型</th>
          <th>状态</th>
          <th>Token</th>
          <th>USD</th>
          <th>密钥</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="item in items" :key="item.id">
          <td class="py-2">
            {{ item.startedAt }}
          </td>
          <td>{{ item.model }}</td>
          <td>{{ item.outcome }}</td>
          <td>{{ item.totalTokens }}</td>
          <td>{{ item.costUsd }}</td>
          <td>{{ item.keyName }}</td>
        </tr>
      </tbody>
    </table>
    <BaseButton v-if="nextCursor" :disabled="loadingMore" @click="loadMore">
      {{ loadingMore ? '加载中…' : '加载更多' }}
    </BaseButton>
  </div>
</template>
