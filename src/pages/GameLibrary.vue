<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useGames } from '@/composables/useTauri'
import SearchBar from '@/components/SearchBar.vue'
import GameCard from '@/components/GameCard.vue'
import ContextMenu from '@/components/ContextMenu.vue'
import type { ContextMenuItem } from '@/components/ContextMenu.vue'
import type { Game } from '@/composables/useTauri'
import { Plus, ChevronDown, CheckSquare } from 'lucide-vue-next'

interface CatInfo { id: number; name: string; count: number }

const { games, loading, error, fetchGames } = useGames()
const searchQuery = ref('')
const sortBy = ref<'name' | 'recent'>('name')
const selectedCategoryId = ref<number | null | 'uncat'>(null)
const catList = ref<CatInfo[]>([])
const selectedGameIds = ref<Set<number>>(new Set())
const batchMode = ref(false)

// Modals
const showCreateCat = ref(false)
const showEditCat = ref<CatInfo | null>(null)
const showDeleteCat = ref<CatInfo | null>(null)
const deleteTransferTo = ref<number | null>(null)
const newCatName = ref('')
const batchMoveOpen = ref(false)

// Context menu
const gameCtxMenu = ref<{ x: number; y: number; game: Game } | null>(null)

// Toast
const toast = ref('')
const toastAction = ref<(() => void) | null>(null)
let toastTimer: ReturnType<typeof setTimeout> | null = null
function showToast(msg: string, action?: () => void) {
  toast.value = msg
  toastAction.value = action ?? null
  if (toastTimer) clearTimeout(toastTimer)
  toastTimer = setTimeout(() => { toast.value = ''; toastAction.value = null }, 4000)
}

const filteredAndSorted = computed<Game[]>(() => {
  let result = [...games.value]
  if (selectedCategoryId.value === 'uncat') result = result.filter(g => !g.category_id)
  else if (selectedCategoryId.value !== null) result = result.filter(g => g.category_id === selectedCategoryId.value)
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase().trim()
    result = result.filter((g) => g.name.toLowerCase().includes(q))
  }
  if (sortBy.value === 'name') result.sort((a, b) => a.name.localeCompare(b.name))
  else if (sortBy.value === 'recent') result.sort((a, b) => (b.last_played ?? 0) - (a.last_played ?? 0))
  return result
})

const uncatCount = computed(() => games.value.filter(g => !g.category_id).length)
const selectCount = computed(() => selectedGameIds.value.size)

async function refresh() {
  await fetchGames()
  try { catList.value = await invoke<CatInfo[]>('list_categories_with_counts') } catch { /* ignore */ }
}

function toggleBatchMode() { batchMode.value = !batchMode.value; if (!batchMode.value) selectedGameIds.value.clear() }
function toggleSelect(gameId: number) { if (selectedGameIds.value.has(gameId)) selectedGameIds.value.delete(gameId); else selectedGameIds.value.add(gameId) }
function selectAll() { filteredAndSorted.value.forEach(g => selectedGameIds.value.add(g.id)) }
function clearAll() { selectedGameIds.value.clear() }

async function doCreateCategory() {
  const name = newCatName.value.trim()
  if (!name) return
  try { await invoke('add_category', { name }); showCreateCat.value = false; newCatName.value = ''; await refresh(); showToast(`分类 "${name}" 已创建`) }
  catch (e) { alert('创建失败: ' + e) }
}

async function doRenameCategory() {
  const cat = showEditCat.value
  if (!cat) return
  try { await invoke('rename_category', { id: cat.id, name: newCatName.value.trim() }); showEditCat.value = null; newCatName.value = ''; await refresh() }
  catch (e) { alert('重命名失败: ' + e) }
}

async function doDeleteCategory() {
  const cat = showDeleteCat.value
  if (!cat) return
  try { await invoke('delete_category_with_transfer', { id: cat.id, transferTo: deleteTransferTo.value }); showDeleteCat.value = null; deleteTransferTo.value = null; selectedCategoryId.value = null; await refresh(); showToast(`分类 "${cat.name}" 已删除`) }
  catch (e) { alert('删除失败: ' + e) }
}

async function handleCategoryChange(gameId: number, catId: number | null) {
  await invoke('assign_game_category', { gameId, categoryId: catId })
  await refresh()
}

async function handleDelete(gameId: number) { await invoke('delete_game', { gameId }); await refresh() }

async function batchMove(targetCatId: number | null) {
  if (selectedGameIds.value.size === 0) return
  const count = selectedGameIds.value.size
  const targetName = targetCatId ? catList.value.find(c => c.id === targetCatId)?.name ?? '所选分类' : '未分类'
  await invoke('batch_move_games', { gameIds: [...selectedGameIds.value], categoryId: targetCatId })
  selectedGameIds.value.clear()
  batchMoveOpen.value = false
  await refresh()
  showToast(`已移动 ${count} 款游戏到 ${targetName}`)
}

async function batchRemove() {
  if (selectedGameIds.value.size === 0) return
  const count = selectedGameIds.value.size
  await invoke('batch_move_games', { gameIds: [...selectedGameIds.value], categoryId: null })
  selectedGameIds.value.clear()
  await refresh()
  showToast(`已移动 ${count} 款游戏到未分类`)
}

function gameMenuItems(game: Game): ContextMenuItem[] {
  const items: ContextMenuItem[] = []
  if (game.category_id) items.push({ label: '移出分类', action: () => handleCategoryChange(game.id, null) })
  items.push({
    label: game.category_id ? '移动到其他分类 ▸' : '添加到分类 ▸',
    children: [
      ...catList.value.filter(c => c.id !== game.category_id).map(c => ({ label: c.name, action: () => handleCategoryChange(game.id, c.id) })),
      ...(game.category_id ? [{ label: '未分类', action: () => handleCategoryChange(game.id, null) }] : []),
    ],
  } as ContextMenuItem)
  return items
}

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    gameCtxMenu.value = null; showCreateCat.value = false; showEditCat.value = null; showDeleteCat.value = null
  }
}

onMounted(async () => { await refresh(); document.addEventListener('keydown', onKeyDown) })
onUnmounted(() => { document.removeEventListener('keydown', onKeyDown) })
</script>

<template>
  <div class="flex flex-col h-full">
    <SearchBar v-model:query="searchQuery" v-model:sort="sortBy">
      <template #extra>
        <button
          class="flex items-center gap-1.5 h-8 px-3 rounded-lg text-xs font-medium transition-colors flex-shrink-0"
          :class="batchMode ? 'bg-accent-blue text-white' : 'bg-card border border-border text-text-secondary hover:text-text-primary'"
          @click="toggleBatchMode"
        >
          <CheckSquare class="w-3.5 h-3.5" />
          批量操作
        </button>
      </template>
    </SearchBar>

    <!-- Category tabs -->
    <div class="flex items-center gap-1 h-10 px-4 border-b border-border bg-background overflow-x-auto flex-shrink-0">
      <button
        class="text-xs px-3 py-1 rounded-full transition-colors flex-shrink-0"
        :class="selectedCategoryId === null ? 'bg-accent-blue text-white' : 'text-text-secondary hover:text-text-primary hover:bg-card'"
        @click="selectedCategoryId = null"
      >
        全部 ({{ games.length }})
      </button>
      <button
        v-for="cat in catList" :key="cat.id"
        class="text-xs px-3 py-1 rounded-full transition-colors flex-shrink-0"
        :class="selectedCategoryId === cat.id ? 'bg-accent-blue text-white' : 'text-text-secondary hover:text-text-primary hover:bg-card'"
        @click="selectedCategoryId = cat.id"
      >
        {{ cat.name }} ({{ cat.count }})
      </button>
      <button
        class="text-xs px-3 py-1 rounded-full transition-colors flex-shrink-0"
        :class="selectedCategoryId === 'uncat' ? 'bg-accent-blue text-white' : 'text-text-secondary hover:text-text-primary hover:bg-card'"
        @click="selectedCategoryId = 'uncat'"
      >
        未分类 ({{ uncatCount }})
      </button>
      <button class="text-xs px-2 py-1 text-text-secondary hover:text-accent-blue transition-colors flex-shrink-0 ml-1" @click="showCreateCat = true">
        <Plus class="w-3 h-3 inline" /> 新建分类
      </button>
    </div>

    <!-- Batch toolbar -->
    <div v-if="batchMode && selectCount > 0" class="flex items-center gap-3 h-10 px-4 bg-accent-blue/5 border-b border-border text-sm flex-shrink-0">
      <span class="text-text-secondary">已选 {{ selectCount }} 款</span>
      <button class="text-accent-blue hover:underline" @click="batchMoveOpen = true">移动到分类 <ChevronDown class="w-3 h-3 inline" /></button>
      <button class="text-accent-blue hover:underline" @click="batchRemove">移出分类</button>
      <button class="text-text-secondary hover:text-text-primary ml-auto" @click="selectAll">全选</button>
      <button class="text-text-secondary hover:text-text-primary" @click="clearAll">取消选择</button>
    </div>

    <div class="flex-1 overflow-y-auto p-8">
      <div v-if="loading" class="flex items-center justify-center py-16">
        <div class="w-8 h-8 border-2 border-accent-blue border-t-transparent rounded-full animate-spin" />
      </div>
      <div v-else-if="error" class="py-16 text-center">
        <p class="text-sm text-red-400">加载失败: {{ error }}</p>
        <button class="mt-2 text-xs text-accent-blue hover:underline" @click="refresh">重试</button>
      </div>
      <div v-else-if="filteredAndSorted.length === 0 && !searchQuery" class="py-16 text-center text-text-secondary">
        <p class="text-sm">暂无游戏</p>
        <p class="text-xs mt-1">前往设置添加扫描文件夹</p>
      </div>
      <div v-else-if="filteredAndSorted.length === 0" class="py-16 text-center text-text-secondary">
        <p class="text-sm">未找到匹配的游戏</p>
      </div>
      <div v-else class="overflow-x-auto grid gap-4" style="grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));">
        <div
          v-for="game in filteredAndSorted" :key="game.id"
          class="relative" @contextmenu.prevent="gameCtxMenu = { x: $event.clientX, y: $event.clientY, game }"
        >
          <GameCard
            :game="game" :categories="catList" :batch-mode="batchMode" :selected="selectedGameIds.has(game.id)"
            @delete="handleDelete" @change-category="handleCategoryChange" @toggle-select="toggleSelect"
          />
        </div>
      </div>
    </div>
  </div>

  <!-- Context menu -->
  <ContextMenu v-if="gameCtxMenu" :items="gameMenuItems(gameCtxMenu.game)" :x="gameCtxMenu.x" :y="gameCtxMenu.y" @close="gameCtxMenu = null" />

  <!-- Batch move modal -->
  <Teleport to="body">
    <div v-if="batchMoveOpen" class="fixed inset-0 z-50 flex items-center justify-center bg-black/30" @click.self="batchMoveOpen = false">
      <div class="w-[200px] rounded-xl bg-card border border-border shadow-xl py-2">
        <p class="text-xs text-text-secondary px-3 py-1">移动到分类</p>
        <div class="border-t border-border my-1" />
        <button v-for="cat in catList" :key="cat.id" class="w-full text-left px-3 py-1.5 text-sm text-text-primary hover:bg-accent-blue/10" @click="batchMove(cat.id)">{{ cat.name }}</button>
        <div class="border-t border-border my-1" />
        <button class="w-full text-left px-3 py-1.5 text-sm text-text-secondary hover:bg-accent-blue/10" @click="batchMove(null)">移至未分类</button>
      </div>
    </div>
  </Teleport>

  <!-- Create category -->
  <Teleport to="body">
    <div v-if="showCreateCat" class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" @click.self="showCreateCat = false">
      <div class="w-[360px] rounded-xl bg-card border border-border p-6 shadow-xl">
        <h3 class="text-sm font-semibold text-text-primary mb-4">新建分类</h3>
        <input v-model="newCatName" type="text" placeholder="分类名称..." class="w-full h-9 px-3 rounded-lg bg-background border border-border text-sm mb-4 outline-none focus:border-accent-blue/50" @keyup.enter="doCreateCategory" />
        <div class="flex justify-end gap-2">
          <button class="h-9 px-4 rounded-lg text-sm text-text-secondary hover:text-text-primary" @click="showCreateCat = false">取消</button>
          <button class="h-9 px-4 rounded-lg bg-accent-blue text-white text-sm font-medium hover:brightness-110" @click="doCreateCategory">创建</button>
        </div>
      </div>
    </div>
  </Teleport>

  <!-- Edit category -->
  <Teleport to="body">
    <div v-if="showEditCat" class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" @click.self="showEditCat = null">
      <div class="w-[360px] rounded-xl bg-card border border-border p-6 shadow-xl">
        <h3 class="text-sm font-semibold text-text-primary mb-4">重命名 "{{ showEditCat.name }}"</h3>
        <input v-model="newCatName" type="text" placeholder="新名称..." class="w-full h-9 px-3 rounded-lg bg-background border border-border text-sm mb-4 outline-none focus:border-accent-blue/50" @keyup.enter="doRenameCategory" />
        <div class="flex justify-end gap-2">
          <button class="h-9 px-4 rounded-lg text-sm text-text-secondary hover:text-text-primary" @click="showEditCat = null">取消</button>
          <button class="h-9 px-4 rounded-lg bg-accent-blue text-white text-sm font-medium hover:brightness-110" @click="doRenameCategory">确定</button>
        </div>
      </div>
    </div>
  </Teleport>

  <!-- Delete category -->
  <Teleport to="body">
    <div v-if="showDeleteCat" class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" @click.self="showDeleteCat = null">
      <div class="w-[420px] rounded-xl bg-card border border-border p-6 shadow-xl">
        <h3 class="text-sm font-semibold text-text-primary mb-2">删除分类 "{{ showDeleteCat.name }}"</h3>
        <p class="text-xs text-text-secondary mb-4">该分类下有 {{ showDeleteCat.count }} 款游戏</p>
        <label class="flex items-center gap-2 mb-3 cursor-pointer text-sm">
          <input type="radio" name="delOpt" :checked="deleteTransferTo === null" @change="deleteTransferTo = null" class="accent-accent-blue" />
          删除分类，游戏变为未分类
        </label>
        <label class="flex items-center gap-2 mb-4 cursor-pointer text-sm">
          <input type="radio" name="delOpt" :checked="deleteTransferTo !== null" @change="deleteTransferTo = catList[0]?.id ?? null" class="accent-accent-blue" />
          将游戏移至：
          <select :value="deleteTransferTo" @change="deleteTransferTo = Number(($event.target as HTMLSelectElement).value) || null" class="h-7 px-2 rounded bg-background border border-border text-xs outline-none" :disabled="deleteTransferTo === null">
            <option v-for="c in catList.filter(c => c.id !== showDeleteCat?.id)" :key="c.id" :value="c.id">{{ c.name }}</option>
          </select>
        </label>
        <div class="flex justify-end gap-2">
          <button class="h-9 px-4 rounded-lg text-sm text-text-secondary hover:text-text-primary" @click="showDeleteCat = null">取消</button>
          <button class="h-9 px-4 rounded-lg bg-red-500 text-white text-sm font-medium hover:brightness-110" @click="doDeleteCategory">确认删除</button>
        </div>
      </div>
    </div>
  </Teleport>

  <!-- Toast -->
  <Teleport to="body">
    <div v-if="toast" class="fixed bottom-8 left-1/2 -translate-x-1/2 z-[200] px-4 py-2 rounded-lg bg-accent-blue text-white text-sm shadow-lg transition-all cursor-pointer" @click="toastAction?.()">
      {{ toast }}
      <span v-if="toastAction" class="ml-2 underline text-xs">点击分配</span>
    </div>
  </Teleport>
</template>
