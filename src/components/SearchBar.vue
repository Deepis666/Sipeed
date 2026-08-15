<script setup lang="ts">
import { Search, ArrowUpDown } from 'lucide-vue-next'

const query = defineModel<string>('query', { required: true })
const sort = defineModel<'name' | 'recent'>('sort', { required: true })
</script>

<template>
  <div class="flex items-center h-14 px-8 border-b border-border gap-3 bg-background">
    <div class="relative flex-1 max-w-md">
      <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text-secondary pointer-events-none" />
      <input
        v-model="query"
        type="text"
        placeholder="搜索游戏..."
        class="w-full h-9 pl-9 pr-4 rounded-[20px] bg-secondary border border-transparent text-sm text-text-primary placeholder:text-text-secondary/50 focus:outline-none focus:border-accent-blue/40 transition-colors"
      />
    </div>

    <div class="relative">
      <select
        v-model="sort"
        class="appearance-none h-9 pl-3 pr-8 rounded-lg bg-card border border-border text-sm text-text-primary focus:outline-none focus:border-accent-blue/40 transition-colors cursor-pointer"
      >
        <option value="name">按名称</option>
        <option value="recent">按最近游玩</option>
      </select>
      <ArrowUpDown class="absolute right-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-secondary pointer-events-none" />
    </div>

    <slot name="extra" />
  </div>
</template>
