import type { RouteRecordRaw } from 'vue-router'

export const routes: RouteRecordRaw[] = [
  { path: '/api-keys', redirect: '/keys' },
  { path: '/account-groups', redirect: '/groups' },
  {
    path: '/login',
    name: 'login',
    component: () => import('@/views/login/index.vue'),
  },
  {
    path: '/key-usage',
    name: 'key-usage',
    component: () => import('@/views/key-usage/index.vue'),
  },
  {
    path: '/',
    component: () => import('@/layout/index.vue'),
    children: [
      {
        path: '',
        name: 'dashboard',
        component: () => import('@/views/dashboard/index.vue'),
      },
      {
        path: 'accounts',
        name: 'accounts',
        component: () => import('@/views/accounts/index.vue'),
      },
      {
        path: 'proxies',
        name: 'proxies',
        component: () => import('@/views/proxies/index.vue'),
      },
      {
        path: 'groups',
        name: 'groups',
        component: () => import('@/views/groups/index.vue'),
      },
      {
        path: 'keys',
        name: 'keys',
        component: () => import('@/views/keys/index.vue'),
      },
      {
        path: 'portal-users',
        name: 'portal-users',
        component: () => import('@/views/portal-admin/users.vue'),
      },
      {
        path: 'portal-plans',
        name: 'portal-plans',
        component: () => import('@/views/portal-admin/plans.vue'),
      },
      {
        path: 'usage',
        name: 'usage',
        component: () => import('@/views/usage/index.vue'),
      },
      {
        path: 'theme',
        name: 'theme',
        component: () => import('@/views/theme/index.vue'),
      },
      {
        path: 'settings',
        name: 'settings',
        component: () => import('@/views/settings/index.vue'),
      },
      {
        path: 'settings/backup',
        name: 'settings-backup',
        component: () => import('@/views/settings/index.vue'),
      },
    ],
  },
  {
    path: '/portal/login',
    name: 'portal-login',
    redirect: '/login',
  },
  {
    path: '/portal',
    component: () => import('@/views/portal/layout.vue'),
    children: [
      {
        path: '',
        name: 'portal-home',
        component: () => import('@/views/portal/index.vue'),
      },
      {
        path: 'keys',
        name: 'portal-keys',
        component: () => import('@/views/portal/keys/index.vue'),
      },
      {
        path: 'usage',
        name: 'portal-usage',
        component: () => import('@/views/portal/usage.vue'),
      },
      {
        path: 'me',
        name: 'portal-me',
        component: () => import('@/views/portal/me.vue'),
      },
    ],
  },
  {
    path: '/:pathMatch(.*)*',
    redirect: '/',
  },
]
