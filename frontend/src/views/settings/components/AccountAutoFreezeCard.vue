<script setup lang="ts">
import { Activity, Gauge, Snowflake, Timer } from '@lucide/vue'

import BaseCard from '@/components/base/BaseCard.vue'
import BaseCheckbox from '@/components/base/BaseCheckbox.vue'
import BaseFormItem from '@/components/base/BaseForm/FormItem.vue'
import BaseForm from '@/components/base/BaseForm/index.vue'
import BaseInput from '@/components/base/BaseInput.vue'
import BaseSwitch from '@/components/base/BaseSwitch.vue'

const enabled = defineModel<boolean>('enabled', { required: true })
const threshold = defineModel<string>('threshold', { required: true })
const windowSeconds = defineModel<string>('windowSeconds', { required: true })
const durationSeconds = defineModel<string>('durationSeconds', { required: true })
const probeEnabled = defineModel<boolean>('probeEnabled', { required: true })
const probeModel = defineModel<string>('probeModel', { required: true })
const adaptiveConcurrency = defineModel<boolean>('adaptiveConcurrency', { required: true })
</script>

<template>
  <BaseCard
    title="账号自动冻结"
    description="容量类错误高频出现时暂停账号调度，在账号管理中显示为限流中"
  >
    <BaseForm class="max-w-6xl sm:grid-cols-2">
      <BaseSwitch
        v-model="enabled"
        class="col-span-full justify-self-start"
        label="启用自动冻结"
        show-label
      />

      <BaseFormItem
        label="触发阈值"
        description="窗口内累计失败尝试达到此次数后触发"
      >
        <BaseInput
          v-model="threshold"
          :disabled="!enabled"
          aria-label="触发阈值"
          type="number"
          min="2"
          max="1000"
          step="1"
        >
          <template #prefix>
            <Activity class="size-4" />
          </template>
          <template #suffix>
            <span class="text-cp-sm">次</span>
          </template>
        </BaseInput>
      </BaseFormItem>

      <BaseFormItem
        label="统计窗口"
        description="每次失败后，计数有效期向后顺延"
      >
        <BaseInput
          v-model="windowSeconds"
          :disabled="!enabled"
          aria-label="统计窗口秒数"
          type="number"
          min="60"
          max="3600"
          step="1"
        >
          <template #prefix>
            <Timer class="size-4" />
          </template>
          <template #suffix>
            <span class="text-cp-sm">秒</span>
          </template>
        </BaseInput>
      </BaseFormItem>

      <BaseFormItem
        label="冷却时长"
        description="探测失败时，按此时长延后恢复"
      >
        <BaseInput
          v-model="durationSeconds"
          :disabled="!enabled"
          aria-label="冷却时长秒数"
          type="number"
          min="300"
          max="604800"
          step="1"
        >
          <template #prefix>
            <Snowflake class="size-4" />
          </template>
          <template #suffix>
            <span class="text-cp-sm">秒</span>
          </template>
        </BaseInput>
      </BaseFormItem>
      <BaseFormItem
        label="恢复探测模型"
        description="启用后需探测成功才恢复，模型留空时选择首个可用模型"
      >
        <template #extra>
          <BaseCheckbox
            v-model="probeEnabled"
            :disabled="!enabled"
            label="恢复前探测"
            show-label
          />
        </template>
        <BaseInput
          v-model="probeModel"
          :disabled="!enabled || !probeEnabled"
          aria-label="恢复探测模型"
          placeholder="留空自动选择"
        >
          <template #prefix>
            <Gauge class="size-4" />
          </template>
        </BaseInput>
      </BaseFormItem>

      <div class="col-span-full flex flex-wrap items-center gap-x-3 gap-y-2">
        <BaseCheckbox
          v-model="adaptiveConcurrency"
          :disabled="!enabled"
          label="自适应并发下调"
          show-label
        />
        <p class="m-0 text-cp-sm leading-5 text-cp-text-secondary">
          修改账号并发上限，恢复后不自动调高
        </p>
      </div>
    </BaseForm>
  </BaseCard>
</template>
