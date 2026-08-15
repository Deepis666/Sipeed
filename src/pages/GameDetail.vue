<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Play, FolderOpen, Pencil, ChevronLeft, Tag, X, Globe } from 'lucide-vue-next'
import { useGameLaunch, useCover, parseJsonArr, invalidateCoverCache } from '@/composables/useTauri'
import type { Game } from '@/composables/useTauri'

interface Category {
  id: number
  name: string
}

const route = useRoute()
const router = useRouter()
const game = ref<Game | null>(null)
const loading = ref(true)
const editingName = ref(false)
const editNameValue = ref('')
const categories = ref<Category[]>([])
const showCatPicker = ref(false)
const { launching, launchGame, openGameFolder } = useGameLaunch()
const { url: coverUrl } = useCover(Number(route.params.id))

const statusLabels: Record<number, string> = { 0: '未开始', 1: '在玩', 2: '玩过', 3: '搁置' }
const fetchingGB = ref(false)
const descExpanded = ref(false)

const genreList = computed(() => parseJsonArr(game.value?.genres))
const themeList = computed(() => parseJsonArr(game.value?.themes))
const modeList = computed(() => parseJsonArr(game.value?.play_modes))
const tagList = computed(() => parseJsonArr(game.value?.tags))
const needsExpand = computed(() => (game.value?.steam_description?.length ?? 0) > 200)
const showShortDesc = computed(() => {
  const s = game.value?.short_description
  const d = game.value?.steam_description
  return s && s !== d
})

async function changeStatus(status: number) {
  if (!game.value) return
  await invoke('set_game_status', { gameId: game.value.id, status })
  game.value.status = status
}

async function fetchGB() {
  if (!game.value) return
  fetchingGB.value = true
  try {
    console.log(`[DEBUG] fetchGB: 调用 fetch_gamebrain_metadata gameId=${game.value.id}`)
    const msg = await invoke<string>('fetch_gamebrain_metadata', { gameId: game.value.id })
    console.log(`[DEBUG] fetchGB: GameBrain 返回消息: ${msg}`)
    const updated = await invoke<Game>('get_game', { id: game.value.id })
    game.value = updated
    console.log(`[DEBUG] fetchGB: 已更新 game 对象, icon_path='${game.value.icon_path}'`)
    console.log(`[DEBUG] 正在保存的封面来源: GameBrain, 路径: ${game.value.icon_path}`)
    const b64 = await invoke<string | null>('read_cover_base64', { gameId: game.value.id })
    if (b64) {
      console.log(`[DEBUG] fetchGB: read_cover_base64 返回数据, 长度=${b64.length}`)
      coverUrl.value = b64
      invalidateCoverCache(game.value.id)
    } else {
      console.log(`[DEBUG] fetchGB: read_cover_base64 返回空`)
    }
    alert(msg)
  } catch (e) {
    console.error(`[DEBUG] fetchGB: 异常:`, e)
    alert(`GameBrain 获取失败: ${e}`)
  } finally {
    fetchingGB.value = false
  }
}

let unlisten: (() => void) | null = null

async function loadGame() {
  const id = Number(route.params.id)
  try {
    game.value = await invoke<Game>('get_game', { id })
  } catch {
    // game not found
  }
}

async function fetchCategories() {
  try {
    categories.value = await invoke<Category[]>('list_categories')
  } catch { /* ignore */ }
}

async function assignCategory(categoryId: number | null) {
  if (!game.value) return
  if (categoryId === null) {
    await invoke('assign_game_category', { gameId: game.value.id, categoryId: null })
    game.value.category_id = undefined
    game.value.category_name = undefined
  } else {
    await invoke('assign_game_category', { gameId: game.value.id, categoryId })
    game.value.category_id = categoryId
    game.value.category_name = categories.value.find(c => c.id === categoryId)?.name
  }
  showCatPicker.value = false
}

async function removeCategory() {
  await assignCategory(null)
}

onMounted(async () => {
  loading.value = true
  await loadGame()
  await fetchCategories()
  loading.value = false

  unlisten = await listen<number>('game-stopped', async (event) => {
    if (game.value && event.payload === game.value.id) {
      try {
        const updated = await invoke<Game>('get_game', { id: game.value.id })
        game.value = updated
      } catch { /* ignore */ }
    }
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
})

function formatPlaytime(minutes: number): string {
  if (!minutes || minutes <= 0) return '未游玩'
  const hrs = Math.floor(minutes / 60)
  const mins = minutes % 60
  if (hrs > 0) return `${hrs} 小时 ${mins} 分钟`
  return `${mins} 分钟`
}

function handleLaunch() {
  if (game.value) {
    launchGame(game.value.id)
  }
}

function handleOpenFolder() {
  if (game.value?.exe_path) {
    openGameFolder(game.value.exe_path)
  }
}

function startEditing() {
  if (game.value) {
    editNameValue.value = game.value.name
    editingName.value = true
  }
}

async function saveEdit() {
  if (!game.value || !editNameValue.value.trim()) return
  try {
    await invoke('update_game', { gameId: game.value.id, name: editNameValue.value.trim() })
    game.value.name = editNameValue.value.trim()
  } catch { /* ignore */ }
  finally { editingName.value = false }
}

function cancelEdit() {
  editingName.value = false
}

function goBack() {
  router.back()
}
</script>

<template>
  <div class="flex flex-col h-full">
    <div class="relative">
      <div
        v-if="game?.steam_image_url"
        class="h-64 bg-cover bg-center"
        :style="{ backgroundImage: `url(${game.steam_image_url})` }"
      >
        <div class="absolute inset-0 bg-gradient-to-t from-background via-background/60 to-transparent" />
      </div>
      <div v-else class="h-40 bg-gradient-to-b from-card to-background" />

      <button
        class="absolute top-4 left-4 p-2 rounded-lg bg-black/40 hover:bg-black/60 text-white transition-colors"
        @click="goBack"
      >
        <ChevronLeft class="w-5 h-5" />
      </button>
    </div>

    <div v-if="loading" class="flex items-center justify-center py-16">
      <div class="w-8 h-8 border-2 border-accent-blue border-t-transparent rounded-full animate-spin" />
    </div>

    <div v-else-if="!game" class="flex-1 flex items-center justify-center text-text-secondary">
      <p>游戏未找到</p>
    </div>

    <div v-else class="px-8 pb-8 -mt-12 relative z-10">
      <!-- Two-column: cover + info -->
      <div class="flex gap-6 mb-6">
        <div class="w-[35%] flex-shrink-0">
          <div class="aspect-[3/4] rounded-xl overflow-hidden bg-card border border-border">
            <img v-if="coverUrl" :src="coverUrl" class="w-full h-full object-cover" />
            <div v-else class="w-full h-full flex items-center justify-center">
              <Play class="w-12 h-12 text-text-secondary opacity-20" />
            </div>
          </div>
        </div>

        <div class="flex-1 min-w-0 flex flex-col gap-3">
          <input
            v-if="editingName"
            v-model="editNameValue"
            class="text-2xl font-bold bg-transparent border-b border-accent-blue text-text-primary outline-none"
            @keyup.enter="saveEdit"
            @keyup.escape="cancelEdit"
            @blur="saveEdit"
          />
          <h1 v-else class="text-2xl font-bold text-text-primary">{{ game.name }}</h1>

          <div class="flex items-center gap-2 flex-wrap">
            <span v-if="game.developer" class="text-sm text-text-secondary">{{ game.developer }}</span>
            <span v-if="game.rating" class="text-sm text-yellow-400">★ {{ (game.rating * 10).toFixed(1) }}</span>
          </div>

          <div class="flex items-center gap-3 text-sm text-text-secondary">
            <span>{{ formatPlaytime(game.total_playtime ?? 0) }}</span>
            <span v-if="game.last_played" class="text-xs text-muted-foreground">
              上次游玩: {{ new Date(game.last_played * 1000).toLocaleDateString() }}
            </span>
          </div>

          <div class="flex items-center gap-1.5">
            <span
              v-for="(label, s) in statusLabels"
              :key="s"
              class="text-xs px-2.5 py-1 rounded cursor-pointer transition-colors"
              :class="game.status === Number(s)
                ? 'bg-accent-blue/20 text-accent-blue border border-accent-blue/40'
                : 'text-text-secondary hover:text-text-primary border border-transparent hover:border-border'"
              @click="changeStatus(Number(s))"
            >
              {{ label }}
            </span>
          </div>

          <p v-if="showShortDesc" class="text-sm text-text-secondary leading-relaxed line-clamp-3">
            {{ game.short_description }}
          </p>
        </div>
      </div>

      <!-- Badges: genres / modes / themes -->
      <div v-if="genreList.length || modeList.length || themeList.length" class="mb-4 space-y-2.5">
        <div v-if="genreList.length" class="flex items-center gap-2">
          <span class="text-xs opacity-50 flex-shrink-0">🎮 类型</span>
          <div class="flex flex-wrap gap-1.5">
            <span v-for="g in genreList" :key="g" class="text-[13px] px-[14px] py-[6px] rounded-2xl bg-blue-500/15 text-blue-400 border border-blue-500/20">{{ g }}</span>
          </div>
        </div>
        <div v-if="modeList.length" class="flex items-center gap-2">
          <span class="text-xs opacity-50 flex-shrink-0">⚔️ 模式</span>
          <div class="flex flex-wrap gap-1.5">
            <span v-for="m in modeList" :key="m" class="text-[13px] px-[14px] py-[6px] rounded-2xl bg-green-500/15 text-green-400 border border-green-500/20">{{ m }}</span>
          </div>
        </div>
        <div v-if="themeList.length" class="flex items-center gap-2">
          <span class="text-xs opacity-50 flex-shrink-0">🌌 主题</span>
          <div class="flex flex-wrap gap-1.5">
            <span v-for="t in themeList" :key="t" class="text-[13px] px-[14px] py-[6px] rounded-2xl bg-purple-500/15 text-purple-400 border border-purple-500/20">{{ t }}</span>
          </div>
        </div>
      </div>

      <!-- Tags -->
      <div v-if="tagList.length" class="flex flex-wrap gap-x-[6px] mb-4 text-xs text-[#888]">
        <span v-for="t in tagList" :key="t">• {{ t }}</span>
      </div>

      <!-- Buttons -->
      <div class="flex items-center gap-3 mb-4">
        <button class="flex items-center gap-2 px-6 py-2.5 bg-accent-blue text-white rounded-lg text-sm font-medium hover:brightness-110 transition-all active:scale-95" :disabled="launching" @click="handleLaunch">
          <div v-if="launching" class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
          <Play v-else class="w-4 h-4" />
          {{ launching ? '启动中...' : '启动' }}
        </button>
        <button class="flex items-center gap-2 px-4 py-2.5 rounded-lg bg-card border border-border text-text-secondary text-sm hover:text-text-primary hover:border-accent-blue/40 transition-colors" @click="handleOpenFolder">
          <FolderOpen class="w-4 h-4" /> 打开文件夹
        </button>
        <button class="flex items-center gap-2 px-4 py-2.5 rounded-lg bg-card border border-border text-text-secondary text-sm hover:text-text-primary hover:border-accent-blue/40 transition-colors" @click="startEditing">
          <Pencil class="w-4 h-4" /> 编辑
        </button>
        <button class="flex items-center gap-2 px-4 py-2.5 rounded-lg bg-card border border-border text-text-secondary text-sm hover:text-text-primary hover:border-accent-blue/40 transition-colors" :disabled="fetchingGB" @click="fetchGB">
          <div v-if="fetchingGB" class="w-4 h-4 border-2 border-text-secondary border-t-transparent rounded-full animate-spin" />
          <Globe v-else class="w-4 h-4" />
          {{ fetchingGB ? '获取中...' : '获取详情' }}
        </button>
      </div>

      <!-- Category -->
      <div class="mb-6">
        <div class="flex items-center gap-2">
          <Tag class="w-4 h-4 text-text-secondary" />
          <span v-if="game.category_name" class="text-sm text-text-primary bg-accent-blue/10 px-2 py-0.5 rounded">
            {{ game.category_name }}
            <button class="ml-1 text-text-secondary hover:text-red-400 align-middle" @click="removeCategory"><X class="w-3 h-3 inline" /></button>
          </span>
          <button v-if="!showCatPicker" class="text-sm text-text-secondary hover:text-accent-blue transition-colors" @click="showCatPicker = true">
            {{ game.category_name ? '更改分类' : '+ 添加分类' }}
          </button>
        </div>
        <div v-if="showCatPicker" class="mt-2 flex flex-wrap gap-1.5">
          <button v-for="cat in categories" :key="cat.id" class="text-xs px-2.5 py-1 rounded bg-card border border-border hover:border-accent-blue/40 text-text-secondary hover:text-text-primary transition-colors" :class="{ 'border-accent-blue text-accent-blue': cat.id === game.category_id }" @click="assignCategory(cat.id)">{{ cat.name }}</button>
          <button class="text-xs px-2.5 py-1 rounded text-text-secondary hover:text-text-primary" @click="showCatPicker = false">取消</button>
        </div>
      </div>

      <!-- Full Description -->
      <div v-if="game.steam_description" class="border-t border-border pt-4">
        <p class="text-sm leading-relaxed" :class="descExpanded ? 'text-text-secondary' : 'text-[#888] line-clamp-3'">{{ game.steam_description }}</p>
        <button v-if="needsExpand" class="mt-1 text-xs text-accent-blue hover:underline" @click="descExpanded = !descExpanded">
          {{ descExpanded ? '收起 ▲' : '... 展开全文 ▼' }}
        </button>
      </div>
    </div>
  </div>
</template>
