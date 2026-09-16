<script setup lang="ts">
import { ref } from 'vue'

import BaseButton from '@/components/base/BaseButton.vue'
import BaseCard from '@/components/base/BaseCard.vue'
import BaseCheckbox from '@/components/base/BaseCheckbox.vue'
import BaseConfirmModal from '@/components/base/BaseConfirmModal.vue'
import BasePageHeader from '@/components/base/BasePageHeader.vue'
import BaseTablePagination from '@/components/base/BaseTable/BaseTablePagination.vue'
import BaseTable from '@/components/base/BaseTable/index.vue'
import LastUsedAtCell from '@/components/LastUsedAtCell.vue'
import { usePageSelection } from '@/composables/usePageSelection'
import ApiKeyActions from '@/views/keys/components/ApiKeyActions.vue'
import ApiKeyBudgetCell from '@/views/keys/components/ApiKeyBudgetCell.vue'
import ApiKeyCreateModal from '@/views/keys/components/ApiKeyCreateModal.vue'
import ApiKeyFilters from '@/views/keys/components/ApiKeyFilters.vue'
import ApiKeyIdentityCell from '@/views/keys/components/ApiKeyIdentityCell.vue'
import ApiKeyPrefixCell from '@/views/keys/components/ApiKeyPrefixCell.vue'
import ApiKeyStatusBadge from '@/views/keys/components/ApiKeyStatusBadge.vue'
import ApiKeyUseModal from '@/views/keys/components/ApiKeyUseModal.vue'
import { useApiKeyUse } from '@/views/keys/composables/useApiKeyUse'
import { usePortalKeyMutations } from './composables/usePortalKeyMutations'
import { usePortalKeysQuery } from './composables/usePortalKeysQuery'
import { portalKeyColumns } from './constants'

const selectedIds = ref<Set<string>>(new Set())
const {
  loading,
  loadError,
  apiKeys,
  loadPortalKeys,
  searchQuery,
  sort,
  apiKeyPagination,
  handlePageChange,
  handlePageSizeChange,
  handleSortChange,
} = usePortalKeysQuery()

const {
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
} = usePortalKeyMutations({ selectedIds, reload: loadPortalKeys })

const { allSelected, indeterminate, selectedRowKeys, toggleSelection, toggleAll } = usePageSelection(
  apiKeys,
  selectedIds,
)

const {
  showUseKeyModal,
  selectedUseKey,
  openAiBaseUrl,
  importCreatedKeyToCcs,
  openUseKeyModal,
  importToCcs,
} = useApiKeyUse({
  createdKey,
  createdKeyName,
  revealPlaintextKey,
})
</script>

<template>
  <div class="flex min-h-0 w-full flex-col md:h-[calc(100dvh-3rem)] md:overflow-hidden">
    <BasePageHeader
      class="shrink-0"
      title="我的密钥"
      description="分组由订阅决定；Key 限额不能高于订阅"
    >
      <template v-if="loadError" #actions>
        <BaseButton variant="secondary" :loading="loading" @click="loadPortalKeys">
          重新加载
        </BaseButton>
      </template>
    </BasePageHeader>

    <BaseCard
      class="mt-5 flex min-h-0 flex-1 flex-col"
    >
      <template #header>
        <ApiKeyFilters
          v-model:search="searchQuery"
          :show-owner-filter="false"
          :batch-deleting="batchDeleting"
          :selected-count="selectedIds.size"
          @create="openCreate"
          @delete-selected="showDeleteModal = true"
        />
      </template>

      <template #body>
        <div class="flex h-full min-h-0 flex-col">
          <BaseTable
            class="min-h-0 flex-1"
            :columns="portalKeyColumns"
            :rows="apiKeys"
            :loading="loading"
            :selected-row-keys="selectedRowKeys"
            :sort="sort"
            :empty-text="loadError ? '无法加载密钥' : '暂无 API Key'"
            @sort-change="handleSortChange"
          >
            <template #header-selection>
              <BaseCheckbox
                :model-value="allSelected"
                :indeterminate="indeterminate"
                label="选择当前页密钥"
                @update:model-value="toggleAll"
              />
            </template>
            <template #selection="{ row }">
              <BaseCheckbox
                :model-value="selectedIds.has(row.id)"
                label="选择密钥"
                @update:model-value="toggleSelection(row.id)"
              />
            </template>
            <template #identity="{ row }">
              <ApiKeyIdentityCell :api-key="row" :show-owner="false" />
            </template>
            <template #prefix="{ row }">
              <ApiKeyPrefixCell
                :prefix="row.prefix"
                :revealing="revealingKeyIds.has(row.id)"
                @copy="copyApiKey(row)"
              />
            </template>
            <template #scope>
              <div class="grid w-full justify-items-center gap-1.5">
                <span class="inline-flex h-6 items-center rounded-lg bg-cp-fill-quaternary px-2 text-cp-xs font-bold text-cp-text-secondary">
                  由订阅决定
                </span>
              </div>
            </template>
            <template #budget="{ row }">
              <ApiKeyBudgetCell :api-key="row" />
            </template>
            <template #limits="{ row }">
              <dl class="m-0 grid grid-cols-[auto_minmax(0,1fr)] gap-x-2 gap-y-1 text-xs tabular-nums">
                <dt class="text-cp-text-tertiary">
                  并发
                </dt>
                <dd class="m-0 truncate text-cp-text" :title="String(row.maxConcurrency || '∞')">
                  {{ row.maxConcurrency || '∞' }}
                </dd>
                <dt class="text-cp-text-tertiary">
                  RPM
                </dt>
                <dd class="m-0 truncate text-cp-text" :title="String(row.requestsPerMinute || '∞')">
                  {{ row.requestsPerMinute || '∞' }}
                </dd>
              </dl>
            </template>
            <template #enabled="{ row }">
              <ApiKeyStatusBadge :api-key="row" />
            </template>
            <template #lastUsedAt="{ row }">
              <LastUsedAtCell :value="row.lastUsedAt" />
            </template>
            <template #actions="{ row }">
              <ApiKeyActions
                :api-key="row"
                :deleting="deletingKey"
                :revealing="revealingKeyIds.has(row.id)"
                :updating-status="updatingStatusKeyIds.has(row.id)"
                @edit="openEdit"
                @delete="requestDeleteKey"
                @import-ccs="importToCcs"
                @toggle="handleToggleStatus"
                @use="openUseKeyModal"
              />
            </template>
          </BaseTable>
          <BaseTablePagination
            :pagination="apiKeyPagination"
            :loading="loading"
            @page-change="handlePageChange"
            @page-size-change="handlePageSizeChange"
          />
        </div>
      </template>
    </BaseCard>

    <ApiKeyCreateModal
      v-model="showFormModal"
      v-model:created-open="showKeyModal"
      v-model:form="form"
      :show-custom-key="false"
      :show-group-picker="false"
      :editing="Boolean(editingKey)"
      :created-key="createdKey"
      :saving="savingKey"
      @copy="copyToClipboard"
      @save="requestSave"
      @import-ccs="importCreatedKeyToCcs"
    />

    <ApiKeyUseModal
      v-model="showUseKeyModal"
      :api-key="selectedUseKey"
      :api-base-url="openAiBaseUrl"
      @copy="copyToClipboard"
    />

    <BaseConfirmModal
      v-model="showDeleteModal"
      title="确认删除"
      description="删除后这些 API Key 将立即失效，此操作不可撤销"
      destructive
      confirm-text="确认删除"
      :loading="batchDeleting"
      @confirm="handleBatchDelete"
    >
      <p class="m-0">
        确定删除选中的 {{ selectedIds.size }} 个 API Key 吗？
      </p>
    </BaseConfirmModal>

    <BaseConfirmModal
      v-model="showSingleDeleteModal"
      title="删除 API Key"
      description="删除后该 API Key 将立即失效，此操作不可撤销"
      destructive
      confirm-text="确认删除"
      :loading="deletingKey"
      @confirm="handleDelete"
    >
      <p class="m-0">
        确定删除 {{ pendingDeleteKey?.name || pendingDeleteKey?.prefix || '该 API Key' }} 吗？
      </p>
    </BaseConfirmModal>
  </div>
</template>
