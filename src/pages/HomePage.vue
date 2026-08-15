<script setup lang="ts">
import { onMounted } from 'vue'
import { RouterLink } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { Gamepad2, Clock, ChevronRight } from 'lucide-vue-next'
import { useRecentGames } from '@/composables/useTauri'
import GameCard from '@/components/GameCard.vue'

const { games, loading, fetchRecent } = useRecentGames()

async function handleDelete(gameId: number) {
  await invoke('delete_game', { gameId })
  await fetchRecent()
}

onMounted(() => {
  fetchRecent()
})
</script>

<template>
  <div class="p-8">
    <h1 class="text-2xl font-bold text-text-primary mb-8">主页</h1>

    <section>
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center gap-2">
          <Clock class="w-5 h-5 text-accent-blue" />
          <h2 class="text-lg font-semibold text-text-primary">最近游玩</h2>
        </div>
        <RouterLink
          to="/library"
          class="flex items-center gap-1 text-sm text-text-secondary hover:text-text-primary transition-colors"
        >
          查看全部
          <ChevronRight class="w-4 h-4" />
        </RouterLink>
      </div>

      <div v-if="loading" class="flex items-center justify-center py-16">
        <div class="w-8 h-8 border-2 border-accent-blue border-t-transparent rounded-full animate-spin" />
      </div>

      <div v-else-if="games.length === 0" class="flex flex-col items-center justify-center py-16 text-text-secondary">
        <Gamepad2 class="w-12 h-12 mb-3 opacity-30" />
        <p class="text-sm">暂无游玩记录</p>
        <p class="text-xs mt-1">去游戏库启动游戏吧</p>
      </div>

      <div v-else class="overflow-x-auto grid gap-4" style="grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));">
        <GameCard
          v-for="game in games"
          :key="game.id"
          :game="game"
          @delete="handleDelete"
        />
      </div>
    </section>

    <section class="mt-10">
      <RouterLink
        to="/library"
        class="inline-flex items-center gap-2 px-5 py-3 rounded-lg bg-card border border-border hover:border-accent-blue/50 transition-colors"
      >
        <Gamepad2 class="w-5 h-5 text-accent-blue" />
        <span class="text-sm font-medium text-text-primary">浏览所有游戏</span>
        <ChevronRight class="w-4 h-4 text-text-secondary" />
      </RouterLink>
    </section>
  </div>
</template>
