import type { RequestOptions } from '../request'
import request from '../request'

export function portalLogin(data: { username: string, password: string }, options: RequestOptions = {}) {
  return request<{ expiresAt: string }>({
    url: '/api/portal/auth/login',
    method: 'POST',
    data,
    ...options,
  })
}

export function portalAuthStatus(options: RequestOptions = {}) {
  return request<{ authenticated: boolean }>({
    url: '/api/portal/auth/status',
    method: 'GET',
    ...options,
  })
}

export function portalLogout(options: RequestOptions = {}) {
  return request({
    url: '/api/portal/auth/logout',
    method: 'POST',
    ...options,
  })
}

export interface PortalMe {
  userId: string
  username: string
  planName: string | null
  subscriptionEndsAt: string | null
  subscriptionEffective: boolean
  dailyLimitUsd: string
  weeklyLimitUsd: string
  dailyUsedUsd: string
  weeklyUsedUsd: string
  maxConcurrency: number
  requestsPerMinute: number
  maxKeys: number
  keyCount: number
}

export function getPortalMe() {
  return request<PortalMe>({
    url: '/api/portal/me',
    method: 'GET',
  })
}

export interface PortalKey {
  id: string
  name: string
  label: string | null
  prefix: string
  enabled: boolean
  maxConcurrency: number
  requestsPerMinute: number
  dailyLimitUsd: string
  weeklyLimitUsd: string
  dailyUsedUsd: string
  weeklyUsedUsd: string
  lastUsedAt: string | null
  createdAt: string
  plaintext?: string | null
}

export function listPortalKeys(options: RequestOptions = {}) {
  return request<PortalKey[]>({
    url: '/api/portal/keys',
    method: 'GET',
    ...options,
  })
}

export function createPortalKey(data: {
  name: string
  label?: string | null
  maxConcurrency?: number
  requestsPerMinute?: number
  dailyLimitUsd?: string
  weeklyLimitUsd?: string
}) {
  return request<PortalKey>({
    url: '/api/portal/keys',
    method: 'POST',
    data,
  })
}

export function revealPortalKey(id: string) {
  return request<{ plaintext: string }>({
    url: '/api/portal/keys/reveal',
    method: 'GET',
    params: { id },
  })
}

export function setPortalKeyEnabled(id: string, enabled: boolean) {
  return request({
    url: enabled ? '/api/portal/keys/enable' : '/api/portal/keys/disable',
    method: 'POST',
    data: { id },
  })
}

export function deletePortalKey(id: string, options: RequestOptions = {}) {
  return request({
    url: '/api/portal/keys/delete',
    method: 'POST',
    data: { id },
    ...options,
  })
}

export function updatePortalKey(data: {
  id: string
  name: string
  label?: string | null
  maxConcurrency?: number
  requestsPerMinute?: number
  dailyLimitUsd?: string
  weeklyLimitUsd?: string
}) {
  return request({
    url: '/api/portal/keys/update',
    method: 'POST',
    data,
  })
}

export interface PortalUsageItem {
  id: string
  startedAt: string
  model: string | null
  outcome: string
  inputTokens: number | null
  outputTokens: number | null
  totalTokens: number | null
  costUsd: string | null
  keyId: string
  keyPrefix: string | null
  keyName: string | null
}

export function listPortalUsage(params: { cursor?: string, pageSize?: number }) {
  return request<{ items: PortalUsageItem[], nextCursor: string | null }>({
    url: '/api/portal/usage/records',
    method: 'GET',
    params,
  })
}

export function changePortalPassword(payload: { currentPassword: string, newPassword: string }) {
  return request<Record<string, never>>({
    url: '/api/portal/auth/password',
    method: 'POST',
    data: payload,
  })
}

export function getPortalUsageSummary() {
  return request<{ requestCount: number, totalTokens: number, totalUsd: string }>({
    url: '/api/portal/usage/summary',
    method: 'GET',
  })
}

export interface AdminPortalUser {
  id: string
  username: string
  status: string
  planId: string | null
  planName: string | null
  subscriptionStartsAt: string | null
  subscriptionEndsAt: string | null
  createdAt: string
}

export function listAdminPortalUsers(
  params: { page?: number, pageSize?: number, search?: string } = {},
  options: RequestOptions = {},
) {
  return request<{ items: AdminPortalUser[], total: number }>({
    url: '/api/admin/portal/users',
    method: 'GET',
    params,
    ...options,
  })
}

export function createAdminPortalUser(data: { username: string, password: string }) {
  return request<{ id: string }>({
    url: '/api/admin/portal/users/create',
    method: 'POST',
    data,
  })
}

export function resetAdminPortalPassword(data: { userId: string, password: string }) {
  return request({
    url: '/api/admin/portal/users/reset-password',
    method: 'POST',
    data,
  })
}

export function setAdminPortalUserEnabled(userId: string, enabled: boolean) {
  return request({
    url: enabled ? '/api/admin/portal/users/enable' : '/api/admin/portal/users/disable',
    method: 'POST',
    data: { userId },
  })
}

export interface AdminPortalPlan {
  id: string
  name: string
  dailyLimitUsd: string
  weeklyLimitUsd: string
  maxConcurrency: number
  requestsPerMinute: number
  maxKeys: number
  groupIds: string[]
  enabled: boolean
}

export function listAdminPortalPlans(params: { page?: number, pageSize?: number } = {}) {
  return request<{ items: AdminPortalPlan[], total: number }>({
    url: '/api/admin/portal/plans',
    method: 'GET',
    params,
  })
}

export function createAdminPortalPlan(data: {
  name: string
  dailyLimitUsd?: string
  weeklyLimitUsd?: string
  maxConcurrency?: number
  requestsPerMinute?: number
  maxKeys?: number
  groupIds: string[]
}) {
  return request<{ id: string }>({
    url: '/api/admin/portal/plans/create',
    method: 'POST',
    data,
  })
}

export function updateAdminPortalPlan(data: {
  id: string
  name: string
  dailyLimitUsd?: string
  weeklyLimitUsd?: string
  maxConcurrency?: number
  requestsPerMinute?: number
  maxKeys?: number
  groupIds: string[]
  enabled?: boolean
}) {
  return request({
    url: '/api/admin/portal/plans/update',
    method: 'POST',
    data,
  })
}

export function assignAdminPortalSubscription(data: {
  userId: string
  planId: string
  startsAt: string
  endsAt?: string | null
}) {
  return request<{ id: string }>({
    url: '/api/admin/portal/subscriptions/assign',
    method: 'POST',
    data,
  })
}

export function disableAdminPortalSubscription(userId: string) {
  return request({
    url: '/api/admin/portal/subscriptions/disable',
    method: 'POST',
    data: { userId },
  })
}
