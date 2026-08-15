<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { FolderPlus, Info, Key, X, Plus } from 'lucide-vue-next'

const scanning = ref(false)
const scanMsg = ref('')
const sgdbApiKey = ref('')
const gbApiKey = ref('')
const newFolderInput = ref('')
const saving = ref(false)

// Edit game info modal
const showEditModal = ref(false)
const editFilePath = ref('')
const editGameName = ref('')
const editCategoryId = ref<number | null>(null)
const editCategories = ref<{ id: number; name: string }[]>([])
const showInlineCat = ref(false)
const inlineCatName = ref('')
const importing = ref(false)
const importToast = ref('')
let importToastTimer: ReturnType<typeof setTimeout> | null = null

onMounted(async () => {
  try { const saved = await invoke<string | null>('get_setting', { key: 'sgdb_api_key' }); if (saved) sgdbApiKey.value = saved } catch { /* ignore */ }
  try { const saved = await invoke<string | null>('get_setting', { key: 'gb_api_key' }); if (saved) gbApiKey.value = saved } catch { /* ignore */ }
})

let saveTimer: ReturnType<typeof setTimeout> | null = null
function autoSaveSetting(key: string, value: string) {
  if (saveTimer) clearTimeout(saveTimer)
  saveTimer = setTimeout(async () => {
    saving.value = true
    try { await invoke('set_setting', { key, value }) } catch { /* ignore */ }
    finally { saving.value = false }
  }, 600)
}
watch(sgdbApiKey, (val) => autoSaveSetting('sgdb_api_key', val))
watch(gbApiKey, (val) => autoSaveSetting('gb_api_key', val))

function showToast(msg: string) {
  importToast.value = msg
  if (importToastTimer) clearTimeout(importToastTimer)
  importToastTimer = setTimeout(() => { importToast.value = '' }, 3000)
}

function extractGameName(path: string): string {
  const name = path.split(/[\\/]/).pop() || ''
  return name
    .replace(/\.exe$/i, '')
    .replace(/[-_.](win64|x64|x86|Shipping|Launcher|v[\d.]+|Steam|GOG)$/gi, '')
    .replace(/[-_]/g, ' ')
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/\s+/g, ' ')
    .trim()
}

// Browse and open edit modal
async function browseFile() {
  const selected = await open({
    multiple: false, title: '选择游戏程序',
    filters: [{ name: '可执行程序', extensions: ['exe', 'app', 'bat', 'cmd'] }],
  })
  if (selected) openEditModal(selected as string)
}

async function handleScanPath() {
  const path = newFolderInput.value.trim()
  if (!path) return
  newFolderInput.value = ''
  openEditModal(path)
}

async function openEditModal(path: string) {
  editFilePath.value = path
  editGameName.value = extractGameName(path)
  editCategoryId.value = null
  showInlineCat.value = false
  inlineCatName.value = ''
  try { editCategories.value = await invoke('list_categories') } catch { editCategories.value = [] }
  showEditModal.value = true
}

async function doInlineCreateCat() {
  const name = inlineCatName.value.trim()
  if (!name) return
  try {
    await invoke('add_category', { name })
    editCategories.value = await invoke('list_categories')
    inlineCatName.value = ''
    showInlineCat.value = false
    // Auto-select the new category
    const newCat = editCategories.value.find(c => c.name === name)
    if (newCat) editCategoryId.value = newCat.id
  } catch (e) { alert('创建失败: ' + e) }
}

async function doImport() {
  if (!editFilePath.value || !editGameName.value.trim()) return
  importing.value = true
  try {
    // Step 1: Scan and import
    const result = await invoke<{ inserted_games: number }>('scan_folder', { path: editFilePath.value, categoryId: null })
    let gameId: number | null = null
    if (result.inserted_games > 0) {
      const games = await invoke<{ id: number }[]>('list_games')
      gameId = games[games.length - 1]?.id ?? null
    } else {
      // Game may already exist, find by exe_path
      const games = await invoke<{ id: number; exe_path: string }[]>('list_games')
      const found = games.find(g => g.exe_path === editFilePath.value)
      gameId = found?.id ?? null
    }

    if (!gameId) { alert('导入失败：无法创建游戏记录'); importing.value = false; return }

    // Step 2: Update name and category
    await invoke('update_game', { gameId, name: editGameName.value.trim() })
    if (editCategoryId.value !== null) {
      await invoke('assign_game_category', { gameId, categoryId: editCategoryId.value })
    }

    // Step 3: Fetch GameBrain metadata (non-blocking, don't wait)
    invoke('fetch_gamebrain_metadata', { gameId }).catch(() => {})

    showEditModal.value = false
    showToast(`已导入 "${editGameName.value}"`)
  } catch (e) {
    alert('导入失败: ' + e)
  } finally {
    importing.value = false
  }
}

// Quick folder scan (kept for batch scanning)
async function doScan(path: string) {
  scanning.value = true; scanMsg.value = '正在扫描...'
  try {
    const result = await invoke<{ found_games: number; inserted_games: number }>('scan_folder', { path, categoryId: null })
    scanMsg.value = `找到 ${result.found_games} 个游戏，新增 ${result.inserted_games} 个`
  } catch (e) { scanMsg.value = `扫描失败: ${e}` }
  finally { scanning.value = false; setTimeout(() => { scanMsg.value = '' }, 4000) }
}

async function handleQuickScan() {
  const path = newFolderInput.value.trim()
  if (!path) return
  await doScan(path)
  newFolderInput.value = ''
}

async function diagnoseMigrations() {
  try { const report = await invoke<string>('diagnose_cover_migration'); console.log(report); alert('诊断报告已输出到控制台 (F12)') }
  catch (e) { alert('诊断失败: ' + e) }
}
</script>

<template>
  <div class="p-8 max-w-2xl">
    <h1 class="text-2xl font-bold text-text-primary mb-8">设置</h1>

    <section class="mb-10">
      <h2 class="text-lg font-semibold text-text-primary mb-4">扫描文件夹</h2>
      <div class="flex items-center gap-2 mb-3">
        <input v-model="newFolderInput" type="text" placeholder="游戏路径，例如 C:\Games" class="flex-1 h-10 px-4 rounded-lg bg-card border border-border text-sm text-text-primary placeholder:text-text-secondary/50 focus:outline-none focus:border-accent-blue/50 transition-colors" @keyup.enter="handleScanPath" />
        <button class="flex items-center gap-1.5 h-10 px-4 rounded-lg bg-card border border-border text-sm text-text-secondary hover:text-text-primary hover:border-accent-blue/40 transition-colors flex-shrink-0" @click="browseFile"><FolderPlus class="w-4 h-4" /> 浏览...</button>
        <button class="flex items-center gap-1.5 h-10 px-4 rounded-lg bg-accent-blue text-white text-sm font-medium hover:brightness-110 transition-all active:scale-95 flex-shrink-0" :disabled="scanning" @click="handleQuickScan">{{ scanning ? '扫描中...' : '扫描' }}</button>
      </div>
      <div v-if="scanning" class="flex items-center gap-2 text-sm text-accent-blue mt-2"><div class="w-4 h-4 border-2 border-accent-blue border-t-transparent rounded-full animate-spin" /> {{ scanMsg }}</div>
      <p v-else-if="scanMsg" class="text-sm text-text-secondary mt-2">{{ scanMsg }}</p>
    </section>

    <section class="mb-10">
      <h2 class="text-lg font-semibold text-text-primary mb-4">SteamGridDB</h2>
      <p class="text-sm text-text-secondary mb-3">配置 SteamGridDB API Key 以获取高清游戏封面。在 <a href="https://www.steamgriddb.com/profile/preferences/api" target="_blank" class="text-accent-blue hover:underline">steamgriddb.com</a> 免费获取。</p>
      <div class="flex items-center gap-2"><input v-model="sgdbApiKey" type="password" placeholder="输入 SteamGridDB API Key..." class="flex-1 h-10 px-4 rounded-lg bg-card border border-border text-sm text-text-primary placeholder:text-text-secondary/50 focus:outline-none focus:border-accent-blue/50 transition-colors" /><Key class="w-4 h-4 flex-shrink-0" :class="saving ? 'text-accent-blue animate-pulse' : 'text-text-secondary'" /></div>
    </section>

    <section class="mb-10">
      <h2 class="text-lg font-semibold text-text-primary mb-4">GameBrain</h2>
      <p class="text-sm text-text-secondary mb-3">配置 GameBrain API Key 以获取游戏详细信息。在 <a href="https://gamebrain.co/api" target="_blank" class="text-accent-blue hover:underline">gamebrain.co</a> 免费获取。</p>
      <div class="flex items-center gap-2"><input v-model="gbApiKey" type="password" placeholder="输入 GameBrain API Key..." class="flex-1 h-10 px-4 rounded-lg bg-card border border-border text-sm text-text-primary placeholder:text-text-secondary/50 focus:outline-none focus:border-accent-blue/50 transition-colors" /><Key class="w-4 h-4 flex-shrink-0" :class="saving ? 'text-accent-blue animate-pulse' : 'text-text-secondary'" /></div>
    </section>

    <section>
      <h2 class="text-lg font-semibold text-text-primary mb-3">关于</h2>
      <div class="rounded-lg bg-card border border-border p-4 flex items-start gap-3">
        <Info class="w-5 h-5 text-text-secondary flex-shrink-0 mt-0.5" />
        <div class="text-sm text-text-secondary space-y-1">
          <p>Sipeed — 极简游戏启动器</p><p>版本 v0.1.0</p><p>使用 Tauri + Vue 3 + TypeScript 构建</p>
          <button class="mt-2 text-xs text-accent-blue hover:underline" @click="diagnoseMigrations">封面迁移诊断</button>
        </div>
      </div>
    </section>
  </div>

  <!-- Edit Game Info Modal -->
  <Teleport to="body">
    <div v-if="showEditModal" class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" @click.self="!importing && (showEditModal = false)">
      <div class="w-[520px] rounded-xl bg-card border border-border shadow-xl max-h-[90vh] overflow-y-auto">
        <div class="flex items-center justify-between px-6 pt-5 pb-3">
          <h3 class="text-sm font-semibold text-text-primary">编辑游戏信息</h3>
          <button class="p-1 rounded hover:bg-card text-text-secondary hover:text-text-primary" :disabled="importing" @click="showEditModal = false"><X class="w-4 h-4" /></button>
        </div>
        <div class="px-6 pb-6 space-y-4">
          <!-- Name -->
          <div>
            <label class="text-xs text-text-secondary mb-1 block">游戏名称 <span class="text-red-400">*</span></label>
            <input v-model="editGameName" type="text" placeholder="游戏名称" class="w-full h-9 px-3 rounded-lg bg-background border border-border text-sm text-text-primary placeholder:text-text-secondary/50 focus:outline-none focus:border-accent-blue/50 transition-colors" @keyup.enter="doImport" />
          </div>

          <!-- Category -->
          <div>
            <label class="text-xs text-text-secondary mb-1 block">分类（可选）</label>
            <div class="flex flex-wrap gap-2 mb-2">
              <button class="text-xs px-3 py-1.5 rounded-full transition-colors" :class="editCategoryId === null ? 'bg-accent-blue text-white' : 'bg-card border border-border text-text-secondary hover:border-accent-blue/40'" @click="editCategoryId = null">未分类</button>
              <button v-for="cat in editCategories" :key="cat.id" class="text-xs px-3 py-1.5 rounded-full transition-colors" :class="editCategoryId === cat.id ? 'bg-accent-blue text-white' : 'bg-card border border-border text-text-secondary hover:border-accent-blue/40'" @click="editCategoryId = cat.id">{{ cat.name }}</button>
            </div>
            <template v-if="showInlineCat">
              <div class="flex items-center gap-1">
                <input v-model="inlineCatName" type="text" placeholder="新分类名..." class="h-7 px-2 rounded bg-background border border-border text-xs w-28 outline-none focus:border-accent-blue/50" @keyup.enter="doInlineCreateCat" />
                <button class="text-xs text-accent-blue hover:underline" @click="doInlineCreateCat">确定</button>
                <button class="text-xs text-text-secondary hover:text-text-primary" @click="showInlineCat = false">取消</button>
              </div>
            </template>
            <button v-else class="text-xs text-text-secondary hover:text-accent-blue" @click="showInlineCat = true"><Plus class="w-3 h-3 inline" /> 新建分类</button>
          </div>

          <!-- File path -->
          <div>
            <label class="text-xs text-text-secondary mb-1 block">启动程序路径</label>
            <input :value="editFilePath" type="text" readonly class="w-full h-9 px-3 rounded-lg bg-black/10 border border-border text-sm text-text-secondary/70 cursor-not-allowed" />
          </div>
        </div>

        <!-- Actions -->
        <div class="flex justify-end gap-2 px-6 pb-5 pt-2 border-t border-border">
          <button class="h-9 px-4 rounded-lg text-sm text-text-secondary hover:text-text-primary" :disabled="importing" @click="showEditModal = false">取消</button>
          <button class="flex items-center gap-1.5 h-9 px-4 rounded-lg bg-accent-blue text-white text-sm font-medium hover:brightness-110 transition-all disabled:opacity-50" :disabled="importing || !editFilePath || !editGameName.trim()" @click="doImport">
            <div v-if="importing" class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
            {{ importing ? '导入中...' : '确认导入' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>

  <!-- Toast -->
  <Teleport to="body">
    <div v-if="importToast" class="fixed bottom-8 left-1/2 -translate-x-1/2 z-[200] px-4 py-2 rounded-lg bg-green-500 text-white text-sm shadow-lg transition-all">{{ importToast }}</div>
  </Teleport>
</template>
