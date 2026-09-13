<script setup lang="ts">
import type { AdminPortalPlan, AdminPortalUser } from '@/api/modules/portal'

import { computed, onMounted, shallowRef, watch } from 'vue'
import { assignAdminPortalSubscription, listAdminPortalPlans } from '@/api/modules/portal'
import BaseButton from '@/components/base/BaseButton.vue'
import BaseCheckbox from '@/components/base/BaseCheckbox.vue'
import BaseFormItem from '@/components/base/BaseForm/FormItem.vue'
import BaseInput from '@/components/base/BaseInput.vue'
import BaseModal from '@/components/base/BaseModal/index.vue'
import BaseSelect from '@/components/base/BaseSelect.vue'
import { formatDateTime, parseTimestamp } from '@/utils/date'

const props = defineProps<{ user: AdminPortalUser, mode: 'assign' | 'renew' }>()
const emit = defineEmits<{ close: [], saved: [] }>()
const open = shallowRef(true)
const plans = shallowRef<AdminPortalPlan[]>([])
const loading = shallowRef(true)
const loadFailed = shallowRef(false)
const pending = shallowRef(false)
const planId = shallowRef(props.user.planId ?? '')
const unlimited = shallowRef(false)
const confirmedUnlimited = shallowRef(false)
const error = shallowRef<string | null>(null)
const failed = shallowRef(false)
const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone
const originalEnd = props.user.subscriptionEndsAt
const originalEndMs = originalEnd ? parseTimestamp(originalEnd) : null
const initialEnd = originalEnd && (props.mode === 'renew' || (originalEndMs ?? 0) > Date.now()) ? originalEnd : null

function localInput(value: string) {
  const date = new Date(value)
  const pad = (n: number, length = 2) => String(n).padStart(length, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}`
}

const initialInput = initialEnd ? localInput(initialEnd) : ''
const endsAt = shallowRef(initialInput)
const options = computed(() => plans.value.map(plan => ({ value: plan.id, label: plan.name, disabled: !plan.enabled })))
const selectedPlan = computed(() => plans.value.find(plan => plan.id === planId.value))
const title = computed(() => props.mode === 'renew' ? '续期订阅' : props.user.planId ? '更换套餐' : '开通订阅')
const endPreview = computed(() => unlimited.value ? '无到期（不会自动到期）' : endsAt.value ? formatDateTime(endsAt.value) : '尚未填写')

watch(open, (value) => {
  if (!value)
    emit('close')
})
watch(unlimited, () => {
  confirmedUnlimited.value = false
})

async function loadPlans() {
  loading.value = true
  loadFailed.value = false
  try {
    const items: AdminPortalPlan[] = []
    let page = 1
    while (true) {
      const result = await listAdminPortalPlans({ page, pageSize: 200 })
      items.push(...result.items)
      if (items.length >= result.total || !result.items.length)
        break
      page += 1
    }
    plans.value = items
  }
  catch {
    loadFailed.value = true
  }
  finally {
    loading.value = false
  }
}

onMounted(() => {
  void loadPlans()
})

async function save() {
  if (pending.value)
    return
  error.value = null
  failed.value = false
  if (loading.value || loadFailed.value || !selectedPlan.value?.enabled) {
    error.value = '请选择已启用的套餐'
    return
  }
  const now = new Date()
  let end: string | null = null
  if (unlimited.value) {
    if (props.mode === 'renew' || !confirmedUnlimited.value) {
      error.value = '请明确确认订阅不会自动到期'
      return
    }
  }
  else {
    const timestamp = endsAt.value ? parseTimestamp(endsAt.value) : null
    if (timestamp === null || timestamp <= now.getTime()) {
      error.value = '请填写晚于当前时间的有效到期时间'
      return
    }
    if (props.mode === 'renew' && (originalEndMs === null || timestamp <= originalEndMs)) {
      error.value = '续期后的到期时间必须晚于原到期时间'
      return
    }
    // 未编辑的原值直接传回，避免浏览器日期输入损失服务端时间精度。
    end = endsAt.value === initialInput && initialEnd ? initialEnd : new Date(timestamp).toISOString()
  }
  const start = props.mode === 'renew' ? props.user.subscriptionStartsAt : now.toISOString()
  if (!start || (end && (parseTimestamp(end) ?? 0) <= (parseTimestamp(start) ?? 0))) {
    error.value = '到期时间必须晚于订阅开始时间'
    return
  }
  pending.value = true
  try {
    await assignAdminPortalSubscription({ userId: props.user.id, planId: planId.value, startsAt: start, endsAt: end })
    emit('saved')
  }
  catch {
    // 请求层统一提示；保留草稿，避免未知提交结果被自动重放。
    failed.value = true
  }
  finally {
    pending.value = false
  }
}
</script>

<template>
  <BaseModal v-model="open" :title="title" :dismissible="!pending">
    <div class="grid gap-4">
      <div class="grid gap-1 rounded-cp bg-cp-fill-quaternary p-3 text-cp-text-secondary">
        <p>用户：{{ user.username }}</p>
        <p>当前套餐：{{ user.planName ?? '未开通' }}</p>
        <p>当前开始时间：{{ user.subscriptionStartsAt ? formatDateTime(user.subscriptionStartsAt) : '—' }}</p>
        <p>当前到期时间：{{ user.planId ? originalEnd ? formatDateTime(originalEnd) : '无到期' : '—' }}</p>
      </div>
      <p class="text-cp-sm text-cp-text-secondary">
        {{ mode === 'renew' ? '仅延长原套餐的到期时间，保留原开始时间，不重置已用额度。' : '确认后立即替换当前订阅，不自动累加时长，也不重置已用额度。' }}
      </p>
      <p v-if="loading" role="status">
        正在加载套餐…
      </p>
      <div v-if="loadFailed" role="alert">
        无法加载套餐。
        <BaseButton @click="loadPlans">
          重试
        </BaseButton>
      </div>
      <BaseFormItem label="套餐">
        <BaseSelect v-model="planId" :options="options" :disabled="pending || loading || mode === 'renew'" />
      </BaseFormItem>
      <BaseCheckbox v-if="mode === 'assign'" v-model="unlimited" label="设置为无到期" show-label :disabled="pending" />
      <BaseFormItem v-if="!unlimited" :label="`新到期时间（${timeZone}）`">
        <BaseInput v-model="endsAt" type="datetime-local" step="0.001" :disabled="pending" />
      </BaseFormItem>
      <BaseCheckbox v-else v-model="confirmedUnlimited" label="我确认此订阅不会自动到期，需要手动停用" show-label :disabled="pending" />
      <p class="break-words text-cp-text-secondary">
        提交结果：{{ selectedPlan?.name ?? '尚未选择套餐' }} · {{ endPreview }}
      </p>
      <p v-if="error" role="alert" class="text-cp-error">
        {{ error }}
      </p>
      <p v-if="failed" role="status" class="text-cp-text-secondary">
        未能确认操作成功，草稿已保留。网络异常时请先刷新用户列表核对订阅，再决定是否重试。
      </p>
    </div>
    <template #footer>
      <BaseButton :disabled="pending" @click="open = false">
        取消
      </BaseButton>
      <BaseButton variant="primary" :loading="pending" :disabled="loading || loadFailed" @click="save">
        确认{{ title }}
      </BaseButton>
    </template>
  </BaseModal>
</template>
