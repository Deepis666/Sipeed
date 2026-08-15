<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useGameCover } from '@/composables/useTauri'
import type { Game } from '@/composables/useTauri'
import { ChevronDown, Check } from 'lucide-vue-next'

const props = defineProps<{
  game: Game
  categories?: { id: number; name: string; count: number }[]
  batchMode?: boolean
  selected?: boolean
}>()

const router = useRouter()
const { url: coverUrl, loading: coverLoading } = useGameCover(props.game.id, props.game.exe_path)
const showCatMenu = ref(false)

const emit = defineEmits<{
  delete: [gameId: number]
  changeCategory: [gameId: number, catId: number | null]
  toggleSelect: [gameId: number]
}>()

function handleClick() {
  if (props.batchMode) {
    emit('toggleSelect', props.game.id)
  } else {
    router.push(`/game/${props.game.id}`)
  }
}

function handleDelete(gameId: number) {
  if (confirm('确定要删除这个游戏吗？')) {
    emit('delete', gameId)
  }
}

function lastPlayedText(lastPlayed?: number): string {
  if (!lastPlayed) return ''
  const d = new Date(lastPlayed * 1000)
  const now = new Date()
  const diff = now.getTime() - d.getTime()
  const days = Math.floor(diff / (1000 * 60 * 60 * 24))
  if (days === 0) return '今天'
  if (days === 1) return '昨天'
  if (days < 7) return `${days} 天前`
  return d.toLocaleDateString()
}

function statusLabel(status: number): string {
  const labels: Record<number, string> = { 0: '', 1: '在玩', 2: '玩过', 3: '搁置' }
  return labels[status] ?? ''
}

function statusColor(status: number): string {
  const colors: Record<number, string> = {
    1: 'bg-green-500/20 text-green-400 border-green-500/30',
    2: 'bg-blue-500/20 text-blue-400 border-blue-500/30',
    3: 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30',
  }
  return colors[status] ?? ''
}
</script>

<template>
  <div
    class="group relative flex flex-col w-full aspect-[3/4] p-3 rounded-xl bg-card border transition-all duration-200 cursor-pointer"
    :class="[
      batchMode && selected
        ? 'border-accent-blue shadow-[0_0_0_2px_rgba(59,130,246,0.3)]'
        : 'border-game-border hover:border-accent-blue/40 hover:-translate-y-1 hover:shadow-lg hover:shadow-black/30',
    ]"
    @click="handleClick"
  >
    <div class="w-full aspect-square rounded-lg overflow-hidden bg-black/20 mb-2">
      <div v-if="coverLoading" class="w-full h-full bg-white/5 animate-pulse" />
      <img
        v-else-if="coverUrl"
        :src="coverUrl"
        loading="lazy"
        class="w-full h-full object-cover"
      />
      <div v-else class="w-full h-full flex items-center justify-center">
        <span class="text-text-secondary text-2xl font-bold opacity-40">{{ game.name.charAt(0) }}</span>
      </div>
    </div>

    <!-- Batch select checkbox -->
    <div
      v-if="batchMode"
      class="absolute top-2 left-2 w-5 h-5 rounded-full border-2 flex items-center justify-center transition-colors z-10"
      :class="selected ? 'bg-accent-blue border-accent-blue' : 'border-white/40 bg-black/30'"
      @click.stop="emit('toggleSelect', game.id)"
    >
      <Check v-if="selected" class="w-3 h-3 text-white" />
    </div>

    <span
      v-if="game.status > 0 && !batchMode"
      class="absolute top-2 left-2 text-[10px] px-1.5 py-0.5 rounded border leading-none"
      :class="statusColor(game.status)"
    >
      {{ statusLabel(game.status) }}
    </span>

    <p class="flex-1 w-full text-center text-sm text-text-primary leading-tight line-clamp-2 group-hover:text-accent-blue transition-colors">
      {{ game.name }}
    </p>

    <div v-if="game.category_name && !batchMode" class="relative mt-1 text-center" @click.stop>
      <div class="inline-flex items-center gap-0.5">
        <span
          v-if="game.category_name"
          class="text-[10px] text-accent-blue bg-accent-blue/10 px-1.5 py-0.5 rounded cursor-pointer"
          @click.stop="showCatMenu = !showCatMenu"
        >
          {{ game.category_name }}
          <ChevronDown class="w-2.5 h-2.5 inline ml-0.5" />
        </span>
      </div>

      <div
        v-if="showCatMenu"
        class="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 w-28 bg-card border border-border rounded-lg shadow-xl z-20 py-1"
      >
        <button
          v-if="game.category_id"
          class="w-full text-left text-[11px] px-3 py-1.5 hover:bg-accent-blue/10 text-text-secondary"
          @click.stop="emit('changeCategory', game.id, null); showCatMenu = false"
        >
          移除分类
        </button>
        <button
          v-for="cat in (categories ?? [])"
          :key="cat.id"
          class="w-full text-left text-[11px] px-3 py-1.5 hover:bg-accent-blue/10 transition-colors"
          :class="cat.id === game.category_id ? 'text-accent-blue' : 'text-text-primary'"
          @click.stop="emit('changeCategory', game.id, cat.id); showCatMenu = false"
        >
          {{ cat.name }}
        </button>
      </div>
      <div v-if="showCatMenu" class="fixed inset-0 z-10" @click="showCatMenu = false" />
    </div>

    <span class="absolute bottom-2 right-3 text-[10px] text-text-secondary group-hover:opacity-0 transition-opacity">
      {{ lastPlayedText(game.last_played) }}
    </span>

    <button
      v-if="!batchMode"
      class="absolute bottom-2 right-2 w-9 h-9 rounded-full bg-red-600/60 hover:bg-red-600/90 text-white text-lg leading-none flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity duration-200"
      @click.stop="handleDelete(game.id)"
    >
      🗑
    </button>
  </div>
</template>
