<template>
  <div class="navigator-bar">
    <!-- 导航按钮组 -->
    <div class="nav-buttons">
      <el-button
          :icon="ArrowLeft"
          :disabled="!canGoBack"
          circle
          size="small"
          @click="emit('back')"
      />
      <el-button
          :icon="ArrowRight"
          :disabled="!canGoForward"
          circle
          size="small"
          @click="emit('forward')"
      />
      <el-button
          :icon="Top"
          :disabled="!canGoUp"
          circle
          size="small"
          @click="emit('up')"
      />
      <el-button
          :icon="Refresh"
          circle
          size="small"
          @click="emit('refresh')"
      />
    </div>

    <div class="input-stage">
      <div class="path-stage">
        <div class="path-input-wrapper">
          <el-input
              v-model="inputPath"
              placeholder="输入路径并按回车跳转"
              clearable
              @keyup.enter="handleNavigate"
              @focus="isEditing = true"
              @blur="handleBlur"
          >
            <template #prefix>
              <el-icon><FolderOpened /></el-icon>
            </template>
          </el-input>
        </div>

        <div
            v-if="!isEditing && breadcrumbs.length > 0"
            class="breadcrumb-overlay"
            @click="focusInput()"
        >
          <!-- 路径过长时显示省略号 -->
          <span v-if="breadcrumbs.length > 4" class="ellipsis-indicator">..</span>
          
          <el-breadcrumb separator="/">
            <!-- 只显示最后3个面包屑，前面用 .. 表示 -->
            <template v-if="breadcrumbs.length <= 4">
              <el-breadcrumb-item
                  v-for="(crumb, index) in breadcrumbs"
                  :key="index"
                  @click.stop="handleCrumbClick(crumb.path)"
              >
                <span class="crumb-item" :class="{ 'is-current': index === breadcrumbs.length - 1 }">
                  {{ crumb.name }}
                </span>
              </el-breadcrumb-item>
            </template>
            <template v-else>
              <el-breadcrumb-item @click.stop="handleCrumbClick('/')">
                <span class="crumb-item ellipsis-crumb">..</span>
              </el-breadcrumb-item>
              <el-breadcrumb-item
                  v-for="(crumb, index) in breadcrumbs.slice(breadcrumbs.length - 3)"
                  :key="'last-' + index"
                  @click.stop="handleCrumbClick(crumb.path)"
              >
                <span class="crumb-item" :class="{ 'is-current': index === 2 }">
                  {{ crumb.name }}
                </span>
              </el-breadcrumb-item>
            </template>
          </el-breadcrumb>
        </div>
      </div>

      <div
          class="search-box"
          :class="{ 'is-expanded': isSearchExpanded, 'has-text': !!searchInput.trim() }"
      >
        <button
            type="button"
            class="search-action"
            aria-label="搜索"
            @click="handleSearchAction"
        >
          <el-icon><Search /></el-icon>
        </button>
        <el-input
            ref="searchInputRef"
            v-model="searchInput"
            placeholder="搜索当前目录..."
            clearable
            @keyup.enter="handleSearch"
            @keyup.esc="handleSearchEscape"
            @clear="handleClearSearch"
            @blur="handleSearchBlur"
            size="small"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'
import { ArrowLeft, ArrowRight, Top, Refresh, FolderOpened, Search } from '@element-plus/icons-vue'

const props = defineProps<{
  currentPath: string
  canGoBack: boolean
  canGoForward: boolean
  canGoUp: boolean
  mode?: 'upload' | 'download' | 'select-directory'
}>()

const emit = defineEmits<{
  'navigate': [path: string]
  'back': []
  'forward': []
  'up': []
  'refresh': []
  'search': [keyword: string]
  'clear-search': []
}>()

const inputPath = ref('')
const isEditing = ref(false)
const searchInput = ref('')
const searchInputRef = ref<InstanceType<typeof import('element-plus')['ElInput']> | null>(null)
const isSearchExpanded = ref(props.mode !== 'upload')

// 面包屑数据
const breadcrumbs = computed(() => {
  if (!props.currentPath) {
    return [{ name: '计算机', path: '' }]
  }

  const parts: { name: string; path: string }[] = []

  // 处理 Windows 路径 (C:\xxx) 和 Unix 路径 (/xxx)
  const path = props.currentPath

  // 检测是否是 Windows 驱动器路径
  const isWindowsPath = /^[A-Za-z]:/.test(path)

  if (isWindowsPath) {
    // Windows 路径
    const driveLetter = path.substring(0, 2) // C:
    parts.push({ name: '计算机', path: '' })
    parts.push({ name: driveLetter, path: driveLetter })

    const restPath = path.substring(3) // 去掉 C:\
    if (restPath) {
      const segments = restPath.split(/[\\/]/).filter(Boolean)
      let currentPath = driveLetter
      for (const segment of segments) {
        currentPath += '\\' + segment
        parts.push({ name: segment, path: currentPath })
      }
    }
  } else {
    // Unix 路径
    parts.push({ name: '根目录', path: '/' })

    const segments = path.split('/').filter(Boolean)
    let currentPath = ''
    for (const segment of segments) {
      currentPath += '/' + segment
      parts.push({ name: segment, path: currentPath })
    }
  }

  return parts
})

// 同步路径到输入框
watch(() => props.currentPath, (newPath) => {
  inputPath.value = newPath
}, { immediate: true })

// 处理导航
function handleNavigate() {
  const path = inputPath.value.trim()
  if (path !== props.currentPath) {
    emit('navigate', path)
  }
  isEditing.value = false
}

// 处理面包屑点击
function handleCrumbClick(path: string) {
  // 空路径（Windows 的"计算机"根目录）会触发 navigate 事件
  // 父组件会处理空路径，直接调用 navigateTo('') 而不是 jumpToPath('')
  if (path !== props.currentPath) {
    emit('navigate', path)
  }
}

// 聚焦输入框
function focusInput() {
  isEditing.value = true
  nextTick(() => {
    const input = document.querySelector('.path-input-wrapper input') as HTMLInputElement
    input?.focus()
    input?.select()
  })
}

// 处理失焦
function handleBlur() {
  // 延迟关闭编辑状态，以便点击事件能够触发
  setTimeout(() => {
    isEditing.value = false
    inputPath.value = props.currentPath
  }, 150)
}

function handleSearchAction() {
  if (!isSearchExpanded.value) {
    isSearchExpanded.value = true
    nextTick(() => {
      searchInputRef.value?.focus()
    })
    return
  }

  const keyword = searchInput.value.trim()
  if (keyword) {
    emit('search', keyword)
    return
  }

  searchInputRef.value?.focus()
}

function handleSearchBlur() {
  setTimeout(() => {
    if (!searchInput.value.trim()) {
      isSearchExpanded.value = false
    }
  }, 200)
}

// 执行搜索
function handleSearch() {
  const keyword = searchInput.value.trim()
  if (keyword) {
    emit('search', keyword)
  } else {
    emit('clear-search')
  }
}

// 清除搜索
function handleClearSearch() {
  searchInput.value = ''
  emit('clear-search')
}

function handleSearchEscape() {
  if (!searchInput.value.trim()) {
    searchInputRef.value?.blur()
    return
  }

  handleClearSearch()
}

// 导航时关闭搜索
watch(() => props.currentPath, () => {
  if (searchInput.value.trim()) {
    searchInput.value = ''
    // 不再 emit clear-search：store 层在 navigateTo/goBack/goForward 中已自行清理搜索并加载目录
  }
})
</script>

<style scoped>
.navigator-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 0;
  position: relative;
}

.nav-buttons {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.input-stage {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 1;
  min-width: 0;
  min-height: 32px;
}

.path-stage {
  position: relative;
  flex: 1;
  min-width: 0;
}

.path-stage :deep(.el-input),
.path-input-wrapper :deep(.el-input) {
  width: 100%;
}

.path-input-wrapper {
  width: 100%;
  position: relative;
}

.search-box {
  width: 32px;
  min-width: 32px;
  max-width: 160px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 0;
  border-radius: 10px;
  background: var(--app-surface-hover);
  box-shadow: inset 0 0 0 1px rgba(148, 163, 184, 0.12);
  transition: width 0.2s ease, padding 0.2s ease, box-shadow 0.2s ease, background-color 0.2s ease;
  overflow: hidden;
  cursor: pointer;
  flex-shrink: 0;
}

.search-box.is-expanded {
  width: 160px;
  padding: 2px 8px 2px 4px;
  justify-content: flex-start;
  cursor: default;
}

.search-box.has-text {
  background: var(--app-accent-bg);
  box-shadow: inset 0 0 0 1px rgba(64, 158, 255, 0.18);
}

.search-box:focus-within {
  background: var(--app-surface);
  box-shadow:
      inset 0 0 0 1px rgba(64, 158, 255, 0.24),
      0 0 0 3px rgba(64, 158, 255, 0.08);
}

.search-action {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--app-text-tertiary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  cursor: pointer;
  transition: color 0.2s ease, background-color 0.2s ease;
}

.search-action:hover {
  color: var(--el-color-primary);
  background: rgba(64, 158, 255, 0.07);
}

.search-box :deep(.el-input) {
  flex: 1;
  width: 0;
  opacity: 0;
  transition: opacity 0.15s ease;
  pointer-events: none;
}

.search-box.is-expanded :deep(.el-input) {
  width: 100%;
  opacity: 1;
  pointer-events: auto;
}

.search-box :deep(.el-input__wrapper) {
  box-shadow: none;
  background: transparent;
  padding: 0;
}

.search-box :deep(.el-input__inner) {
  font-size: 12px;
  color: var(--app-text-primary);
}

.search-box :deep(.el-input__inner::placeholder) {
  color: var(--app-text-tertiary);
}

.breadcrumb-overlay {
  position: absolute;
  left: 0;
  right: 0;
  top: 50%;
  transform: translateY(-50%);
  background: var(--el-fill-color-blank);
  padding: 0 12px 0 32px;
  height: 30px;
  display: flex;
  align-items: center;
  border-radius: 4px;
  cursor: text;
  overflow: hidden;
  z-index: 1; /* 确保在输入框上方 */

  .ellipsis-indicator {
    font-size: 13px;
    color: var(--app-text-tertiary);
    margin-right: 4px;
    flex-shrink: 0;
  }
}

.breadcrumb-overlay :deep(.el-breadcrumb) {
  font-size: 13px;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.crumb-item {
  cursor: pointer;
  color: var(--el-text-color-regular);
  transition: color 0.2s;
  padding: 2px 4px; /* 增加点击区域 */
  border-radius: 4px;
  display: inline-block;
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.crumb-item:hover {
  color: var(--el-color-primary);
  background-color: var(--el-fill-color-light); /* 悬停背景 */
}

.crumb-item.is-current {
  color: var(--el-text-color-primary);
  font-weight: 500;
}

.crumb-item.ellipsis-crumb {
  color: var(--app-text-tertiary);
  font-weight: normal;
}

:deep(.el-dialog) {
  border-radius: 14px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);

  .el-dialog__header {
    border-bottom: 1px solid var(--app-divider);
  }

  .el-dialog__footer {
    border-top: 1px solid var(--app-divider);
  }
}
</style>
