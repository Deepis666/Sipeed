<script setup lang="ts">
import { useRoute } from 'vue-router'
import { Home, Gamepad2, Settings } from 'lucide-vue-next'
import { cn } from '@/lib/utils'

const route = useRoute()

const menuItems = [
  { path: '/', label: '主页', icon: Home },
  { path: '/library', label: '游戏库', icon: Gamepad2 },
  { path: '/settings', label: '设置', icon: Settings },
]
</script>

<template>
  <aside class="w-[240px] h-screen flex flex-col bg-sidebar border-r border-border flex-shrink-0 select-none">
    <div class="flex items-center gap-2 px-5 h-14 border-b border-border">
      <Gamepad2 class="w-6 h-6 text-accent-blue" />
      <span class="text-sm font-semibold tracking-wide text-text-primary">Sipeed</span>
    </div>

    <nav class="flex flex-col gap-1 px-3 py-4 flex-1">
      <RouterLink
        v-for="item in menuItems"
        :key="item.path"
        :to="item.path"
        :class="
          cn(
            'relative flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors duration-150',
            route.path === item.path
              ? 'text-text-primary bg-white/5 before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:w-[3px] before:h-5 before:rounded-full before:bg-accent-blue'
              : 'text-text-secondary hover:text-text-primary hover:bg-white/5'
          )
        "
      >
        <component :is="item.icon" class="w-5 h-5 flex-shrink-0" />
        <span>{{ item.label }}</span>
      </RouterLink>
    </nav>

    <div class="px-5 py-3 border-t border-border">
      <span class="text-xs text-text-secondary">Sipeed v0.1.0</span>
    </div>
  </aside>
</template>
