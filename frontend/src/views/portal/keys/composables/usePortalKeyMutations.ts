import type { Ref } from 'vue'
import type { PortalKeyRow } from '../constants'
import type { ApiKeyFormValue } from '@/views/api-keys/composables/useApiKeyMutations'
import { ref, shallowRef, watch } from 'vue'
import {
  createPortalKey,
  deletePortalKey,
  revealPortalKey,
  setPortalKeyEnabled,
  updatePortalKey,
} from '@/api/modules/portal'
import { toast } from '@/components/base/BaseToast'
import { useAsyncAction } from '@/composables/useAsyncAction'
import { useCopyText } from '@/composables/useCopyText'
import { useIdSet } from '@/composables/useIdSet'

export function usePortalKeyMutations(options: {
  selectedIds: Ref<Set<string>>
  reload: () => Promise<unknown>
}) {
  const copyText = useCopyText()
  const showFormModal = shallowRef(false)
  const showDeleteModal = shallowRef(false)
  const showSingleDeleteModal = shallowRef(false)
  const showKeyModal = shallowRef(false)
  const createdKey = shallowRef('')
  const createdKeyName = shallowRef('')
  const editingKey = shallowRef<PortalKeyRow | null>(null)
  const pendingDeleteKey = shallowRef<PortalKeyRow | null>(null)
  const savingKeyAction = useAsyncAction()
  const deletingKeyAction = useAsyncAction()
  const batchDeletingAction = useAsyncAction()
  const updatingStatusKeys = useIdSet<string>()
  const revealingKeys = useIdSet<string>()
  const savingKey = savingKeyAction.loading
  const deletingKey = deletingKeyAction.loading
  const batchDeleting = batchDeletingAction.loading
  const updatingStatusKeyIds = updatingStatusKeys.ids
  const revealingKeyIds = revealingKeys.ids
  const form = ref<ApiKeyFormValue>(emptyForm())

  function openCreate() {
    editingKey.value = null
    form.value = emptyForm()
    showFormModal.value = true
  }

  function openEdit(key: PortalKeyRow) {
    editingKey.value = key
    form.value = {
      customKey: '',
      name: key.name,
      label: key.label ?? '',
      groupIds: [],
      maxConcurrency: limitInputValue(key.maxConcurrency),
      requestsPerMinute: limitInputValue(key.requestsPerMinute),
      dailyLimitUsd: limitInputValue(key.dailyLimitUsd),
      weeklyLimitUsd: limitInputValue(key.weeklyLimitUsd),
    }
    showFormModal.value = true
  }

  function requestSave() {
    if (!validateForm() || savingKey.value)
      return
    void save()
  }

  async function save() {
    if (!validateForm() || savingKey.value)
      return

    await savingKeyAction.run(
      async () => {
        const payload = {
          name: form.value.name.trim(),
          label: form.value.label.trim() || null,
          maxConcurrency: parseLimit(form.value.maxConcurrency),
          requestsPerMinute: parseLimit(form.value.requestsPerMinute),
          dailyLimitUsd: form.value.dailyLimitUsd.trim() || '0',
          weeklyLimitUsd: form.value.weeklyLimitUsd.trim() || '0',
        }
        const current = editingKey.value
        if (current) {
          await updatePortalKey({ id: current.id, ...payload })
        }
        else {
          const result = await createPortalKey(payload)
          createdKey.value = result.plaintext ?? ''
          createdKeyName.value = payload.name
        }

        showFormModal.value = false
        editingKey.value = null
        form.value = emptyForm()
        await options.reload()
        if (current) {
          toast.success('API Key 已更新')
        }
        else {
          showKeyModal.value = true
          toast.success('API Key 创建成功')
        }
      },
      { onError: () => void options.reload() },
    )
  }

  function validateForm() {
    for (const [label, value] of [['日限额', form.value.dailyLimitUsd], ['周限额', form.value.weeklyLimitUsd]]) {
      if (value.trim() && !/^\d{1,10}(?:\.\d{1,10})?$/.test(value.trim())) {
        toast.warning(`${label}必须是非负金额，最多 10 位小数`)
        return false
      }
    }
    if (!form.value.name.trim()) {
      toast.warning('请输入 API Key 名称')
      return false
    }
    for (const [label, value] of [
      ['最大并发', form.value.maxConcurrency],
      ['每分钟请求数', form.value.requestsPerMinute],
    ] as const) {
      const parsed = Number(value)
      if (!Number.isSafeInteger(parsed) || parsed < 0) {
        toast.warning(`${label}必须是非负整数`)
        return false
      }
    }
    return true
  }

  function requestDeleteKey(key: PortalKeyRow) {
    pendingDeleteKey.value = key
    showSingleDeleteModal.value = true
  }

  async function handleDelete() {
    if (deletingKey.value)
      return
    const keyId = pendingDeleteKey.value?.id
    if (!keyId)
      return

    await deletingKeyAction.run(
      async () => {
        await deletePortalKey(keyId)
        const remaining = new Set(options.selectedIds.value)
        remaining.delete(keyId)
        options.selectedIds.value = remaining
        showSingleDeleteModal.value = false
        pendingDeleteKey.value = null
        await options.reload()
        toast.success('删除成功')
      },
      { onError: () => void options.reload() },
    )
  }

  async function handleBatchDelete() {
    if (batchDeleting.value || options.selectedIds.value.size === 0)
      return

    await batchDeletingAction.run(
      async () => {
        const ids = [...options.selectedIds.value]
        let deleted = 0
        // 部分失败保留未确认删除的选择项，统一反馈，避免每个接口重复弹错。
        for (const keyId of ids) {
          try {
            await deletePortalKey(keyId, { silent: true })
            const remaining = new Set(options.selectedIds.value)
            remaining.delete(keyId)
            options.selectedIds.value = remaining
            deleted += 1
          }
          catch {
            // 网络错误可能结果未知，不自动重放；刷新后由用户决定是否重试。
          }
        }
        showDeleteModal.value = false
        await options.reload()
        if (deleted === ids.length)
          toast.success(`已删除 ${deleted} 个 API Key`)
        else
          toast.warning(`已确认删除 ${deleted} 个，${ids.length - deleted} 个未确认删除，请刷新核对后重试`)
      },
      { onError: () => void options.reload() },
    )
  }

  async function handleToggleStatus(key: PortalKeyRow) {
    await updatingStatusKeys.run(key.id, async () => {
      try {
        await setPortalKeyEnabled(key.id, !key.enabled)
        await options.reload()
        toast.success(key.enabled ? '已禁用' : '已启用')
      }
      catch {
        void options.reload()
      }
    })
  }

  async function copyToClipboard(text: string) {
    await copyText(text, { successText: '已复制到剪贴板', emptyErrorText: '复制失败' })
  }

  async function revealPlaintextKey(apiKey: PortalKeyRow) {
    try {
      const result = await revealingKeys.run(apiKey.id, () => revealPortalKey(apiKey.id))
      if (!result)
        return undefined
      if (!result.plaintext) {
        toast.error('完整 API Key 不可用')
        return undefined
      }
      return result.plaintext
    }
    catch {
      return undefined
    }
  }

  async function copyApiKey(apiKey: PortalKeyRow) {
    const key = await revealPlaintextKey(apiKey)
    if (key)
      await copyToClipboard(key)
  }

  watch(showKeyModal, (open) => {
    if (!open) {
      createdKey.value = ''
      createdKeyName.value = ''
    }
  })
  watch(showFormModal, (open) => {
    if (!open && !savingKey.value) {
      editingKey.value = null
      form.value = emptyForm()
    }
  })

  return {
    showFormModal,
    showDeleteModal,
    showSingleDeleteModal,
    showKeyModal,
    createdKey,
    createdKeyName,
    editingKey,
    pendingDeleteKey,
    savingKey,
    deletingKey,
    batchDeleting,
    updatingStatusKeyIds,
    revealingKeyIds,
    form,
    openCreate,
    openEdit,
    requestSave,
    requestDeleteKey,
    handleDelete,
    handleBatchDelete,
    handleToggleStatus,
    copyToClipboard,
    revealPlaintextKey,
    copyApiKey,
  }
}

function emptyForm(): ApiKeyFormValue {
  // customKey / groupIds 只为共用创建弹窗的表单形状，门户请求不会提交它们。
  return {
    customKey: '',
    name: '',
    label: '',
    groupIds: [],
    maxConcurrency: '',
    requestsPerMinute: '',
    dailyLimitUsd: '',
    weeklyLimitUsd: '',
  }
}

function limitInputValue(limit: string | number) {
  return Number(limit) === 0 ? '' : String(limit)
}

function parseLimit(value: string) {
  return Number(value)
}
