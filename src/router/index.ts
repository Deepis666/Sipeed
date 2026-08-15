import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'home',
      component: () => import('@/pages/HomePage.vue'),
    },
    {
      path: '/library',
      name: 'library',
      component: () => import('@/pages/GameLibrary.vue'),
    },
    {
      path: '/game/:id',
      name: 'game-detail',
      component: () => import('@/pages/GameDetail.vue'),
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('@/pages/SettingsPage.vue'),
    },
  ],
})

export default router
