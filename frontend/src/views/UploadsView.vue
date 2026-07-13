<template>
  <div class="uploads-container">
    <!-- 顶部工具栏 -->
    <div class="toolbar">
      <div class="header-left">
        <h2>上传管理</h2>
        <el-tag :type="activeCountType">
          {{ activeCount }} 个任务进行中
        </el-tag>
        <!-- 多账号过滤器 -->
        <AccountFilter
            v-if="authStore.hasMultipleAccounts"
            v-model="ownerFilter"
            :counts="ownerFilterCounts"
            class="account-filter-slot"
        />
      </div>
      <div class="header-right">
        <el-button size="small" @click="refreshTasks">
          <el-icon><Refresh /></el-icon>
          <span>刷新</span>
        </el-button>
        <el-dropdown @command="handleBatchCommand" trigger="click">
          <el-button size="small">
            批量操作
            <el-icon class="el-icon--right"><ArrowDown /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="pause" :disabled="activeCount === 0">
                <el-icon><VideoPause /></el-icon>
                全部暂停 ({{ activeCount }})
              </el-dropdown-item>
              <el-dropdown-item command="resume" :disabled="pausedCount === 0">
                <el-icon><VideoPlay /></el-icon>
                全部继续 ({{ pausedCount }})
              </el-dropdown-item>
              <el-dropdown-item command="clearCompleted" :disabled="completedCount === 0" divided>
                <el-icon><Delete /></el-icon>
                清除已完成 ({{ completedCount }})
              </el-dropdown-item>
              <el-dropdown-item command="clearFailed" :disabled="failedCount === 0">
                <el-icon><Delete /></el-icon>
                清除失败 ({{ failedCount }})
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </div>

    <!-- 上传任务列表 -->
    <div class="task-container">
      <el-empty v-if="!loading && displayedItems.length === 0" :description="ownerFilter === null ? '暂无上传任务' : '当前过滤条件下暂无上传任务'">
        <template #image>
          <el-icon :size="80" color="var(--app-text-secondary)"><Upload /></el-icon>
        </template>
        <template #description>
          <p>暂无上传任务</p>
          <p style="font-size: 12px; color: var(--app-text-secondary);">
            前往「网盘管理」页面点击"上传"按钮
          </p>
        </template>
      </el-empty>

      <div v-else class="task-list">
        <div
            v-for="item in displayedItems"
            :key="item.id"
            class="task-row"
            :class="{ 'task-active': item.status === 'uploading' || item.status === 'encrypting' }"
        >
          <!-- 主行：图标 + 文件名 + 状态 + 统计 + 操作 -->
          <div class="task-row-main">
            <div class="task-row-info">
              <el-icon :size="16" class="file-icon">
                <Upload />
              </el-icon>
              <span class="filename">{{ getFilename(item.local_path) }}</span>
              <el-tag :type="getStatusType(item.status)" size="small">
                {{ getStatusText(item.status) }}
              </el-tag>
              <!-- 多账号 chip -->
              <AccountBadge :owner-uid="item.owner_uid" size="small" class="task-account-badge" />
              <!-- 秒传标识 -->
              <el-tag v-if="item.is_rapid_upload && item.status === 'completed'" type="success" size="small">
                <el-icon><CircleCheck /></el-icon>
                秒传
              </el-tag>
              <!-- 加密标识（已完成的加密任务） -->
              <el-tag v-if="item.encrypt_enabled && (item.status === 'completed' || item.status === 'rapid_upload_success')" type="info" size="small">
                <el-icon><Lock /></el-icon>
                已加密
              </el-tag>
            </div>

            <!-- 紧凑统计 -->
            <div class="task-row-stats">
              <span class="stat-item">{{ formatFileSize(item.uploaded_size) }} / {{ formatFileSize(item.total_size) }}</span>
              <span v-if="item.status === 'uploading'" class="stat-item speed">{{ formatSpeed(item.speed) }}</span>
              <span v-if="item.status === 'uploading'" class="stat-item">{{ formatETA(calculateETA(item)) }}</span>
            </div>

            <!-- 操作按钮 -->
            <div class="task-row-actions">
              <el-button
                  v-if="item.status === 'uploading'"
                  size="small"
                  circle
                  @click="handlePause(item)"
              >
                <el-icon><VideoPause /></el-icon>
              </el-button>
              <el-button
                  v-if="item.status === 'paused'"
                  size="small"
                  type="primary"
                  circle
                  @click="handleResume(item)"
              >
                <el-icon><VideoPlay /></el-icon>
              </el-button>
              <el-button
                  v-if="item.status === 'failed'"
                  size="small"
                  type="warning"
                  circle
                  @click="handleResume(item)"
              >
                <el-icon><RefreshRight /></el-icon>
              </el-button>
              <el-button
                  size="small"
                  type="danger"
                  circle
                  @click="handleDelete(item)"
              >
                <el-icon><Delete /></el-icon>
              </el-button>
            </div>
          </div>

          <!-- 路径 -->
          <div class="task-row-path">
            本地: {{ item.local_path }} → 网盘: {{ item.remote_path }}
          </div>

          <!-- 加密进度 -->
          <div v-if="item.status === 'encrypting'" class="encrypt-progress">
            <el-icon class="encrypt-icon"><Lock /></el-icon>
            <span>正在加密文件...</span>
            <el-progress
                :percentage="item.encrypt_progress || 0"
                :stroke-width="4"
                status="warning"
                class="encrypt-progress-bar"
            >
              <template #default="{ percentage }">
                <span class="progress-text">{{ percentage.toFixed(1) }}%</span>
              </template>
            </el-progress>
          </div>

          <!-- 进度条 -->
          <div class="task-row-progress" v-if="item.status !== 'encrypting'">
            <el-progress
                :percentage="calculateProgress(item)"
                :status="getProgressStatus(item.status)"
                :stroke-width="4"
            >
              <template #default="{ percentage }">
                <span class="progress-text">{{ percentage.toFixed(1) }}%</span>
              </template>
            </el-progress>
          </div>

          <!-- 错误信息 -->
          <div v-if="item.error" class="task-row-error">
            <span class="error-label">错误:</span>
            <span class="error-text">{{ item.error }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- 底部状态栏 -->
    <div class="status-bar">
      <div class="status-left">
        <span class="status-item">总任务: {{ uploadItems.length }}</span>
        <span class="status-item active">活跃: {{ activeCount }}</span>
        <span class="status-item completed">已完成: {{ completedCount }}</span>
        <span v-if="failedCount > 0" class="status-item failed">失败: {{ failedCount }}</span>
        <span v-if="pausedCount > 0" class="status-item paused">暂停: {{ pausedCount }}</span>
      </div>
      <div class="status-right">
        <span class="status-item">{{ wsConnected ? 'WebSocket 已连接' : 'WebSocket 未连接' }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  getAllUploads,
  pauseUpload,
  resumeUpload,
  deleteUpload,
  clearCompleted,
  clearFailed,
  batchPauseUploads,
  batchResumeUploads,
  calculateProgress,
  calculateETA,
  formatFileSize,
  formatSpeed,
  formatETA,
  getStatusText,
  getStatusType,
  extractFilename,
  type UploadTask,
  type UploadTaskStatus,
} from '@/api/upload'
import {
  Refresh,
  Upload,
  VideoPause,
  VideoPlay,
  Delete,
  CircleCheck,
  RefreshRight,
  Lock,
  ArrowDown,
} from '@element-plus/icons-vue'
import {createAdaptivePoller} from '@/utils/backendHealth'
// 🔥 WebSocket 相关导入
import { getWebSocketClient, connectWebSocket, type ConnectionState } from '@/utils/websocket'
import type { UploadEvent } from '@/types/events'
// 多账号集成
import { useAuthStore } from '@/stores/auth'
import AccountFilter from '@/components/AccountFilter.vue'
import AccountBadge from '@/components/AccountBadge.vue'

// 状态
const loading = ref(false)
const uploadItems = ref<UploadTask[]>([])

// 多账号状态
const authStore = useAuthStore()
const ownerFilter = ref<number | null>(null)

const displayedItems = computed(() => {
  if (ownerFilter.value === null) return uploadItems.value
  return uploadItems.value.filter((item) => item.owner_uid === ownerFilter.value)
})

const ownerFilterCounts = computed(() => {
  const map: Record<number, number> = {}
  for (const item of uploadItems.value) {
    const uid = typeof item.owner_uid === 'number' ? item.owner_uid : null
    if (uid !== null) map[uid] = (map[uid] || 0) + 1
  }
  return map
})

// 自适应轮询器：WS 未连接时兜底刷新；后端断开后阶梯式退避重连，避免高频刷屏
const refreshPoller = createAdaptivePoller(() => { refreshTasks() }, { baseDelayMs: 1000, maxDelayMs: 30000 })
// 🔥 WebSocket 事件订阅清理函数
let unsubscribeUpload: (() => void) | null = null
let unsubscribeConnectionState: (() => void) | null = null
// 🔥 WebSocket 连接状态
const wsConnected = ref(false)
// 🔥 是否已成功加载过一次任务列表，用于初始加载失败时保持重试
let initialLoadDone = false

// 是否有活跃任务（需要实时刷新）
const hasActiveTasks = computed(() => {
  return uploadItems.value.some(item =>
      item.status === 'uploading' || item.status === 'pending' || item.status === 'encrypting' || item.status === 'checking_rapid'
  )
})

// 计算属性
const activeCount = computed(() => {
  return uploadItems.value.filter(item => item.status === 'uploading' || item.status === 'encrypting').length
})

const completedCount = computed(() => {
  return uploadItems.value.filter(item => item.status === 'completed').length
})

const failedCount = computed(() => {
  return uploadItems.value.filter(item => item.status === 'failed').length
})

const pausedCount = computed(() => {
  return uploadItems.value.filter(item => item.status === 'paused').length
})

const activeCountType = computed(() => {
  if (activeCount.value === 0) return 'info'
  if (activeCount.value <= 3) return 'success'
  return 'warning'
})

// 获取文件名
function getFilename(path: string): string {
  return extractFilename(path)
}

// 获取进度条状态
function getProgressStatus(status: UploadTaskStatus): 'success' | 'exception' | 'warning' | undefined {
  if (status === 'completed' || status === 'rapid_upload_success') return 'success'
  if (status === 'failed') return 'exception'
  if (status === 'paused') return 'warning'
  if (status === 'encrypting') return 'warning'
  return undefined
}

// 刷新任务列表
async function refreshTasks() {
  // 如果正在加载中，跳过本次请求，避免并发请求
  if (loading.value) {
    return
  }

  loading.value = true
  try {
    uploadItems.value = await getAllUploads()
    initialLoadDone = true
  } catch (error: any) {
    console.error('刷新任务列表失败:', error)
    // 🔥 不清空列表：保留现有数据，避免临时失败导致页面变空 + 轮询停止的死锁
  } finally {
    loading.value = false
    // 无论成功还是失败，都要检查并更新自动刷新状态
    updateAutoRefresh()
  }
}

// 更新自动刷新状态
function updateAutoRefresh() {
  // 🔥 如果 WebSocket 已连接，不使用轮询（由 WebSocket 推送更新）
  if (wsConnected.value) {
    if (refreshPoller.isRunning()) {
      console.log('[UploadsView] WebSocket 已连接，停止轮询')
      refreshPoller.stop()
    }
    return
  }

  // 🔥 WebSocket 未连接时，回退到轮询模式
  // 有活跃任务 或 初始加载尚未成功时，启动或保持定时刷新
  if (hasActiveTasks.value || !initialLoadDone) {
    if (!refreshPoller.isRunning()) {
      console.log('[UploadsView] WebSocket 未连接，启动轮询模式，活跃任务数:', activeCount.value)
      refreshPoller.start()
    }
  } else {
    if (refreshPoller.isRunning()) {
      console.log('[UploadsView] 停止轮询，当前任务数:', uploadItems.value.length)
      refreshPoller.stop()
    }
  }
}

// 暂停任务
async function handlePause(item: UploadTask) {
  try {
    await pauseUpload(item.id)
    ElMessage.success('任务已暂停')
    refreshTasks()
  } catch (error: any) {
    console.error('暂停任务失败:', error)
  }
}

// 恢复任务
async function handleResume(item: UploadTask) {
  try {
    await resumeUpload(item.id)
    ElMessage.success(item.status === 'failed' ? '任务正在重试' : '任务已继续')
    refreshTasks()
  } catch (error: any) {
    console.error('恢复任务失败:', error)
  }
}

// 删除任务
async function handleDelete(item: UploadTask) {
  try {
    await ElMessageBox.confirm(
        '确定要删除此上传任务吗？',
        '删除确认',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
    )

    await deleteUpload(item.id)
    ElMessage.success('任务已删除')
    refreshTasks()
  } catch (error: any) {
    if (error !== 'cancel') {
      console.error('删除任务失败:', error)
    }
  }
}

// 清除已完成
async function handleClearCompleted() {
  try {
    await ElMessageBox.confirm(
        `确定要清除所有已完成的任务吗？（共${completedCount.value}个）`,
        '批量清除',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
    )
    const count = await clearCompleted()
    ElMessage.success(`已清除 ${count} 个任务`)
    refreshTasks()
  } catch (error: any) {
    if (error !== 'cancel') {
      console.error('清除已完成任务失败:', error)
    }
  }
}

// 清除失败
async function handleClearFailed() {
  try {
    await ElMessageBox.confirm(
        `确定要清除所有失败的任务吗？（共${failedCount.value}个）`,
        '批量清除',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
    )
    const count = await clearFailed()
    ElMessage.success(`已清除 ${count} 个任务`)
    refreshTasks()
  } catch (error: any) {
    if (error !== 'cancel') {
      console.error('清除失败任务失败:', error)
    }
  }
}

// 批量操作命令分发
function handleBatchCommand(command: string) {
  switch (command) {
    case 'pause': handleBatchPause(); break
    case 'resume': handleBatchResume(); break
    case 'clearCompleted': handleClearCompleted(); break
    case 'clearFailed': handleClearFailed(); break
  }
}

// 全部暂停
async function handleBatchPause() {
  try {
    const res = await batchPauseUploads({ all: true })
    ElMessage.success(`已暂停 ${res.success_count} 个任务`)
    refreshTasks()
  } catch (error: any) {
    console.error('批量暂停失败:', error)
  }
}

// 全部继续
async function handleBatchResume() {
  try {
    const res = await batchResumeUploads({ all: true })
    ElMessage.success(`已恢复 ${res.success_count} 个任务`)
    refreshTasks()
  } catch (error: any) {
    console.error('批量恢复失败:', error)
  }
}

// 🔥 处理上传事件
function handleUploadEvent(event: UploadEvent) {
  console.log('[UploadsView] 收到上传事件:', event.event_type, event.task_id)

  switch (event.event_type) {
    case 'created':
      // 新任务创建，刷新列表
      refreshTasks()
      break
    case 'progress': {
      // 进度更新
      const progressIdx = uploadItems.value.findIndex(t => t.id === event.task_id)
      if (progressIdx !== -1) {
        uploadItems.value[progressIdx].uploaded_size = event.uploaded_size
        uploadItems.value[progressIdx].total_size = event.total_size
        uploadItems.value[progressIdx].speed = event.speed
        if (event.completed_chunks !== undefined) {
          uploadItems.value[progressIdx].completed_chunks = event.completed_chunks
        }
        if (event.total_chunks !== undefined) {
          uploadItems.value[progressIdx].total_chunks = event.total_chunks
        }
        // 🔥 如果当前是加密状态，收到传输进度后自动切换为上传状态
        if (uploadItems.value[progressIdx].status === 'encrypting') {
          uploadItems.value[progressIdx].status = 'uploading'
        }
      }
      break
    }
    case 'encrypt_progress': {
      // 🔥 加密进度更新
      const encryptIdx = uploadItems.value.findIndex(t => t.id === event.task_id)
      if (encryptIdx !== -1) {
        uploadItems.value[encryptIdx].encrypt_progress = event.encrypt_progress
        uploadItems.value[encryptIdx].status = 'encrypting'
      }
      break
    }
    case 'encrypt_completed': {
      // 🔥 加密完成
      const encryptCompletedIdx = uploadItems.value.findIndex(t => t.id === event.task_id)
      if (encryptCompletedIdx !== -1) {
        uploadItems.value[encryptCompletedIdx].encrypt_progress = 100
        uploadItems.value[encryptCompletedIdx].original_size = event.original_size
        // 🔥 直接更新状态为 uploading，避免依赖 status_changed 事件导致状态不同步
        uploadItems.value[encryptCompletedIdx].status = 'uploading'
      }
      break
    }
    case 'status_changed': {
      // 状态变更
      const statusIdx = uploadItems.value.findIndex(t => t.id === event.task_id)
      if (statusIdx !== -1) {
        uploadItems.value[statusIdx].status = event.new_status as UploadTaskStatus
      }
      break
    }
    case 'completed':
    case 'failed':
      // 完成或失败，刷新列表获取最终状态
      refreshTasks()
      break
    case 'paused': {
      // 任务暂停，直接更新状态
      const pausedIdx = uploadItems.value.findIndex(t => t.id === event.task_id)
      if (pausedIdx !== -1) {
        uploadItems.value[pausedIdx].status = 'paused'
        uploadItems.value[pausedIdx].speed = 0
      }
      break
    }
    case 'resumed': {
      // 任务恢复，直接更新状态为 uploading
      const resumedIdx = uploadItems.value.findIndex(t => t.id === event.task_id)
      if (resumedIdx !== -1) {
        // 🔥 设为 uploading 而不是 pending，这样 UI 会显示速度和剩余时间
        // 后续的 progress 事件会更新实际的速度值
        uploadItems.value[resumedIdx].status = 'uploading'
      }
      break
    }
    case 'deleted':
      uploadItems.value = uploadItems.value.filter(t => t.id !== event.task_id)
      break
  }
}

// 🔥 设置 WebSocket 订阅
function setupWebSocketSubscriptions() {
  const wsClient = getWebSocketClient()

  // 🔥 订阅服务端上传事件
  wsClient.subscribe(['upload:*'])

  unsubscribeUpload = wsClient.onUploadEvent(handleUploadEvent)

  unsubscribeConnectionState = wsClient.onConnectionStateChange((state: ConnectionState) => {
    const wasConnected = wsConnected.value
    wsConnected.value = state === 'connected'

    console.log('[UploadsView] WebSocket 状态变化:', state, ', 是否连接:', wsConnected.value)

    // 🔥 任何状态变化都检查轮询策略（包括 connecting 状态）
    updateAutoRefresh()

    // 🔥 WebSocket 重新连接成功时，刷新一次获取最新数据
    if (!wasConnected && wsConnected.value) {
      refreshTasks()
    }
  })

  connectWebSocket()
  console.log('[UploadsView] WebSocket 订阅已设置')
}

// 🔥 清理 WebSocket 订阅
function cleanupWebSocketSubscriptions() {
  const wsClient = getWebSocketClient()

  // 🔥 取消服务端订阅
  wsClient.unsubscribe(['upload:*'])

  if (unsubscribeUpload) {
    unsubscribeUpload()
    unsubscribeUpload = null
  }
  if (unsubscribeConnectionState) {
    unsubscribeConnectionState()
    unsubscribeConnectionState = null
  }
  console.log('[UploadsView] WebSocket 订阅已清理')
}

// 多账号切换处理器
function handleActiveChanged() {
  refreshTasks()
}

// 组件挂载时加载任务列表
onMounted(() => {
  refreshTasks()
  setupWebSocketSubscriptions()
  window.addEventListener('multi-account:active-changed', handleActiveChanged)
})

// 组件卸载时清除定时器
onUnmounted(() => {
  refreshPoller.stop()
  cleanupWebSocketSubscriptions()
  window.removeEventListener('multi-account:active-changed', handleActiveChanged)
})
</script>

<style scoped lang="scss">
.uploads-container {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--app-bg);
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: var(--app-surface);
  border-bottom: 1px solid var(--app-divider);
  padding: 8px 16px;

  .header-left {
    display: flex;
    align-items: center;
    gap: 12px;

    h2 {
      margin: 0;
      font-size: 15px;
      font-weight: 600;
      color: var(--app-text-primary);
    }
  }

  .header-right {
    display: flex;
    gap: 8px;
  }
}

.task-container {
  flex: 1;
  padding: 0;
  overflow: auto;
}

.task-list {
  display: flex;
  flex-direction: column;
}

// =====================
// 紧凑任务行
// =====================
.task-row {
  display: flex;
  flex-direction: column;
  padding: 8px 16px;
  border-bottom: 1px solid var(--app-divider);
  transition: background-color 0.15s ease;
  font-size: 13px;

  &.task-active {
    background: var(--app-accent-bg);
  }

  &:hover {
    background: var(--app-surface-hover);
  }
}

// 主行：信息 + 统计 + 操作
.task-row-main {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 28px;
}

// 文件信息区
.task-row-info {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
  overflow: hidden;

  .file-icon {
    flex-shrink: 0;
    color: var(--el-color-success);
  }

  .filename {
    font-size: 13px;
    font-weight: 500;
    color: var(--app-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
}

// 紧凑统计区
.task-row-stats {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
  font-size: 12px;
  color: var(--app-text-secondary);
  white-space: nowrap;

  .stat-item {
    font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;

    &.speed {
      color: var(--el-color-success);
      font-weight: 500;
    }
  }
}

// 操作按钮区
.task-row-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;

  .el-button {
    padding: 4px;
  }
}

// 路径
.task-row-path {
  font-size: 11px;
  color: var(--app-text-tertiary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  padding-left: 24px;
  margin-top: 2px;
  font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;
}

// 进度条
.task-row-progress {
  margin-top: 4px;
  padding-left: 24px;

  .progress-text {
    font-size: 11px;
    font-weight: 500;
  }
}

// 错误信息
.task-row-error {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  padding-left: 24px;
  font-size: 11px;

  .error-label {
    color: var(--el-color-danger);
    font-weight: 500;
    flex-shrink: 0;
  }

  .error-text {
    color: var(--el-color-danger);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
}

:deep(.el-progress__text) {
  font-size: 11px !important;
}

// =====================
// 加密进度样式（紧凑）
// =====================
.encrypt-progress {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
  padding-left: 24px;
  font-size: 11px;
  color: var(--el-color-warning);

  .encrypt-icon {
    animation: pulse 1.5s infinite;
    flex-shrink: 0;
  }

  .encrypt-progress-bar {
    flex: 1;
    min-width: 0;
  }

  .progress-text {
    font-size: 11px;
    font-weight: 500;
  }
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

// =====================
// 底部状态栏
// =====================
.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 16px;
  background: var(--app-surface-active);
  border-top: 1px solid var(--app-divider);
  font-size: 12px;
  color: var(--app-text-secondary);
  flex-shrink: 0;

  .status-left,
  .status-right {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .status-item {
    font-weight: 500;

    &.active {
      color: var(--app-accent);
    }

    &.completed {
      color: var(--el-color-success);
    }

    &.failed {
      color: var(--el-color-danger);
    }

    &.paused {
      color: var(--el-color-warning);
    }
  }
}
</style>
