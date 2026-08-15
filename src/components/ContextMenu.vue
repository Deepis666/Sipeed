<script setup lang="ts">
import { ref, onMounted } from 'vue'

export interface ContextMenuItem {
  label: string
  action?: () => void
  danger?: boolean
  children?: ContextMenuItem[]
}

defineProps<{
  items: ContextMenuItem[]
  x: number
  y: number
}>()

const emit = defineEmits<{
  close: []
}>()

const el = ref<HTMLElement>()

function handleAction(item: ContextMenuItem) {
  item.action?.()
  emit('close')
}

onMounted(() => {
  setTimeout(() => {
    document.addEventListener('click', () => emit('close'), { once: true })
  }, 0)
})
</script>

<template>
  <div
    ref="el"
    class="fixed z-[100] min-w-[140px] bg-card border border-border rounded-lg shadow-xl py-1"
    :style="{ left: x + 'px', top: y + 'px' }"
  >
    <template v-for="item in items" :key="item.label">
      <div class="relative group/item">
        <button
          class="w-full text-left px-3 py-1.5 text-sm transition-colors flex items-center justify-between"
          :class="item.danger ? 'text-red-400 hover:bg-red-500/10' : 'text-text-primary hover:bg-accent-blue/10'"
          @click="item.children ? undefined : handleAction(item)"
        >
          {{ item.label }}
          <span v-if="item.children" class="text-text-secondary">▶</span>
        </button>
        <div v-if="item.children" class="absolute left-full top-0 hidden group-hover/item:block min-w-[120px] bg-card border border-border rounded-lg shadow-xl py-1">
          <button
            v-for="child in item.children"
            :key="child.label"
            class="w-full text-left px-3 py-1.5 text-sm text-text-primary hover:bg-accent-blue/10 transition-colors"
            @click="handleAction(child)"
          >
            {{ child.label }}
          </button>
        </div>
      </div>
    </template>
  </div>
</template>
