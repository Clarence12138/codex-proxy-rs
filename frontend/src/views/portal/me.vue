<script setup lang="ts">
import { shallowRef } from 'vue'
import { useRouter } from 'vue-router'

import { changePortalPassword } from '@/api/modules/portal'
import BaseButton from '@/components/base/BaseButton.vue'
import FormItem from '@/components/base/BaseForm/FormItem.vue'
import BaseInput from '@/components/base/BaseInput.vue'
import BasePageHeader from '@/components/base/BasePageHeader.vue'
import { toast } from '@/components/base/BaseToast'
import { usePortalAuthStore } from '@/stores/modules/portal-auth'
import { errorMessage } from '@/utils/async'

const auth = usePortalAuthStore()
const router = useRouter()
const currentPassword = shallowRef('')
const newPassword = shallowRef('')
const pending = shallowRef(false)
const error = shallowRef<string | null>(null)

async function submit() {
  if (pending.value)
    return
  pending.value = true
  error.value = null
  try {
    await changePortalPassword({ currentPassword: currentPassword.value, newPassword: newPassword.value })
    currentPassword.value = ''
    newPassword.value = ''
    auth.invalidateSession()
    toast.success('密码已修改，请重新登录')
    await router.replace('/login')
  }
  catch (cause: unknown) {
    error.value = errorMessage(cause, '修改密码失败，请重试')
  }
  finally {
    pending.value = false
  }
}
</script>

<template>
  <div class="grid gap-4">
    <BasePageHeader title="我的" description="修改登录密码；修改后需要重新登录，密钥和订阅不受影响" />
    <form class="grid w-full max-w-md gap-4 rounded-md bg-cp-bg-container p-4" :aria-busy="pending" @submit.prevent="submit">
      <FormItem label="当前密码" control-id="current-password">
        <BaseInput v-model="currentPassword" type="password" autocomplete="current-password" :disabled="pending" />
      </FormItem>
      <FormItem label="新密码" control-id="new-password" description="至少 6 个字符，最多 256 个 UTF-8 字节，无需组合字母、数字或符号。">
        <BaseInput v-model="newPassword" type="password" autocomplete="new-password" :disabled="pending" />
      </FormItem>
      <p v-if="error" role="alert" class="text-cp-sm text-cp-error">
        {{ error }}
      </p>
      <BaseButton type="submit" variant="primary" :loading="pending" :disabled="pending || !currentPassword || !newPassword">
        修改密码
      </BaseButton>
    </form>
  </div>
</template>
