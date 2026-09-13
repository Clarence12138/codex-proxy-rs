<script setup lang="ts">
withDefaults(defineProps<{
  apiKey: {
    name: string
    label: string | null
    ownerUserId?: string | null
  }
  // 门户密钥都属于当前用户，关闭后避免把缺失的 owner 显示成「管理员」。
  showOwner?: boolean
}>(), {
  showOwner: true,
})
</script>

<template>
  <div class="flex flex-col gap-0.5">
    <span class="text-cp font-bold text-cp-text">
      {{ apiKey.name }}
    </span>
    <span v-if="apiKey.label" class="text-cp-sm font-emphasis text-cp-text-tertiary">
      {{ apiKey.label }}
    </span>
    <span v-if="showOwner" class="text-cp-sm font-emphasis text-cp-text-tertiary">
      {{ apiKey.ownerUserId ? `用户 ${apiKey.ownerUserId}` : '管理员' }}
    </span>
  </div>
</template>
