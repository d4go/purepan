<template>
  <div class="titlebar" data-tauri-drag-region>
    <!-- 中间：品牌图标 -->
    <!-- <div class="titlebar__center" data-tauri-drag-region>
      <img src="/icon.png" alt="logo" class="titlebar__logo" />
    </div> -->

    <!-- 右侧：窗口控制按钮 -->
    <div class="titlebar__controls">
      <div class="titlebar__window-btns">
        <button
          class="titlebar__btn titlebar__btn--min"
          @click="handleMinimize"
          title="最小化"
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line x1="0" y1="5" x2="10" y2="5" stroke="currentColor" stroke-width="1" />
          </svg>
        </button>
        <button
          v-if="maximizable"
          class="titlebar__btn titlebar__btn--max"
          @click="handleToggleMaximize"
          :title="isMaximized ? '还原' : '最大化'"
        >
          <svg v-if="!isMaximized" width="10" height="10" viewBox="0 0 10 10">
            <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
          <svg v-else width="10" height="10" viewBox="0 0 10 10">
            <rect x="0.5" y="2.5" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
            <path d="M2.5 2.5 V0.5 H9.5 V7.5 H7.5" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
        </button>
        <button
          class="titlebar__btn titlebar__btn--close"
          @click="handleClose"
          :title="closeMinimizes ? '最小化到系统托盘' : '关闭'"
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" stroke-width="1" />
            <line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" stroke-width="1" />
          </svg>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'

const props = withDefaults(defineProps<{
  maximizable?: boolean
  /** 关闭按钮是否最小化窗口（默认 true）。登录页可设为 false 以真正关闭。 */
  closeMinimizes?: boolean
}>(), {
  maximizable: false,
  closeMinimizes: true,
})

const isMaximized = ref(false)
let unlisten: (() => void) | null = null

async function getWin() {
  if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) return null
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    return getCurrentWindow()
  } catch {
    return null
  }
}

async function handleMinimize() {
  const win = await getWin()
  if (win) await win.minimize()
}

async function handleToggleMaximize() {
  const win = await getWin()
  if (!win) return
  try {
    if (isMaximized.value) {
      await win.unmaximize()
    } else {
      await win.maximize()
    }
  } catch (e) {
    console.warn('[TitleBar] toggleMaximize failed:', e)
  }
}

async function handleClose() {
  const win = await getWin()
  // Always emit a native close request. Tauri applies the latest saved setting
  // and either closes the application or hides the window to the tray.
  if (win) await win.close()
}

onMounted(async () => {
  const win = await getWin()
  if (!win) return
  try {
    isMaximized.value = await win.isMaximized()
    unlisten = await win.onResized(() => {
      win.isMaximized().then(v => { isMaximized.value = v })
    })
  } catch {
    // non-tauri env
  }
})

onUnmounted(() => {
  unlisten?.()
})
</script>

<style scoped>
.titlebar {
  display: flex;
  align-items: center;
  height: 32px;
  flex-shrink: 0;
  user-select: none;
  background: var(--app-surface);
}

/* .titlebar__center {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  pointer-events: none;
} */
/* 
.titlebar__logo {
  width: 24px;
  height: 24px;
  display: block;
} */

.titlebar__controls {
  display: flex;
  align-items: stretch;
  height: 100%;
  margin-left: auto;
}

.titlebar__window-btns {
  display: flex;
  align-items: stretch;
  height: 100%;
}

.titlebar__btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--app-text-primary);
  cursor: pointer;
  transition: background 0.15s ease;
  -webkit-app-region: no-drag;
}

.titlebar__btn--min:hover,
.titlebar__btn--max:hover {
  background: var(--app-surface-hover);
}

.titlebar__btn--close:hover {
  background: #e81123;
  color: #ffffff;
}

.titlebar__btn:active {
  background: var(--app-surface-active);
}

.titlebar__btn--close:active {
  background: #f1707a;
  color: #ffffff;
}

.titlebar__btn svg {
  pointer-events: none;
}

.titlebar__window-btns .titlebar__btn {
  border-radius: 0;
}
</style>
