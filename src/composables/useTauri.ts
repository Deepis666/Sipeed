import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const coverCache = new Map<number, string>()
const exeIconCache = new Map<string, string>()

export function invalidateCoverCache(gameId: number) {
  coverCache.delete(gameId)
}

export function useCover(gameId: number) {
  const url = ref<string>()
  const loading = ref(false)

  async function load() {
    if (coverCache.has(gameId)) {
      console.log(`[DEBUG] useCover(${gameId}): 命中缓存`)
      url.value = coverCache.get(gameId)
      return
    }
    loading.value = true
    try {
      console.log(`[DEBUG] useCover(${gameId}): 调用 read_cover_base64`)
      const b64 = await invoke<string | null>('read_cover_base64', { gameId })
      if (b64) {
        console.log(`[DEBUG] useCover(${gameId}): read_cover_base64 返回数据, 长度=${b64.length}`)
        coverCache.set(gameId, b64)
        url.value = b64
      } else {
        console.log(`[DEBUG] useCover(${gameId}): read_cover_base64 返回 null/空`)
      }
    } catch (e) { 
      console.log(`[DEBUG] useCover(${gameId}): read_cover_base64 异常:`, e)
    }
    finally { loading.value = false }
  }

  load()
  return { url, loading }
}

/**
 * Four-tier priority cover loader:
 * 1. SteamGridDB → sgdb_cover_path
 * 2. GameBrain → icon_path  
 * 3. Exe embedded icon → extract_icon
 * 4. Fallback → placeholder
 */
export function useGameCover(gameId: number, exePath: string) {
  const url = ref<string>()
  const loading = ref(false)

  async function load() {
    loading.value = true

    // Tier 1+2: icon_path from DB (SGDB/GB prioritized by backend)
    try {
      console.log(`[DEBUG] useGameCover(${gameId}): 调用 read_cover_base64 (后端按 SGDB→GB 优先级)`)
      const b64 = await invoke<string | null>('read_cover_base64', { gameId })
      if (b64) {
        console.log(`[DEBUG] 决策: 使用 数据库封面 (SGDB或GB), 数据长度=${b64.length}`)
        url.value = b64
        loading.value = false
        return
      }
      console.log(`[DEBUG] useGameCover(${gameId}): read_cover_base64 无数据, 回退到Exe图标`)
    } catch (e) {
      console.log(`[DEBUG] useGameCover(${gameId}): read_cover_base64 异常, 回退到Exe图标:`, e)
    }

    // Tier 3: Exe embedded icon
    if (exePath) {
      try {
        if (exeIconCache.has(exePath)) {
          console.log(`[DEBUG] 决策: 使用 本地EXE图标 (缓存命中), path=${exePath}`)
          url.value = exeIconCache.get(exePath)
          loading.value = false
          return
        }
        console.log(`[DEBUG] useGameCover(${gameId}): 调用 extract_icon exePath=${exePath}`)
        const b64 = await invoke<string | null>('extract_icon', { exePath })
        if (b64) {
          console.log(`[DEBUG] 决策: 使用 本地EXE图标, 数据长度=${b64.length}`)
          exeIconCache.set(exePath, b64)
          url.value = b64
          loading.value = false
          return
        }
        console.log(`[DEBUG] useGameCover(${gameId}): extract_icon 返回空, 回退到占位图`)
      } catch (e) {
        console.log(`[DEBUG] useGameCover(${gameId}): extract_icon 异常, 回退到占位图:`, e)
      }
    } else {
      console.log(`[DEBUG] useGameCover(${gameId}): 无 exePath, 跳过Exe图标提取`)
    }

    // Tier 4: placeholder
    console.log(`[DEBUG] 决策: 使用 默认占位图 (所有来源无数据)`)
    loading.value = false
  }

  load()
  return { url, loading }
}

export interface Game {
  id: number
  name: string
  exe_path: string
  icon_path?: string
  developer?: string
  category_id?: number
  category_name?: string
  total_playtime: number
  last_played?: number
  status: number
  rating?: number
  short_description?: string
  genres?: string
  themes?: string
  play_modes?: string
  tags?: string
  steam_description?: string
  steam_image_url?: string
}

export function parseJsonArr(raw?: string): string[] {
  if (!raw) return []
  try { return JSON.parse(raw) } catch { return [] }
}

export function useGames() {
  const games = ref<Game[]>([])
  const loading = ref(false)
  const error = ref('')

  async function fetchGames() {
    loading.value = true
    error.value = ''
    try {
      games.value = await invoke<Game[]>('list_games')
    } catch (e) {
      error.value = String(e)
      games.value = []
    } finally {
      loading.value = false
    }
  }

  return { games, loading, error, fetchGames }
}

export function useRecentGames() {
  const games = ref<Game[]>([])
  const loading = ref(false)

  async function fetchRecent() {
    loading.value = true
    try {
      games.value = await invoke<Game[]>('get_recent_games', { limit: 5 })
    } finally {
      loading.value = false
    }
  }

  return { games, loading, fetchRecent }
}

export function useGameLaunch() {
  const launching = ref(false)

  async function launchGame(gameId: number) {
    launching.value = true
    try {
      await invoke('launch_game', { gameId })
    } finally {
      launching.value = false
    }
  }

  async function openGameFolder(exePath: string) {
    await invoke('open_game_folder', { exePath })
  }

  return { launching, launchGame, openGameFolder }
}
