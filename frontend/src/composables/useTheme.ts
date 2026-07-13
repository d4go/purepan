/**
 * 主题管理 Composable
 *
 * 支持 'auto' | 'light' | 'dark' 三种模式：
 * - auto: 跟随系统 prefers-color-scheme，系统切换时自动响应
 * - light: 强制浅色
 * - dark: 强制深色
 *
 * 通过 document.documentElement 上的 data-theme 属性和 .dark class 控制主题。
 * - .dark class: 触发 Element Plus 暗色模式 CSS
 * - data-theme="light": 覆盖 prefers-color-scheme: dark 的自动行为
 * - data-theme="dark": 在 light 系统下强制暗色
 *
 * 用户选择持久化到 localStorage。
 */

import { ref, watch, onMounted } from 'vue'

export type ThemeMode = 'auto' | 'light' | 'dark'

const STORAGE_KEY = 'app-theme-mode'
const themeMode = ref<ThemeMode>('auto')
let initialized = false

/** 获取系统当前是否为暗色模式 */
function getSystemDark(): boolean {
  return window.matchMedia('(prefers-color-scheme: dark)').matches
}

/** 根据模式 + 系统偏好，决定实际是否使用暗色 */
function resolveDark(mode: ThemeMode): boolean {
  if (mode === 'dark') return true
  if (mode === 'light') return false
  return getSystemDark()
}

/** 将主题应用到 DOM */
function applyTheme(mode: ThemeMode) {
  const isDark = resolveDark(mode)
  const root = document.documentElement

  if (isDark) {
    root.classList.add('dark')
    root.setAttribute('data-theme', 'dark')
  } else if (mode === 'light') {
    root.classList.remove('dark')
    root.setAttribute('data-theme', 'light')
  } else {
    // auto + light system: 移除 dark class 和 data-theme
    root.classList.remove('dark')
    root.removeAttribute('data-theme')
  }
}

/** 初始化主题（仅执行一次，幂等） */
export function initTheme() {
  if (initialized) return
  initialized = true

  // 从 localStorage 读取用户选择
  const stored = localStorage.getItem(STORAGE_KEY) as ThemeMode | null
  if (stored && ['auto', 'light', 'dark'].includes(stored)) {
    themeMode.value = stored
  }

  applyTheme(themeMode.value)

  // 监听系统主题变化（仅 auto 模式下生效）
  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
  const handleSystemChange = () => {
    if (themeMode.value === 'auto') {
      applyTheme('auto')
    }
  }
  mediaQuery.addEventListener('change', handleSystemChange)

  // 监听 themeMode 变化
  watch(themeMode, (newMode) => {
    localStorage.setItem(STORAGE_KEY, newMode)
    applyTheme(newMode)
  })
}

/** 设置主题模式 */
function setThemeMode(mode: ThemeMode) {
  themeMode.value = mode
}

/** 获取当前实际是否为暗色（响应式） */
function isDark() {
  return resolveDark(themeMode.value)
}

export function useTheme() {
  onMounted(() => {
    initTheme()
  })

  return {
    themeMode,
    setThemeMode,
    isDark,
    initTheme,
  }
}
