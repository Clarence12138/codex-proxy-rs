<script setup lang="ts">
import type { getApiKeys } from '@/api'

type ApiKeyRow = Awaited<ReturnType<typeof getApiKeys>>['items'][number]

defineProps<{
  apiKey: ApiKeyRow
}>()
</script>

<template>
  <div class="flex flex-col gap-0.5">
    <span class="text-cp font-bold text-cp-text">
      {{ apiKey.name }}
    </span>
    <span v-if="apiKey.label" class="text-cp-sm font-emphasis text-cp-text-tertiary">
      {{ apiKey.label }}
    </span>
    <span class="text-cp-sm font-emphasis text-cp-text-tertiary">
      {{ apiKey.ownerUserId ? `用户 ${apiKey.ownerUserId}` : '管理员' }}
    </span>
  </div>
</template>
