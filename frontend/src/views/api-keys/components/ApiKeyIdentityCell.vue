<script setup lang="ts">
withDefaults(defineProps<{
  apiKey: {
    name: string
    label: string | null
    ownerUserId?: string | null
    ownerUsername?: string | null
  }
  // 门户密钥都属于当前用户，关闭后避免把缺失的 owner 显示成「管理员」。
  showOwner?: boolean
}>(), {
  showOwner: true,
})

function ownerLabel(apiKey: {
  ownerUserId?: string | null
  ownerUsername?: string | null
}) {
  if (apiKey.ownerUsername) {
    return `用户 ${apiKey.ownerUsername}`
  }
  // 用户已删除时仍能区分拼车 Key，但不回退展示内部用户 ID。
  if (apiKey.ownerUserId) {
    return '用户'
  }
  return '管理员'
}
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
      {{ ownerLabel(apiKey) }}
    </span>
  </div>
</template>
