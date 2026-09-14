<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  keyId: string
  keyName: string | null
  keyPrefix: string | null
}>()

const name = computed(() => props.keyName?.trim() || '已删除密钥')
const metadata = computed(() => props.keyPrefix ? `${props.keyPrefix}••••••••` : props.keyId)
const title = computed(() => props.keyPrefix
  ? `${name.value} · ${props.keyPrefix}…`
  : `${name.value} · ${props.keyId}`)
</script>

<template>
  <div class="grid min-w-0 gap-1" :title="title">
    <span class="truncate text-cp-sm leading-none font-bold text-cp-text">
      {{ name }}
    </span>
    <code class="truncate font-mono text-cp-xs leading-none font-emphasis text-cp-text-tertiary">
      {{ metadata }}
    </code>
  </div>
</template>
