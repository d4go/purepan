<template>
  <div class="files-container">
    <!-- 客户端预热遮罩：登录后后端 client 初始化中，自动重试直到就绪 -->
    <div v-if="clientWarmingUp" class="client-warmup-mask">
      <el-icon class="is-loading" :size="36"><Loading /></el-icon>
      <p class="warmup-text">正在初始化客户端...</p>
    </div>

    <!-- 桌面端工具栏 -->
    <div class="toolbar">
      <div class="toolbar-left">
        <!-- 面包屑导航 -->
        <el-breadcrumb separator="/" class="breadcrumb">
          <el-breadcrumb-item @click="navigateToDir('/')">
            <el-icon><HomeFilled/></el-icon>
            <span class="breadcrumb-text">根目录</span>
          </el-breadcrumb-item>
          <el-breadcrumb-item
              v-for="(part, index) in pathParts"
              :key="index"
              @click="navigateToDir(getPathUpTo(index))"
          >
            <span class="breadcrumb-text" :title="part">{{ part }}</span>
          </el-breadcrumb-item>
        </el-breadcrumb>
      </div>

      <div class="toolbar-center">
        <!-- 搜索框 -->
        <div class="search-box">
          <el-icon class="search-icon"><Search /></el-icon>
          <input
              ref="searchInputRef"
              v-model="searchKeyword"
              type="text"
              placeholder="搜索"
              @keyup.enter="handleSearch"
              @keyup.esc="handleSearchEscape"
          />
          <button v-if="searchKeyword" class="clear-btn" @click="handleSearchClear">
            <el-icon><Close /></el-icon>
          </button>
        </div>
      </div>

      <div class="toolbar-right">
        <!-- 选中状态：批量操作按钮 -->
        <template v-if="selectedFiles.length > 0">
          <el-button @click="handleBatchDownload">
            <el-icon><Download /></el-icon>
            下载
          </el-button>
          <el-button @click="handleBatchCopy">
            <el-icon><CopyDocument /></el-icon>
            复制
          </el-button>
          <el-button @click="handleBatchMove">
            <el-icon><Rank /></el-icon>
            移动
          </el-button>
          <el-button @click="handleBatchShare">
            <el-icon><Link /></el-icon>
            分享
          </el-button>
        </template>

        <!-- 未选中状态：主要操作按钮 -->
        <template v-else>
          <el-button type="primary" @click="showCreateFolderDialog">
          <el-icon><FolderAdd /></el-icon>
          新建
        </el-button>
        <el-button type="success" @click="showFilePicker = true">
          <el-icon><Upload /></el-icon>
          上传
        </el-button>
        <el-dropdown trigger="click">
          <el-button type="warning">
            <el-icon><Share /></el-icon>
            更多
            <el-icon class="el-icon--right"><ArrowDown /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item @click="showTransferDialog = true">
                <el-icon><Share /></el-icon>
                转存
              </el-dropdown-item>
              <el-dropdown-item @click="showShareDirectDownloadDialog = true">
                <el-icon><Download /></el-icon>
                分享直下
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-button @click="refreshFileList">
          <el-icon><Refresh /></el-icon>
          刷新
        </el-button>
        </template>
      </div>
    </div>
    
    <!-- FilePicker 文件选择器弹窗 -->
    <FilePickerModal
        v-model="showFilePicker"
        :select-type="'both'"
        :title="'选择上传文件'"
        :confirm-text="'上传'"
        :multiple="true"
        :initial-path="uploadConfig?.recent_directory"
        :show-encryption="hasEncryptionKey"
        :show-conflict-strategy="true"
        :default-upload-conflict-strategy="uploadConflictStrategy"
        @select="handleFilePickerSelect"
        @select-multiple="handleFilePickerMultiSelect"
    />

    <!-- 文件列表 -->
    <div class="file-list" ref="fileListRef" @scroll="handleScroll">
      <el-table
          v-loading="loading"
          :data="fileList"
          :key="currentDir"
          style="width: 100%"
          @row-click="handleRowClick"
          @row-contextmenu="handleRowContextMenu"
          @selection-change="handleSelectionChange"
          :row-class-name="getRowClassName"
          highlight-current-row
      >
        <el-table-column type="selection" width="48" align="center" />
        
        <el-table-column label="文件名" min-width="320" sortable>
          <template #default="{ row }">
            <div class="file-name-cell">
              <el-icon :size="20" class="file-icon" :class="{ 'folder-icon': row.isdir === 1 }">
                <Folder v-if="row.isdir === 1"/>
                <Document v-else/>
              </el-icon>
              <span class="filename-text" :title="getDisplayName(row)">{{ getDisplayName(row) }}</span>
              <el-tag v-if="row.is_encrypted || row.is_encrypted_folder" type="warning" size="small" class="encrypted-tag">
                🔒
              </el-tag>
            </div>
          </template>
        </el-table-column>

        <el-table-column label="大小" width="100" align="right" sortable>
          <template #default="{ row }">
            <span v-if="row.isdir === 0" class="size-text">{{ formatFileSize(row.size) }}</span>
            <span v-else class="folder-text">-</span>
          </template>
        </el-table-column>

        <el-table-column label="修改时间" width="160" sortable>
          <template #default="{ row }">
            <span class="time-text">{{ formatTime(row.server_mtime) }}</span>
          </template>
        </el-table-column>

        <el-table-column label="类型" width="100">
          <template #default="{ row }">
            <span class="type-text">{{ row.isdir === 1 ? '文件夹' : getFileType(row.server_filename) }}</span>
          </template>
        </el-table-column>

        <template #empty>
          <div @contextmenu.prevent></div>
        </template>
      </el-table>

      <!-- 加载状态 -->
      <div v-if="loadingMore" class="loading-indicator">
        <el-icon class="is-loading"><Loading /></el-icon>
        <span>加载中...</span>
      </div>
      <div v-else-if="!(isSearchMode ? searchHasMore : hasMore) && fileList.length > 0" class="end-indicator">
        已显示全部 {{ fileList.length }} 项
      </div>

      <!-- 空状态 -->
      <el-empty v-if="!loading && !searchLoading && fileList.length === 0" :description="isSearchMode ? '未找到匹配的文件' : '当前目录为空'"/>
    </div>

    <!-- 底部状态栏 -->
    <div class="status-bar">
      <div class="status-left">
        <span class="item-count">{{ fileList.length }} 项</span>
        <span v-if="selectedFiles.length > 0" class="selection-info">
          · 已选择 {{ selectedFiles.length }} 项
        </span>
      </div>
      <div class="status-right">
        <span v-if="isSearchMode" class="search-status">搜索结果: "{{ searchKeyword }}"</span>
        <span class="path-display">{{ currentDir }}</span>
      </div>
    </div>

    <!-- 创建文件夹对话框 -->
    <el-dialog
        v-model="createFolderDialogVisible"
        title="新建文件夹"
        width="500px"
        @close="handleDialogClose"
    >
      <el-form :model="createFolderForm" label-width="80px">
        <el-form-item label="文件夹名">
          <el-input
              v-model="createFolderForm.folderName"
              placeholder="请输入文件夹名称"
              @keyup.enter="handleCreateFolder"
              autofocus
          />
        </el-form-item>
        <el-form-item label="当前路径">
          <el-text>{{ currentDir }}</el-text>
        </el-form-item>
      </el-form>
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="createFolderDialogVisible = false">取消</el-button>
          <el-button
              type="primary"
              :loading="creatingFolder"
              @click="handleCreateFolder"
          >
            创建
          </el-button>
        </span>
      </template>
    </el-dialog>

    <!-- 下载目录选择弹窗 -->
    <FilePickerModal
        v-model="showDownloadPicker"
        mode="download"
        select-type="directory"
        title="选择下载目录"
        :initial-path="downloadConfig?.recent_directory || downloadConfig?.default_directory || downloadConfig?.download_dir"
        :default-download-dir="downloadConfig?.default_directory || downloadConfig?.download_dir"
        :show-conflict-strategy="true"
        :default-conflict-strategy="downloadConflictStrategy"
        @confirm-download="handleConfirmDownload"
        @use-default="handleUseDefaultDownload"
    />

    <!-- 转存对话框 -->
    <TransferDialog
        v-model="showTransferDialog"
        :current-path="currentDir"
        @success="handleTransferSuccess"
    />

    <!-- 分享对话框 -->
    <ShareDialog
        v-model="showShareDialog"
        :files="shareFiles"
        @success="handleShareSuccess"
    />

    <!-- 分享直下对话框 -->
    <ShareDirectDownloadDialog
        v-model="showShareDirectDownloadDialog"
        @success="handleShareDirectDownloadSuccess"
    />

    <!-- 网盘文件夹选择器（用于复制 / 移动） -->
    <NetdiskFolderPickerModal
        v-model="showFolderPicker"
        :title="folderPickerTitle"
        :initial-path="folderPickerInitialPath"
        :blocked-paths="folderPickerBlockedPaths"
        :blocked-exact-paths="folderPickerBlockedExactPaths"
        @confirm="handleFolderPicked"
    />

    <!-- 重命名对话框 -->
    <el-dialog
        v-model="renameDialogVisible"
        title="重命名"
        width="500px"
        @close="resetRenameDialog"
    >
      <el-form label-width="80px">
        <el-form-item label="原名称">
          <el-text>{{ renameTarget?.server_filename }}</el-text>
        </el-form-item>
        <el-form-item label="新名称" :error="renameNameError ?? undefined">
          <el-input
              v-model="renameNewName"
              placeholder="输入新名称"
              autofocus
              @keyup.enter="handleRenameConfirm"
          />
        </el-form-item>
        <el-form-item label="所在目录">
          <el-text>{{ renameTarget ? dirname(renameTarget.path) : '-' }}</el-text>
        </el-form-item>
      </el-form>
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="renameDialogVisible = false">取消</el-button>
          <el-button
              type="primary"
              :loading="renameSubmitting"
              :disabled="!!renameNameError"
              @click="handleRenameConfirm"
          >确定</el-button>
        </span>
      </template>
    </el-dialog>

    <!-- 右键上下文菜单 -->
    <div
        v-if="contextMenuVisible"
        class="context-menu"
        :style="contextMenuStyle"
        @click.stop
    >
      <div class="context-menu-header" v-if="contextMenuTarget">
        <el-icon :size="14"><Document v-if="contextMenuTarget.isdir === 0"/><Folder v-else/></el-icon>
        <span class="truncate">{{ getDisplayName(contextMenuTarget) }}</span>
      </div>
      <div class="context-menu-divider"></div>
      <div class="context-menu-item" @click="handleContextMenuAction('open')">
        <el-icon><TopRight /></el-icon>
        <span>打开</span>
      </div>
      <div class="context-menu-item" @click="handleContextMenuAction('download')">
        <el-icon><Download /></el-icon>
        <span>下载</span>
      </div>
      <div class="context-menu-item" @click="handleContextMenuAction('share')">
        <el-icon><Share /></el-icon>
        <span>分享</span>
      </div>
      <div class="context-menu-item" @click="handleContextMenuAction('rename')">
        <el-icon><Edit /></el-icon>
        <span>重命名</span>
      </div>
      <div class="context-menu-divider"></div>
      <div class="context-menu-item" @click="handleContextMenuAction('copy')">
        <el-icon><CopyDocument /></el-icon>
        <span>复制到...</span>
      </div>
      <div class="context-menu-item" @click="handleContextMenuAction('move')">
        <el-icon><Rank /></el-icon>
        <span>移动到...</span>
      </div>
      <div class="context-menu-divider"></div>
      <div class="context-menu-item danger" @click="handleContextMenuAction('delete')">
        <el-icon><Delete /></el-icon>
        <span>删除</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {ref, onMounted, onBeforeUnmount, computed, nextTick} from 'vue'
import {ElMessage, ElMessageBox} from 'element-plus'
import type {InputInstance} from 'element-plus'
import {
  getFileList,
  searchFiles,
  formatFileSize,
  formatTime,
  createFolder,
  deleteFiles,
  copyFiles,
  moveFiles,
  renameFile,
  validateFilename,
  joinPath,
  basename,
  dirname,
  type FileItem,
  type FileOperationItem,
  type FileOperationOutcomeDto,
} from '@/api/file'
import NetdiskFolderPickerModal from '@/components/NetdiskFolderPickerModal.vue'
import {createDownload, createFolderDownload, createBatchDownload, type BatchDownloadItem, type DownloadConflictStrategy} from '@/api/download'
import {createUpload, createFolderUpload, type UploadConflictStrategy} from '@/api/upload'
import {getConfig, updateRecentDirDebounced, setDefaultDownloadDir, type DownloadConfig, type UploadConfig} from '@/api/config'
import {getEncryptionStatus} from '@/api/autobackup'
import {FilePickerModal} from '@/components/FilePicker'
import TransferDialog from '@/components/TransferDialog.vue'
import ShareDialog from '@/components/ShareDialog.vue'
import ShareDirectDownloadDialog from '@/components/ShareDirectDownloadDialog.vue'
import type {FileEntry} from '@/api/filesystem'

// 下载配置状态
const downloadConfig = ref<DownloadConfig | null>(null)

// 上传配置状态
const uploadConfig = ref<UploadConfig | null>(null)

// 冲突策略状态
const uploadConflictStrategy = ref<UploadConflictStrategy>('smart_dedup')
const downloadConflictStrategy = ref<DownloadConflictStrategy>('overwrite')

// 加密密钥状态
const hasEncryptionKey = ref(false)

// 状态
const loading = ref(false)
const loadingMore = ref(false)
const fileList = ref<FileItem[]>([])
const currentDir = ref('/')
const currentPage = ref(1)
const hasMore = ref(true)
const fileListRef = ref<HTMLElement | null>(null)

// 客户端预热中（登录后首次进入首页，后端 client 仍在初始化）
const clientWarmingUp = ref(false)
let warmupRetryTimer: ReturnType<typeof setTimeout> | null = null
let warmupRetryCount = 0
const MAX_WARMUP_RETRIES = 120 // 最多重试 60 秒（500ms × 120），超时后回退到错误提示
const downloadingFolders = ref<Set<string>>(new Set())
const createFolderDialogVisible = ref(false)
const creatingFolder = ref(false)
const createFolderForm = ref({
  folderName: ''
})

// FilePicker 状态
const showFilePicker = ref(false)

// 批量选择状态
const selectedFiles = ref<FileItem[]>([])
const showDownloadPicker = ref(false)
const batchDownloading = ref(false)
const batchDeleting = ref(false)

// 批量复制 / 批量移动 / 重命名 状态
const batchCopying = ref(false)
const batchMoving = ref(false)
const showFolderPicker = ref(false)
const folderPickerOperation = ref<'copy' | 'move'>('copy')
const folderPickerTitle = computed(() =>
    folderPickerOperation.value === 'copy' ? '选择复制目标文件夹' : '选择移动目标文件夹'
)
const folderPickerBlockedPaths = ref<string[]>([])      // 子树禁选（含子目录）
const folderPickerBlockedExactPaths = ref<string[]>([]) // 精确禁选（仅自身）
const folderPickerInitialPath = ref<string>('/')

// 重命名对话框
const renameDialogVisible = ref(false)
const renameTarget = ref<FileItem | null>(null)
const renameNewName = ref('')
const renameSubmitting = ref(false)
const renameNameError = computed(() => validateFilename(renameNewName.value))

// 单文件下载（支持 ask_each_time）
const pendingDownloadFile = ref<FileItem | null>(null)

// 转存对话框状态
const showTransferDialog = ref(false)

// 分享对话框状态
const showShareDialog = ref(false)
const shareFiles = ref<FileItem[]>([])

// 分享直下对话框状态
const showShareDirectDownloadDialog = ref(false)

// 搜索状态
const searchExpanded = ref(false)
const searchKeyword = ref('')
const searchLoading = ref(false)
const isSearchMode = ref(false)
const searchPage = ref(1)
const searchHasMore = ref(false)
const searchInputRef = ref<InputInstance>()
const searchWrapperRef = ref<HTMLElement | null>(null)

// 右键菜单状态
const contextMenuVisible = ref(false)
const contextMenuTarget = ref<FileItem | null>(null)
const contextMenuPosition = ref({ x: 0, y: 0 })
const contextMenuStyle = computed(() => ({
  left: `${contextMenuPosition.value.x}px`,
  top: `${contextMenuPosition.value.y}px`
}))

// 请求版本号，用于取消过期的请求回调
let fileRequestVersion = 0

// 路径分割
const pathParts = computed(() => {
  if (currentDir.value === '/') return []
  return currentDir.value.split('/').filter(p => p)
})

// 获取指定深度的路径
function getPathUpTo(index: number): string {
  const parts = pathParts.value.slice(0, index + 1)
  return '/' + parts.join('/')
}

// 加载文件列表
async function loadFiles(dir: string, append: boolean = false) {
  // 目录切换请求进行中时禁止旧目录触发滚动分页。否则 currentDir 尚未更新，
  // 根目录的下一页请求会递增 fileRequestVersion，并把刚发出的目录请求判为过期。
  if (append && (loading.value || loadingMore.value || !hasMore.value)) return
  const version = ++fileRequestVersion

  if (append) {
    loadingMore.value = true
  } else {
    loading.value = true
    currentPage.value = 1
    hasMore.value = true
  }

  // 追加时请求下一页；页码只在成功后前进，避免失败时无限累加
  const requestPage = append ? currentPage.value + 1 : 1

  try {
    const data = await getFileList(dir, requestPage, 50)
    if (version !== fileRequestVersion) return

    // 成功获取数据，关闭预热遮罩并重置计数
    clientWarmingUp.value = false
    warmupRetryCount = 0

    if (append) {
      fileList.value = [...fileList.value, ...data.list]
    } else {
      fileList.value = data.list
      currentDir.value = dir
    }

    hasMore.value = data.has_more
    currentPage.value = data.page
  } catch (error: any) {
    if (version !== fileRequestVersion) return
    // 失败时停止继续加载，避免滚动加载在出错后不断 page+1 重发造成死循环
    hasMore.value = false

    const msg = error?.message || ''
    // 登录后后端 client 预热未完成 → 显示遮罩 loading 并每 500ms 自动重试，直到就绪
    if (!append && msg.includes('未登录或客户端未初始化')) {
      clientWarmingUp.value = true
      warmupRetryCount++
      if (warmupRetryCount > MAX_WARMUP_RETRIES) {
        clientWarmingUp.value = false
        warmupRetryCount = 0
        ElMessage.error('客户端初始化超时，请稍后手动刷新重试')
      } else {
        scheduleWarmupRetry(dir)
      }
      return
    }

    ElMessage.error(msg || '加载文件列表失败')
    console.error('加载文件列表失败:', error)
  } finally {
    if (version === fileRequestVersion) {
      loading.value = false
      loadingMore.value = false
    }
  }
}

// 客户端预热重试：每 500ms 重新拉取文件列表，相当于自动点击「刷新」，直到后端 client 就绪
function scheduleWarmupRetry(dir: string) {
  if (warmupRetryTimer) clearTimeout(warmupRetryTimer)
  warmupRetryTimer = setTimeout(() => {
    warmupRetryTimer = null
    loadFiles(dir)
  }, 500)
}

// 加载下一页
async function loadNextPage() {
  if (loading.value || loadingMore.value || !hasMore.value) return
  await loadFiles(currentDir.value, true)
}

// 滚动事件处理
function handleScroll(event: Event) {
  const target = event.target as HTMLElement
  const { scrollTop, scrollHeight, clientHeight } = target

  // 当滚动到距离底部 100px 时加载更多
  if (scrollHeight - scrollTop - clientHeight < 100) {
    if (isSearchMode.value) {
      loadMoreSearchResults()
    } else {
      loadNextPage()
    }
  }
}

// 导航到目录
function navigateToDir(dir: string) {
  if (isSearchMode.value) {
    resetSearchState()
  }
  loadFiles(dir)
}

// 刷新文件列表
function refreshFileList() {
  if (isSearchMode.value) {
    resetSearchState()
  }
  loadFiles(currentDir.value)
}

// 行点击事件
function handleRowClick(row: FileItem) {
  if (row.isdir === 1) {
    // 进入目录
    navigateToDir(row.path)
  }
}

// 右键菜单事件
function handleRowContextMenu(row: FileItem, _column: unknown, event: MouseEvent) {
  event.preventDefault()
  event.stopPropagation()
  contextMenuTarget.value = row
  // 估算菜单高度（7个菜单项 ≈ 300px），当靠近底部时向上弹出
  const menuHeight = 300
  const x = event.clientX
  const y = event.clientY
  let top = y
  if (y + menuHeight > window.innerHeight) {
    top = Math.max(8, y - menuHeight)
  }
  contextMenuPosition.value = { x, y: top }
  contextMenuVisible.value = true
}

// 关闭右键菜单
function closeContextMenu() {
  contextMenuVisible.value = false
  contextMenuTarget.value = null
}

// 处理右键菜单动作
async function handleContextMenuAction(action: string) {
  const target = contextMenuTarget.value
  if (!target) return

  closeContextMenu()

  switch (action) {
    case 'open':
      if (target.isdir === 1) {
        navigateToDir(target.path)
      } else {
        handleDownload(target)
      }
      break
    case 'download':
      if (target.isdir === 1) {
        await handleDownloadFolder(target)
      } else {
        await handleDownload(target)
      }
      break
    case 'share':
      handleSingleShare(target)
      break
    case 'rename':
      handleSingleRename(target)
      break
    case 'copy':
      selectedFiles.value = [target]
      handleBatchCopy()
      break
    case 'move':
      selectedFiles.value = [target]
      handleBatchMove()
      break
    case 'delete':
      selectedFiles.value = [target]
      handleBatchDelete()
      break
  }
}

// 行样式
function getRowClassName({row}: { row: FileItem }) {
  return row.isdir === 1 ? 'directory-row' : ''
}

// 获取文件显示名称（加密文件/文件夹显示原始名称）
function getDisplayName(file: FileItem): string {
  if ((file.is_encrypted || file.is_encrypted_folder) && file.original_name) {
    return file.original_name
  }
  return file.server_filename
}

// 获取文件类型（从扩展名推断）
function getFileType(filename: string): string {
  const ext = filename.split('.').pop()?.toLowerCase()
  const typeMap: Record<string, string> = {
    'jpg': '图片', 'jpeg': '图片', 'png': '图片', 'gif': '图片', 'webp': '图片', 'svg': '图片',
    'mp4': '视频', 'avi': '视频', 'mov': '视频', 'mkv': '视频', 'flv': '视频',
    'mp3': '音频', 'wav': '音频', 'flac': '音频', 'aac': '音频',
    'pdf': 'PDF', 'doc': '文档', 'docx': '文档', 'txt': '文本', 'md': '文本',
    'zip': '压缩', 'rar': '压缩', '7z': '压缩', 'tar': '压缩', 'gz': '压缩',
    'exe': '程序', 'msi': '程序', 'app': '程序',
    'js': '代码', 'ts': '代码', 'py': '代码', 'java': '代码', 'cpp': '代码',
  }
  return typeMap[ext || ''] || '文件'
}

// 下载文件
async function handleDownload(file: FileItem) {
  // 确保配置已加载
  if (!downloadConfig.value) {
    await loadDownloadConfig()
  }

  // 检查是否需要询问下载目录
  if (downloadConfig.value?.ask_each_time) {
    pendingDownloadFile.value = file
    showDownloadPicker.value = true
  } else {
    // 使用默认目录直接下载
    try {
      ElMessage.info('正在创建:' + file.server_filename + ' 下载任务...')

      // 创建下载任务
      await createDownload({
        fs_id: file.fs_id,
        remote_path: file.path,
        filename: file.server_filename,
        total_size: file.size,
        conflict_strategy: downloadConflictStrategy.value,
      })

      ElMessage.success('下载任务已创建')

    } catch (error: any) {
      ElMessage.error(error.message || '创建下载任务失败')
      console.error('创建下载任务失败:', error)
    }
  }
}

// 下载文件夹
async function handleDownloadFolder(folder: FileItem) {
  // 防止重复点击
  if (downloadingFolders.value.has(folder.path)) {
    return
  }

  // 确保配置已加载
  if (!downloadConfig.value) {
    await loadDownloadConfig()
  }

  // 检查是否需要询问下载目录
  if (downloadConfig.value?.ask_each_time) {
    pendingDownloadFile.value = folder
    showDownloadPicker.value = true
  } else {
    downloadingFolders.value.add(folder.path)

    try {
      // 获取显示名称（如果是加密文件夹，使用原始名称）
      const displayName = getDisplayName(folder)
      ElMessage.info('正在创建文件夹:' + displayName + ' 下载任务...')

      // 创建文件夹下载任务（如果是加密文件夹，传递原始名称）
      const originalName = folder.is_encrypted_folder ? folder.original_name : undefined
      await createFolderDownload(folder.path, originalName, downloadConflictStrategy.value)

      ElMessage.success('文件夹下载任务已创建，正在扫描文件...')

    } catch (error: any) {
      ElMessage.error(error.message || '创建文件夹下载任务失败')
      console.error('创建文件夹下载任务失败:', error)
    } finally {
      downloadingFolders.value.delete(folder.path)
    }
  }
}

// 处理 FilePicker 选择结果
async function handleFilePickerSelect(entry: FileEntry, encrypt: boolean = false, conflictStrategy?: string) {
  // 如果用户选择了冲突策略，更新当前策略
  if (conflictStrategy) {
    uploadConflictStrategy.value = conflictStrategy as any
  }

  try {
    if (entry.entryType === 'file') {
      // 单文件上传
      const remotePath = currentDir.value === '/'
          ? `/${entry.name}`
          : `${currentDir.value}/${entry.name}`

      await createUpload({
        local_path: entry.path,
        remote_path: remotePath,
        encrypt,
        conflict_strategy: uploadConflictStrategy.value,
      })

      ElMessage.success(encrypt ? '已添加加密上传任务' : '已添加上传任务')
    } else {
      // 文件夹上传
      const remoteFolderPath = currentDir.value === '/'
          ? `/${entry.name}`
          : `${currentDir.value}/${entry.name}`

      await createFolderUpload({
        local_folder: entry.path,
        remote_folder: remoteFolderPath,
        encrypt,
        conflict_strategy: uploadConflictStrategy.value,
      })

      ElMessage.success(encrypt ? '已添加加密文件夹上传任务' : '已添加文件夹上传任务')
    }

    // 更新上传最近目录（使用文件/文件夹的父目录）
    const parentDir = getParentDirectory(entry.path)
    if (parentDir) {
      updateRecentDirDebounced({ dir_type: 'upload', path: parentDir })
      if (uploadConfig.value) {
        uploadConfig.value.recent_directory = parentDir
      }
    }

  } catch (error: any) {
    ElMessage.error(error.message || '创建上传任务失败')
    console.error('创建上传任务失败:', error)
  }
}

// 处理 FilePicker 多选结果
async function handleFilePickerMultiSelect(entries: FileEntry[], encrypt: boolean = false, conflictStrategy?: string) {
  if (entries.length === 0) return

  // 如果用户选择了冲突策略，更新当前策略
  if (conflictStrategy) {
    uploadConflictStrategy.value = conflictStrategy as any
  }

  let successCount = 0
  let failedCount = 0

  ElMessage.info(`正在添加 ${entries.length} 个${encrypt ? '加密' : ''}上传任务...`)

  for (const entry of entries) {
    try {
      if (entry.entryType === 'file') {
        // 单文件上传
        const remotePath = currentDir.value === '/'
            ? `/${entry.name}`
            : `${currentDir.value}/${entry.name}`

        await createUpload({
          local_path: entry.path,
          remote_path: remotePath,
          encrypt,
          conflict_strategy: uploadConflictStrategy.value,
        })
        successCount++
      } else {
        // 文件夹上传
        const remoteFolderPath = currentDir.value === '/'
            ? `/${entry.name}`
            : `${currentDir.value}/${entry.name}`

        await createFolderUpload({
          local_folder: entry.path,
          remote_folder: remoteFolderPath,
          encrypt,
          conflict_strategy: uploadConflictStrategy.value,
        })
        successCount++
      }
    } catch (error: any) {
      failedCount++
      console.error(`上传任务创建失败: ${entry.name}`, error)
    }
  }

  // 显示结果
  if (failedCount === 0) {
    ElMessage.success(`成功添加 ${successCount} 个${encrypt ? '加密' : ''}上传任务`)
  } else if (successCount > 0) {
    ElMessage.warning(`成功 ${successCount} 个，失败 ${failedCount} 个`)
  } else {
    ElMessage.error(`全部 ${failedCount} 个任务创建失败`)
  }

  // 更新上传最近目录（使用第一个文件/文件夹的父目录）
  if (successCount > 0 && entries.length > 0) {
    const parentDir = getParentDirectory(entries[0].path)
    if (parentDir) {
      updateRecentDirDebounced({ dir_type: 'upload', path: parentDir })
      if (uploadConfig.value) {
        uploadConfig.value.recent_directory = parentDir
      }
    }
  }
}

// 获取文件/文件夹的父目录
function getParentDirectory(filePath: string): string | null {
  // 处理 Windows 和 Unix 风格路径
  const normalizedPath = filePath.replace(/\\/g, '/')
  const lastSlashIndex = normalizedPath.lastIndexOf('/')
  if (lastSlashIndex > 0) {
    return normalizedPath.substring(0, lastSlashIndex)
  } else if (lastSlashIndex === 0) {
    return '/'
  }
  return null
}

// 显示创建文件夹对话框
function showCreateFolderDialog() {
  createFolderDialogVisible.value = true
  createFolderForm.value.folderName = ''
}

// 对话框关闭时重置表单
function handleDialogClose() {
  createFolderForm.value.folderName = ''
  creatingFolder.value = false
}

// 创建文件夹
async function handleCreateFolder() {
  const folderName = createFolderForm.value.folderName.trim()

  // 验证文件夹名
  if (!folderName) {
    ElMessage.warning('请输入文件夹名称')
    return
  }

  // 验证文件夹名不能包含特殊字符
  if (/[<>:"/\\|?*]/.test(folderName)) {
    ElMessage.warning('文件夹名称不能包含特殊字符: < > : " / \\ | ? *')
    return
  }

  creatingFolder.value = true

  try {
    // 构建完整路径
    const fullPath = currentDir.value === '/'
        ? `/${folderName}`
        : `${currentDir.value}/${folderName}`

    // 调用创建文件夹 API
    await createFolder(fullPath)

    ElMessage.success('文件夹创建成功')

    // 关闭对话框
    createFolderDialogVisible.value = false

    // 刷新文件列表
    await loadFiles(currentDir.value)

  } catch (error: any) {
    ElMessage.error(error.message || '创建文件夹失败')
    console.error('创建文件夹失败:', error)
  } finally {
    creatingFolder.value = false
  }
}

// ============================================
// 批量选择与下载相关函数
// ============================================

// 加载下载配置
async function loadDownloadConfig() {
  try {
    const config = await getConfig()
    downloadConfig.value = config.download
    uploadConfig.value = config.upload

    // 加载默认冲突策略
    if (config.conflict_strategy) {
      uploadConflictStrategy.value = config.conflict_strategy.default_upload_strategy || 'smart_dedup'
      downloadConflictStrategy.value = config.conflict_strategy.default_download_strategy || 'overwrite'
    }
  } catch (error: any) {
    console.error('加载配置失败:', error)
  }

  // 加载加密状态
  try {
    const encryptionStatus = await getEncryptionStatus()
    hasEncryptionKey.value = encryptionStatus.has_key
  } catch (error: any) {
    console.error('加载加密状态失败:', error)
    hasEncryptionKey.value = false
  }
}

// 处理表格选择变化
function handleSelectionChange(selection: FileItem[]) {
  selectedFiles.value = selection
}

// 批量下载入口
async function handleBatchDownload() {
  if (selectedFiles.value.length === 0) {
    ElMessage.warning('请先选择要下载的文件或文件夹')
    return
  }

  // 确保配置已加载
  if (!downloadConfig.value) {
    await loadDownloadConfig()
  }

  // 检查是否需要询问下载目录
  if (downloadConfig.value?.ask_each_time) {
    showDownloadPicker.value = true
  } else {
    // 使用默认目录直接下载
    const targetDir = downloadConfig.value?.default_directory || downloadConfig.value?.download_dir || 'downloads'
    await executeBatchDownload(targetDir)
  }
}

// 处理下载目录确认
async function handleConfirmDownload(payload: { path: string; setAsDefault: boolean; conflictStrategy?: string }) {
  const { path, setAsDefault, conflictStrategy } = payload
  showDownloadPicker.value = false

  // 如果用户选择了冲突策略，更新当前策略
  if (conflictStrategy) {
    downloadConflictStrategy.value = conflictStrategy as any
  }

  // 如果设置为默认目录
  if (setAsDefault) {
    try {
      await setDefaultDownloadDir({ path })
      if (downloadConfig.value) {
        downloadConfig.value.default_directory = path
      }
    } catch (error: any) {
      console.error('设置默认下载目录失败:', error)
    }
  }

  // 更新最近目录（使用防抖版本，避免频繁 IO）
  updateRecentDirDebounced({ dir_type: 'download', path })
  if (downloadConfig.value) {
    downloadConfig.value.recent_directory = path
  }

  // 执行下载
  if (pendingDownloadFile.value) {
    // 单文件下载
    await executeSingleDownload(pendingDownloadFile.value, path)
    pendingDownloadFile.value = null
  } else {
    // 批量下载
    await executeBatchDownload(path)
  }
}

// 处理使用默认目录下载
async function handleUseDefaultDownload(conflictStrategy?: string) {
  showDownloadPicker.value = false

  // 如果用户选择了冲突策略，更新当前策略
  if (conflictStrategy) {
    downloadConflictStrategy.value = conflictStrategy as any
  }

  const targetDir = downloadConfig.value?.default_directory || downloadConfig.value?.download_dir || 'downloads'

  if (pendingDownloadFile.value) {
    // 单文件下载
    await executeSingleDownload(pendingDownloadFile.value, targetDir)
    pendingDownloadFile.value = null
  } else {
    // 批量下载
    await executeBatchDownload(targetDir)
  }
}

// 分批处理常量
const BATCH_SIZE = 10 // 每批处理 10 个下载项

// 执行批量下载（支持分批处理）
async function executeBatchDownload(targetDir: string) {
  if (selectedFiles.value.length === 0) return

  batchDownloading.value = true

  try {
    // 构建批量下载请求项
    const allItems: BatchDownloadItem[] = selectedFiles.value.map(file => ({
      fs_id: file.fs_id,
      path: file.path,
      name: file.server_filename,
      is_dir: file.isdir === 1,
      size: file.isdir === 0 ? file.size : undefined,
      // 🔥 修复：传递 original_name 以支持加密文件夹名称还原
      original_name: (file.is_encrypted || file.is_encrypted_folder) ? file.original_name : undefined
    }))

    const totalCount = allItems.length
    const batchCount = Math.ceil(totalCount / BATCH_SIZE)

    // 统计结果
    let totalTaskIds: string[] = []
    let totalFolderTaskIds: string[] = []
    let totalFailed: { path: string; reason: string }[] = []

    ElMessage.info(`正在创建 ${totalCount} 个下载任务（共 ${batchCount} 批）...`)

    // 分批处理
    for (let i = 0; i < batchCount; i++) {
      const start = i * BATCH_SIZE
      const end = Math.min(start + BATCH_SIZE, totalCount)
      const batchItems = allItems.slice(start, end)

      try {
        const response = await createBatchDownload({
          items: batchItems,
          target_dir: targetDir,
          conflict_strategy: downloadConflictStrategy.value,
        })

        // 累计结果
        totalTaskIds = totalTaskIds.concat(response.task_ids)
        totalFolderTaskIds = totalFolderTaskIds.concat(response.folder_task_ids)
        totalFailed = totalFailed.concat(response.failed)

        // 显示进度（仅在多批时显示）
        if (batchCount > 1) {
          console.log(`批次 ${i + 1}/${batchCount} 完成: ${response.task_ids.length + response.folder_task_ids.length} 成功, ${response.failed.length} 失败`)
        }

      } catch (batchError: any) {
        console.error(`批次 ${i + 1}/${batchCount} 失败:`, batchError)
        // 将整批标记为失败
        batchItems.forEach(item => {
          totalFailed.push({
            path: item.path,
            reason: batchError.message || '批次请求失败'
          })
        })
      }
    }

    // 显示最终结果统计
    const successCount = totalTaskIds.length + totalFolderTaskIds.length
    const failedCount = totalFailed.length

    if (failedCount === 0) {
      ElMessage.success(`成功创建 ${successCount} 个下载任务`)
    } else if (successCount > 0) {
      ElMessage.warning(`成功 ${successCount} 个，失败 ${failedCount} 个`)
      console.warn('部分下载任务创建失败:', totalFailed)
    } else {
      ElMessage.error(`全部 ${failedCount} 个任务创建失败`)
      console.error('批量下载创建失败:', totalFailed)
    }

    // 清空选择
    selectedFiles.value = []

  } catch (error: any) {
    ElMessage.error(error.message || '批量下载失败')
    console.error('批量下载失败:', error)
  } finally {
    batchDownloading.value = false
  }
}

// 执行单文件下载（带目录选择）
async function executeSingleDownload(file: FileItem, targetDir: string) {
  try {
    const displayName = getDisplayName(file)
    ElMessage.info('正在创建:' + displayName + ' 下载任务...')

    // 获取原始名称（如果是加密文件/文件夹）
    const originalName = (file.is_encrypted || file.is_encrypted_folder) ? file.original_name : undefined

    // 使用批量下载 API 以支持自定义目录
    const response = await createBatchDownload({
      items: [{
        fs_id: file.fs_id,
        path: file.path,
        name: file.server_filename,
        is_dir: file.isdir === 1,
        size: file.isdir === 0 ? file.size : undefined,
        original_name: originalName
      }],
      target_dir: targetDir,
      conflict_strategy: downloadConflictStrategy.value,
    })

    if (response.failed.length === 0) {
      ElMessage.success('下载任务已创建')
    } else {
      ElMessage.error(response.failed[0].reason || '创建下载任务失败')
    }

  } catch (error: any) {
    ElMessage.error(error.message || '创建下载任务失败')
    console.error('创建下载任务失败:', error)
  }
}

// ============================================
// 搜索相关函数
// ============================================

// 执行搜索
async function handleSearch() {
  const keyword = searchKeyword.value.trim()
  if (!keyword) {
    if (isSearchMode.value) {
      await exitSearch({ keepExpanded: true })
    }
    return
  }

  searchLoading.value = true
  isSearchMode.value = true
  searchPage.value = 1
  const version = ++fileRequestVersion

  try {
    const data = await searchFiles(keyword, 1, 100)
    if (version !== fileRequestVersion) return
    fileList.value = data.list as FileItem[]
    searchHasMore.value = data.has_more
  } catch (error: any) {
    ElMessage.error(error.message || '搜索失败')
    console.error('搜索失败:', error)
  } finally {
    searchLoading.value = false
    loading.value = false
  }
}

// 加载更多搜索结果
async function loadMoreSearchResults() {
  if (searchLoading.value || !searchHasMore.value) return

  searchLoading.value = true
  loadingMore.value = true
  const version = ++fileRequestVersion
  // 页码只在成功后前进，避免失败时无限累加
  const requestPage = searchPage.value + 1

  try {
    const data = await searchFiles(searchKeyword.value.trim(), requestPage, 100)
    if (version !== fileRequestVersion) return
    fileList.value = [...fileList.value, ...(data.list as FileItem[])]
    searchHasMore.value = data.has_more
    searchPage.value = requestPage
  } catch (error: any) {
    if (version !== fileRequestVersion) return
    // 失败时停止继续加载，避免滚动加载在出错后不断翻页死循环
    searchHasMore.value = false
    ElMessage.error(error.message || '加载更多搜索结果失败')
  } finally {
    if (version === fileRequestVersion) {
      searchLoading.value = false
      loadingMore.value = false
    }
  }
}

function resetSearchState(options: { keepExpanded?: boolean } = {}) {
  fileRequestVersion++
  isSearchMode.value = false
  searchKeyword.value = ''
  searchExpanded.value = options.keepExpanded ?? false
  searchPage.value = 1
  searchHasMore.value = false
}

async function openSearch() {
  if (!searchExpanded.value) {
    searchExpanded.value = true
    await nextTick()
  }
  searchInputRef.value?.focus()
}

async function handleSearchTrigger() {
  if (!searchKeyword.value.trim()) {
    if (isSearchMode.value) {
      await exitSearch({ keepExpanded: false })
      return
    }

    searchInputRef.value?.focus()
    return
  }

  if (!searchExpanded.value) {
    await openSearch()
    return
  }

  if (!searchKeyword.value.trim()) {
    if (isSearchMode.value) {
      await exitSearch()
    } else {
      resetSearchState()
    }
    return
  }

  await handleSearch()
}

async function handleSearchClear() {
  if (isSearchMode.value) {
    await exitSearch({ keepExpanded: false })
    return
  }

  await nextTick()
  searchInputRef.value?.focus()
}

async function handleSearchEscape() {
  if (isSearchMode.value) {
    await exitSearch({ keepExpanded: false })
    return
  }

  searchKeyword.value = ''
  await nextTick()
  searchInputRef.value?.focus()
}

function handleSearchOutsidePointerDown(event: MouseEvent) {
  const target = event.target as Node | null
  if (!searchExpanded.value) return
  if (target && searchWrapperRef.value?.contains(target)) return
  if (searchKeyword.value.trim()) return

  if (isSearchMode.value) {
    void exitSearch()
    return
  }

  resetSearchState()
}

// 退出搜索模式
async function exitSearch(options: { keepExpanded?: boolean } = {}) {
  resetSearchState(options)
  await loadFiles(currentDir.value)

  await nextTick()
  searchInputRef.value?.focus()
}

// 多账号切换处理器：重置到根目录并重拉文件列表
// （新账号的目录树通常与旧账号不同，沿用旧 currentDir 会 404 或显示空）
function handleActiveChanged() {
  if (isSearchMode.value) {
    resetSearchState()
  }
  selectedFiles.value = []
  loadFiles('/')
  loadDownloadConfig()
}

// 全局点击关闭右键菜单
function handleGlobalClick(event: MouseEvent) {
  if (contextMenuVisible.value) {
    // 如果点击的是右键菜单内部，不关闭
    const menuEl = document.querySelector('.context-menu')
    if (menuEl && menuEl.contains(event.target as Node)) {
      return
    }
    closeContextMenu()
  }
  handleSearchOutsidePointerDown(event)
}

// 组件挂载时加载根目录和配置
onMounted(() => {
  document.addEventListener('mousedown', handleGlobalClick)
  loadFiles('/')
  loadDownloadConfig()
  // 多账号 active 切换 → 重置到根目录并重拉（与 DownloadsView 等 5 视图一致）
  window.addEventListener('multi-account:active-changed', handleActiveChanged)
})

onBeforeUnmount(() => {
  document.removeEventListener('mousedown', handleGlobalClick)
  window.removeEventListener('multi-account:active-changed', handleActiveChanged)
  if (warmupRetryTimer) {
    clearTimeout(warmupRetryTimer)
    warmupRetryTimer = null
  }
  clientWarmingUp.value = false
})

// ============================================
// 转存相关函数
// ============================================

// 转存成功处理
function handleTransferSuccess(taskId: string) {
  console.log('转存任务创建成功:', taskId)
  // 刷新文件列表以显示转存后的文件
  refreshFileList()
}

// ============================================
// 分享相关函数
// ============================================

// 单个文件分享
function handleSingleShare(file: FileItem) {
  shareFiles.value = [file]
  showShareDialog.value = true
}

// 批量分享（工具栏按钮）
function handleBatchShare() {
  if (selectedFiles.value.length === 0) {
    ElMessage.warning('请先选择要分享的文件或文件夹')
    return
  }
  shareFiles.value = [...selectedFiles.value]
  showShareDialog.value = true
}

async function handleBatchDelete() {
  if (selectedFiles.value.length === 0) return
  const count = selectedFiles.value.length
  const paths = selectedFiles.value.map(f => f.path)
  try {
    await ElMessageBox.confirm(
        `确定要删除选中的 ${count} 个文件/文件夹吗？删除后可在回收站找回。`,
        '确认删除',
        {
          confirmButtonText: '删除',
          cancelButtonText: '取消',
          type: 'warning',
          beforeClose: async (action, instance, done) => {
            if (action !== 'confirm') { done(); return }
            instance.confirmButtonLoading = true
            instance.confirmButtonText = '删除中...'
            try {
              const result = await deleteFiles(paths)
              done()
              if (result.failed_paths.length > 0) {
                ElMessage.warning(`成功删除 ${result.deleted_count} 个，失败 ${result.failed_paths.length} 个`)
              } else {
                ElMessage.success(`成功删除 ${result.deleted_count} 个文件/文件夹`)
              }
              selectedFiles.value = []
              await refreshFileList()
            } catch (error: any) {
              done()
              // errno=132：百度二次安全验证（多见于删除文件过多的大目录）
              const isVerify = error?.code === 132 ||
                  (typeof error?.message === 'string' && error.message.includes('安全验证'))
              if (isVerify) {
                ElMessageBox.alert(
                    `百度风控拦截：${error.message}。删除大目录（内含大量文件）会触发百度二次安全验证，请前往 pan.baidu.com 在浏览器中完成验证后再试。`,
                    '删除被风控拦截',
                    { type: 'warning' }
                )
              } else {
                ElMessage.error(error.message || '删除失败')
              }
            } finally {
              instance.confirmButtonLoading = false
            }
          }
        }
    )
  } catch {
    // 用户取消
  }
}

// 分享成功处理
function handleShareSuccess() {
  // 清空选择
  selectedFiles.value = []
}

// ============================================
// 分享直下相关函数
// ============================================

// 分享直下成功处理
function handleShareDirectDownloadSuccess(taskId: string) {
  console.log('分享直下任务创建成功:', taskId)
  ElMessage.success('分享直下任务已创建')
}

// =====================================================
// 文件管理操作（filemanager: copy / move / rename）
// =====================================================

/** 选中文件中是否含加密文件/文件夹（这种禁止 copy/move） */
function selectionContainsEncrypted(): FileItem | null {
  return selectedFiles.value.find((f) => f.is_encrypted || f.is_encrypted_folder) ?? null
}

/** 触发批量复制：弹出文件夹选择器 */
function handleBatchCopy() {
  if (selectedFiles.value.length === 0) {
    ElMessage.warning('请先选择要复制的文件或文件夹')
    return
  }
  const enc = selectionContainsEncrypted()
  if (enc) {
    // 提示使用用户可见的原始名称（加密文件页面显示的是 original_name 而非 UUID.dat）
    ElMessage.error(`加密文件/文件夹禁止复制：${getDisplayName(enc)}`)
    return
  }
  folderPickerOperation.value = 'copy'
  folderPickerInitialPath.value = currentDir.value
  // 复制允许进入源文件夹自身子目录（百度支持），但禁选"目标 == 源父目录"：
  // 目录选择器无法改 newname，默认用源 basename，若选回源父目录相当于同名复制，
  // 百度会返回 errno=-8 这类"文件已存在"错误，提前在 UI 禁选避免误点。
  const exactBlocked = new Set<string>()
  for (const f of selectedFiles.value) {
    exactBlocked.add(dirname(f.path))
  }
  folderPickerBlockedPaths.value = []
  folderPickerBlockedExactPaths.value = Array.from(exactBlocked)
  showFolderPicker.value = true
}

/** 触发批量移动：根据后端校验规则同步禁选 */
function handleBatchMove() {
  if (selectedFiles.value.length === 0) {
    ElMessage.warning('请先选择要移动的文件或文件夹')
    return
  }
  const enc = selectionContainsEncrypted()
  if (enc) {
    ElMessage.error(`加密文件/文件夹禁止移动：${getDisplayName(enc)}`)
    return
  }
  folderPickerOperation.value = 'move'
  folderPickerInitialPath.value = currentDir.value
  // 与后端 move-only 校验对齐，分两类禁选：
  //   1) 子树禁选（subtree）：选中的文件夹 + 其所有子目录
  //      → 对应 ensure_not_move_into_self
  //   2) 精确禁选（exact）：选中文件 / 文件夹的父目录（仅自身，不含其它兄弟目录）
  //      → 对应 ensure_move_not_same_parent
  const subtreeBlocked = new Set<string>()
  const exactBlocked = new Set<string>()
  for (const f of selectedFiles.value) {
    if (f.isdir === 1) {
      subtreeBlocked.add(f.path)
    }
    exactBlocked.add(dirname(f.path))
  }
  folderPickerBlockedPaths.value = Array.from(subtreeBlocked)
  folderPickerBlockedExactPaths.value = Array.from(exactBlocked)
  showFolderPicker.value = true
}

/** 文件夹选择器确认 */
async function handleFolderPicked(destPath: string) {
  if (folderPickerOperation.value === 'copy') {
    await executeBatchCopy(destPath)
  } else {
    await executeBatchMove(destPath)
  }
}

/** 处理 filemanager 操作的统一结果，返回是否成功 */
function reportOutcome(action: string, outcome: FileOperationOutcomeDto): boolean {
  if (outcome.kind === 'success') {
    ElMessage.success(`${action}成功：共 ${outcome.total} 项`)
    return true
  }
  // failed
  if (outcome.still_running) {
    ElMessage.warning(`${action}任务仍在后台处理，请稍后刷新查看`)
    return false
  }
  // 风控判断：errno / task_errno 命中 132，或后端透传了 authwidget（含 saferand/safesign/safetpl）
  // 任一条件命中均视为百度风控介入。
  const hitVerifyCode = outcome.errno === 132 || outcome.task_errno === 132
  const hasAuthWidget = !!outcome.authwidget && Object.keys(outcome.authwidget).length > 0
  if (hitVerifyCode || hasAuthWidget) {
    ElMessageBox.alert(
        `百度风控拦截：${outcome.message}。请前往 pan.baidu.com 在浏览器中完成验证后再试。`,
        `${action}被风控拦截`,
        { type: 'warning' }
    )
    return false
  }
  ElMessage.error(`${action}失败：${outcome.message}`)
  return false
}

async function executeBatchCopy(dest: string) {
  const items: FileOperationItem[] = selectedFiles.value.map((f) => ({
    path: f.path,
    dest,
    newname: f.server_filename,
  }))
  batchCopying.value = true
  try {
    const outcome = await copyFiles(items)
    if (reportOutcome('复制', outcome)) {
      selectedFiles.value = []
      await refreshFileList()
    }
  } catch (e: unknown) {
    ElMessage.error(e instanceof Error ? e.message : '复制请求失败')
  } finally {
    batchCopying.value = false
  }
}

async function executeBatchMove(dest: string) {
  const items: FileOperationItem[] = selectedFiles.value.map((f) => ({
    path: f.path,
    dest,
    newname: f.server_filename,
  }))
  batchMoving.value = true
  try {
    const outcome = await moveFiles(items)
    if (reportOutcome('移动', outcome)) {
      selectedFiles.value = []
      await refreshFileList()
    }
  } catch (e: unknown) {
    ElMessage.error(e instanceof Error ? e.message : '移动请求失败')
  } finally {
    batchMoving.value = false
  }
}

/** 单个文件/文件夹重命名入口 */
function handleSingleRename(row: FileItem) {
  if (row.is_encrypted || row.is_encrypted_folder) {
    ElMessage.error(`加密${row.isdir === 1 ? '文件夹' : '文件'}禁止重命名`)
    return
  }
  renameTarget.value = row
  renameNewName.value = row.server_filename
  renameDialogVisible.value = true
}

function resetRenameDialog() {
  renameTarget.value = null
  renameNewName.value = ''
  renameSubmitting.value = false
}

async function handleRenameConfirm() {
  if (!renameTarget.value) return
  if (renameNameError.value) {
    ElMessage.error(renameNameError.value)
    return
  }
  if (renameNewName.value === renameTarget.value.server_filename) {
    ElMessage.info('名称未变化')
    renameDialogVisible.value = false
    return
  }
  renameSubmitting.value = true
  try {
    const outcome = await renameFile({
      path: renameTarget.value.path,
      newname: renameNewName.value,
      id: renameTarget.value.fs_id,
    })
    if (reportOutcome('重命名', outcome)) {
      renameDialogVisible.value = false
      await refreshFileList()
    }
  } catch (e: unknown) {
    ElMessage.error(e instanceof Error ? e.message : '重命名请求失败')
  } finally {
    renameSubmitting.value = false
  }
}

// 防止 import 被 tree-shake 误判 unused（basename / joinPath 在模板中未直接使用）
void basename
void joinPath
</script>

<script lang="ts">
// 图标导入
export {Folder, Document, Refresh, HomeFilled, Upload, ArrowDown, FolderAdd, Download, Share, Loading, Link, Delete, Search, Close, CopyDocument, Rank, Edit, Operation, TopRight} from '@element-plus/icons-vue'
</script>

<style scoped lang="scss">
.files-container {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background: var(--app-bg);
  overflow: hidden;
}

// 客户端预热遮罩：登录后后端 client 初始化期间覆盖文件视图
.client-warmup-mask {
  position: absolute;
  inset: 0;
  z-index: 2000;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  background: var(--app-surface);
  backdrop-filter: blur(4px);

  .warmup-text {
    margin: 0;
    font-size: 14px;
    color: var(--app-text-secondary);
  }
}

// =====================
// Toolbar
// =====================
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 0 24px;
  background: var(--app-surface);
  border-bottom: 1px solid var(--app-divider);
  min-height: 64px;
  flex-shrink: 0;

  .toolbar-left {
    flex: 1;
    min-width: 0;
    overflow: hidden;

    .breadcrumb {
      font-size: 15px;
      font-weight: 500;
      display: block;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;

      :deep(.el-breadcrumb__list) {
        flex-wrap: nowrap !important;
      }

      :deep(.el-breadcrumb__item) {
        cursor: pointer;
        flex-shrink: 0;

        &:hover .el-breadcrumb__inner {
          color: var(--app-accent);
        }

        .el-breadcrumb__inner {
          transition: color 0.2s ease;
          color: var(--app-text-primary);
        }
      }

      .breadcrumb-text {
        display: inline-block;
        max-width: 120px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        vertical-align: bottom;
      }
    }
  }

  .toolbar-center {
    flex: 0 0 auto;
    width: 300px;
    
    .search-box {
      position: relative;
      display: flex;
      align-items: center;
      width: 100%;
      height: 40px;
      
      .search-icon {
        position: absolute;
        left: 12px;
        color: var(--app-text-tertiary);
        font-size: 16px;
        pointer-events: none;
      }
      
      input {
        width: 100%;
        height: 40px;
        padding: 0 36px 0 36px;
        border: 1px solid var(--app-border);
        border-radius: 12px;
        background: var(--app-surface);
        color: var(--app-text-primary);
        font-size: 14px;
        outline: none;
        transition: all 0.2s ease;
        
        &::placeholder {
          color: var(--app-text-tertiary);
        }
        
        &:focus {
          border-color: var(--app-accent);
          box-shadow: 0 0 0 3px var(--app-accent-bg);
        }
      }
      
      .clear-btn {
        position: absolute;
        right: 8px;
        padding: 4px;
        border: none;
        background: transparent;
        color: var(--app-text-tertiary);
        cursor: pointer;
        border-radius: 6px;
        transition: all 0.2s ease;
        
        &:hover {
          background: var(--app-surface-hover);
          color: var(--app-text-primary);
        }
      }
    }
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
    
    .el-button {
      height: 38px;
      border-radius: var(--app-radius-sm);
      padding: 0 16px;
      font-size: 14px;
      
      .el-icon {
        font-size: 16px;
        margin-right: 4px;
      }
    }
  }
}

// =====================
// File List
// =====================
.file-list {
  flex: 1;
  overflow: auto;
  background: var(--app-surface);
  
  :deep(.el-table) {
    --el-table-header-bg-color: var(--app-surface-active);
    --el-table-row-hover-bg-color: var(--app-file-hover);
    --el-table-current-row-bg-color: var(--app-accent-bg);
    
    font-size: 14px;
    
    .el-table__header-wrapper {
      th {
        font-weight: 600;
        font-size: 13px;
        color: var(--app-text-secondary);
        padding: 0;
        height: 44px;
        background: var(--app-surface-active);
        border-bottom: 1px solid var(--app-divider);
      }
    }
    
    .el-table__body-wrapper {
      tr {
        transition: background-color 0.2s ease;
        
        &:hover {
          background-color: var(--app-file-hover);
        }
      }
      
      td {
        padding: 0;
        height: 52px;
        color: var(--app-text-primary);
        border-bottom: 1px solid var(--app-row-divider);
      }
    }
  }
  
  // File name cell
  .file-name-cell {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-left: 12px;
    
    .file-icon {
      flex-shrink: 0;
      font-size: 20px;
      color: var(--app-text-secondary);
    }
    
    .folder-icon {
      color: var(--app-accent);
    }
    
    .filename-text {
      flex: 1;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-weight: 500;
      font-size: 16px;
      color: var(--app-text-primary);
    }
    
    .encrypted-tag {
      flex-shrink: 0;
      font-size: 12px;
      padding: 1px 4px;
    }
  }
  
  
  // Size and time columns
  .size-text,
  .time-text,
  .type-text {
    font-size: 14px;
    color: var(--app-text-secondary);
  }
  
  .folder-text {
    color: var(--app-text-tertiary);
    font-size: 14px;
  }
}

// =====================
// Status Bar
// =====================
.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 24px;
  background: var(--app-surface);
  border-top: 1px solid var(--app-divider);
  font-size: 13px;
  color: var(--app-text-secondary);
  flex-shrink: 0;
  min-height: 40px;
  
  .status-left,
  .status-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  
  .item-count {
    font-weight: 500;
    color: var(--app-text-primary);
  }
  
  .selection-info {
    color: var(--app-accent);
    font-weight: 500;
  }
  
  .path-display {
    font-size: 12px;
    opacity: 0.7;
    max-width: 400px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

// =====================
// Loading & Empty States
// =====================
.loading-indicator,
.end-indicator {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 20px;
  color: var(--app-text-tertiary);
  font-size: 14px;
}

// =====================
// Responsive Adjustments
// =====================
@media (max-width: 1024px) {
  .toolbar {
    flex-wrap: wrap;
    gap: 8px;
    
    .toolbar-center {
      order: 3;
      width: 100%;
    }
  }
  
  .status-bar {
    .path-display {
      display: none;
    }
  }
}

// =====================
// Context Menu - Desktop Native
// =====================
.context-menu {
  position: fixed;
  z-index: 9999;
  min-width: 200px;
  background: var(--app-surface);
  border: 1px solid var(--app-divider);
  border-radius: 8px;
  box-shadow: var(--app-shadow-card);
  padding: 6px 0;
  font-size: 13px;
  animation: contextMenuFadeIn 0.1s ease;
  user-select: none;

  @keyframes contextMenuFadeIn {
    from {
      opacity: 0;
      transform: scale(0.95);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .context-menu-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    margin: 0 6px 4px;
    background: var(--app-surface-active);
    border-radius: 6px;
    color: var(--app-text-primary);
    font-weight: 500;
    font-size: 12px;

    .truncate {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      flex: 1;
    }

    .el-icon {
      flex-shrink: 0;
      color: var(--app-accent);
    }
  }

  .context-menu-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    margin: 0 4px;
    border-radius: 5px;
    cursor: pointer;
    color: var(--app-text-primary);
    transition: background-color 0.08s ease;

    .el-icon {
      color: var(--app-text-secondary);
      flex-shrink: 0;
    }

    span {
      flex: 1;
    }

    &:hover {
      background-color: var(--app-surface-hover);
    }

    &:active {
      background-color: var(--app-surface-active);
    }

    &.danger {
      color: var(--el-color-danger);

      .el-icon {
        color: var(--el-color-danger);
      }

      &:hover {
        background-color: var(--el-color-danger-light-9);
      }
    }
  }

  .context-menu-divider {
    height: 1px;
    background: var(--app-divider);
    margin: 4px 10px;
  }
}
</style>
