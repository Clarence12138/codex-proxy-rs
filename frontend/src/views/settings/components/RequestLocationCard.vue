<script setup lang="ts">
import type { RequestLocation } from '@/api'
import BaseCard from '@/components/base/BaseCard.vue'
import BaseForm from '@/components/base/BaseForm/index.vue'
import BaseSwitch from '@/components/base/BaseSwitch.vue'
import RequestLocationFields from '@/components/RequestLocationFields.vue'

defineProps<{ disabled: boolean }>()
const location = defineModel<RequestLocation>({ required: true })
const enabled = defineModel<boolean>('enabled', { required: true })
</script>

<template>
  <BaseCard title="请求时区与位置" description="开启后覆盖 Codex 请求的时区与位置，代理自定义配置优先">
    <template #body>
      <div>
        <BaseSwitch v-model="enabled" label="启用" show-label :disabled="disabled" />
      </div>
      <BaseForm class="mt-4 max-w-6xl">
        <RequestLocationFields v-model="location" :disabled="disabled || !enabled" />
      </BaseForm>
    </template>
  </BaseCard>
</template>
