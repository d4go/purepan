<template>
  <div class="cloud-dl-view">
    <!-- 顶部工具栏 -->
    <div class="toolbar">
      <div class="header-left">
        <h2>离线下载</h2>
        <el-tag :type="activeCountType" size="large">
          {{ activeCount }} 个任务进行中
        </el-tag>
        <!-- 离线下载是百度云端实时数据，仅显示活跃账号；切换账号通过右上角 AccountSwitcher 触发刷新 -->
      </div>
      <div class="header-right">
        <el-button type="primary" @click="showAddDialog = true">
          <el-icon><Plus /></el-icon>
          添加任务
        </el-button>
        <el-button @click="handleRefresh" :loading="isRefreshing">
          <el-icon><Refresh /></el-icon>
          刷新
        </el-button>
      </div>
    </div>

    <!-- 任务列表表格 -->
    <div class="task-container">
      <el-empty
          v-if="!loading && displayedTasks.length === 0"
          description="暂无离线下载任务"
      />
      <el-table v-else :data="displayedTasks" v-loading="loading" style="width: 100%">
        <el-table-column prop="task_name" label="任务名称" min-width="200" show-overflow-tooltip />
        <el-table-column label="状态" width="120">
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)">{{ row.status_text || getStatusText(row.status) }}</el-tag>
          </template>
        </el-table-column>
        <!--
          🔥
          离线下载已收紧为「按 active 账号隔离」（与 SharesView 同语义），
          不再显示账号列；切换账号通过右上角 AccountSwitcher 触发自动重拉。
          `owner_uid` 仍保留为内部事件路由 / 兼容字段，但 UI 不再展示。
        -->
        <el-table-column prop="save_path" label="保存路径" min-width="150" show-overflow-tooltip />
        <el-table-column label="创建时间" width="180">
          <template #default="{ row }">{{ formatTimestamp(row.create_time) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="{ row }">
            <el-button size="small" type="primary" plain @click="handleShowDetail(row)">详情</el-button>
            <el-button v-if="row.status === 1" size="small" @click="handleCancel(row)">取消</el-button>
            <el-button size="small" type="danger" @click="handleDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <!-- 任务详情弹窗 -->
    <el-dialog
        v-model="showDetailDialog"
        title="任务详情"
        width="600px"
        @close="detailTask = null"
    >
      <div v-if="detailLoading" class="detail-loading">
        <el-icon class="is-loading"><Loading /></el-icon>
        <span>加载中...</span>
      </div>
      <div v-else-if="detailTask" class="task-detail">
        <div class="detail-header">
          <h3 class="detail-title">{{ detailTask.task_name }}</h3>
          <el-tag :type="getStatusType(detailTask.status)" size="large">
            {{ detailTask.status_text || getStatusText(detailTask.status) }}
          </el-tag>
        </div>

        <!-- 进度条（仅进行中任务显示） -->
        <div v-if="detailTask.status === 1" class="detail-progress">
          <el-progress
              :percentage="getProgress(detailTask)"
              :stroke-width="10"
              status="warning"
          >
            <template #default="{ percentage }">
              <span class="progress-text">{{ percentage.toFixed(1) }}%</span>
            </template>
          </el-progress>
          <div class="progress-info">
            {{ formatFileSize(detailTask.finished_size) }} / {{ formatFileSize(detailTask.file_size) }}
          </div>
        </div>

        <!-- 基本信息 -->
        <div class="detail-section">
          <h4>基本信息</h4>
          <div class="detail-grid">
            <div class="detail-item">
              <span class="detail-label">任务ID</span>
              <span class="detail-value">{{ detailTask.task_id }}</span>
            </div>
            <div class="detail-item">
              <span class="detail-label">文件大小</span>
              <span class="detail-value">{{ formatFileSize(detailTask.file_size) || '未知' }}</span>
            </div>
            <div class="detail-item">
              <span class="detail-label">已下载</span>
              <span class="detail-value">{{ formatFileSize(detailTask.finished_size) }}</span>
            </div>
            <div class="detail-item">
              <span class="detail-label">保存路径</span>
              <span class="detail-value path">{{ detailTask.save_path }}</span>
            </div>
            <div class="detail-item">
              <span class="detail-label">创建时间</span>
              <span class="detail-value">{{ formatTimestamp(detailTask.create_time) }}</span>
            </div>
            <div class="detail-item" v-if="detailTask.start_time > 0">
              <span class="detail-label">开始时间</span>
              <span class="detail-value">{{ formatTimestamp(detailTask.start_time) }}</span>
            </div>
            <div class="detail-item" v-if="detailTask.finish_time > 0">
              <span class="detail-label">完成时间</span>
              <span class="detail-value">{{ formatTimestamp(detailTask.finish_time) }}</span>
            </div>
          </div>
        </div>

        <!-- 下载链接 -->
        <div class="detail-section">
          <h4>下载链接</h4>
          <div class="source-url">{{ detailTask.source_url }}</div>
        </div>

        <!-- 文件列表 -->
        <div v-if="detailTask.file_list && detailTask.file_list.length > 0" class="detail-section">
          <h4>文件列表 ({{ detailTask.file_list.length }} 个文件)</h4>
          <div class="file-list">
            <div v-for="(file, index) in detailTask.file_list" :key="index" class="file-item">
              <el-icon><Document /></el-icon>
              <span class="file-name">{{ file.file_name }}</span>
              <span class="file-size">{{ formatFileSize(file.file_size) }}</span>
            </div>
          </div>
        </div>
      </div>
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="showDetailDialog = false">关闭</el-button>
          <el-button v-if="detailTask && detailTask.status === 1" @click="handleCancelFromDetail">取消任务</el-button>
          <el-button v-if="detailTask" type="danger" @click="handleDeleteFromDetail">删除任务</el-button>
        </div>
      </template>
    </el-dialog>

    <!-- 添加任务对话框 -->
    <el-dialog
        v-model="showAddDialog"
        title="添加离线下载任务"
        width="550px"
        @close="resetAddForm"
    >
      <el-form :model="addForm" label-width="100px" label-position="right">
        <!--
          🔥
          CloudDl 收紧为 active-only（与 SharesView 同语义），UI 不再展示账号维度。
          创建强制使用当前活跃账号、列表只显示当前账号、不再渲染 AccountBadge。
          切换账号请使用右上角 AccountSwitcher，切换后自动重拉。
          跨账号创建后 list/cancel 走 active
          路由会出现行为不一致与安全注入风险，因此仍保留「强制 active_uid」契约。
        -->
        <el-form-item label="下载链接" required>
          <el-input
              v-model="addForm.source_url"
              type="textarea"
              :rows="3"
              placeholder="请输入下载链接（支持 HTTP/HTTPS/磁力链接/ed2k）"
          />
        </el-form-item>
        <el-form-item label="保存路径">
          <div class="path-selector">
            <el-input v-model="addForm.save_path" placeholder="默认保存到根目录" readonly />
            <el-button @click="showPathSelector = true">选择</el-button>
          </div>
        </el-form-item>
        <el-form-item label="自动下载">
          <el-switch v-model="addForm.auto_download" />
          <span class="auto-download-hint">完成后自动下载到本地</span>
        </el-form-item>
        <el-form-item v-if="addForm.auto_download" label="下载目录">
          <div class="path-selector">
            <el-input v-model="addForm.local_download_path" placeholder="选择本地下载目录" readonly />
            <el-button @click="showLocalPathSelector = true">选择</el-button>
          </div>
          <el-checkbox v-model="addForm.ask_download_path" class="ask-path-checkbox">
            每次询问下载目录
          </el-checkbox>
        </el-form-item>
      </el-form>
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="showAddDialog = false">取消</el-button>
          <el-button type="primary" @click="handleAddTask" :loading="adding" :disabled="!addForm.source_url.trim()">
            确定
          </el-button>
        </div>
      </template>
    </el-dialog>

    <!-- 网盘路径选择器 -->
    <NetdiskPathSelector
        v-model="addForm.save_path"
        :fs-id="addForm.save_path_fs_id"
        @update:fs-id="addForm.save_path_fs_id = $event"
        ref="netdiskPathSelectorRef"
    />
    <el-dialog
        v-model="showPathSelector"
        title="选择网盘保存路径"
        width="600px"
        @open="handlePathSelectorOpen"
    >
      <NetdiskPathSelector
          v-model="tempSavePath"
          :fs-id="tempSavePathFsId"
          @update:fs-id="tempSavePathFsId = $event"
      />
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="showPathSelector = false">取消</el-button>
          <el-button type="primary" @click="confirmSavePath">确定</el-button>
        </div>
      </template>
    </el-dialog>

    <!-- 本地目录选择器 -->
    <FilePickerModal
        v-model="showLocalPathSelector"
        mode="select-directory"
        title="选择本地下载目录"
        confirm-text="选择"
        :initial-path="downloadConfig?.recent_directory || downloadConfig?.default_directory"
        @confirm="handleLocalPathSelect"
    />

    <!-- 自动下载确认弹窗 -->
    <FilePickerModal
        v-model="showAutoDownloadPicker"
        mode="download"
        title="选择下载目录"
        :default-download-dir="autoDownloadDefaultDir || downloadConfig?.default_directory"
        :initial-path="downloadConfig?.recent_directory || downloadConfig?.default_directory"
        @confirm-download="handleAutoDownloadConfirm"
        @use-default="handleAutoDownloadUseDefault"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Refresh, Loading, Document } from '@element-plus/icons-vue'
import { formatFileSize } from '@/utils/fileUtils'
import NetdiskPathSelector from '@/components/NetdiskPathSelector.vue'
import { FilePickerModal } from '@/components/FilePicker'
import {
  listTasks,
  addTask,
  cancelTask,
  deleteTask,
  queryTask,
  getStatusText,
  getStatusType,
  calculateProgress,
  formatTimestamp,
  type CloudDlTaskInfo,
  CloudDlTaskStatus,
} from '@/api/cloudDl'
import {
  useCloudDlWebSocket,
  type CloudDlTaskCompletedEvent,
  type CloudDlTaskListRefreshedEvent,
  type CloudDlProgressUpdateEvent,
  type CloudDlStatusChangedEvent,
} from '@/composables/useCloudDlWebSocket'
import { createBatchDownload, type BatchDownloadItem } from '@/api/download'
import { getConfig, updateRecentDirDebounced, updateTransferConfig, type DownloadConfig, type UploadConfig, type TransferConfig } from '@/api/config'

// CloudDl 是 active-only 路由，但 WS 订阅是 cloud_dl 通用主题，
// 切账号过程中可能收到旧账号的 in-flight 事件。需要按 event.owner_uid === activeUid
// 过滤，否则旧事件可能写回新账号视图 / 触发跨账号自动下载弹窗。
import { useAuthStore } from '@/stores/auth'
// 多账号集成：CloudDl 收紧为 active-only 后 UI 不再展示账号维度。
// AccountFilter 已移除：离线下载只显示活跃账号实时数据，切换账号自动刷新
// AccountBadge 已移除：与 SharesView 同语义
// AccountSelect 已移除：离线下载不再支持跨账号创建
// useAuthStore 已不再导入：UI 不再依赖账号维度状态，
//   切账号自动重拉由 multi-account:active-changed window 事件驱动

// 配置状态
const downloadConfig = ref<DownloadConfig | null>(null)
const uploadConfig = ref<UploadConfig | null>(null)
const transferConfig = ref<TransferConfig | null>(null)

// 状态
const loading = ref(false)
const adding = ref(false)
const tasks = ref<CloudDlTaskInfo[]>([])

// active-uid 过滤源 — 用于 WS 事件归属判断
const authStore = useAuthStore()

// 请求版本号 — 切账号时丢弃旧请求回包
//
// 防御场景：A 账号 listTasks/refresh 在路上 → 用户切到 B 账号 → B 的列表先回 →
// A 的列表后回会覆盖 B 视图。后续操作按 B active 路由，可能 404 / 误操作 B 资源。
let cloudDlListRequestVersion = 0

// 详情请求独立版本号
//
// 切账号清空详情状态已经做了，但仍挡不住"A 详情请求在路上 → 切到 B → 用户打开
// B 的详情 → A 旧回包覆盖 B 弹窗"序列。这里和 list 分开维护版本号，因为详情
// 弹窗的生命周期独立于列表（用户可只刷新列表不开详情）。
let cloudDlDetailRequestVersion = 0

// WS 事件归属过滤 helper
//
// CloudDl 已收紧为 active-only 路由，但 WS 订阅是 `cloud_dl` 通用主题，事件流里
// 可能短暂出现旧账号事件（切账号时 `state.rebind_cloud_dl_ws_subscribers` 完成
// 之前已在飞行的事件 / 多账号 monitor 并行轮询窗口）。所有 WS 回调入口先过滤，
// 不匹配 → 直接 return，避免覆盖新账号视图或触发跨账号自动下载弹窗。
function isCurrentCloudDlEvent(
    event: { owner_uid?: number | null },
): boolean {
  const ownerUid = event.owner_uid
  // 后端总是 stamp owner_uid（cloud_dl_monitor.convert_to_ws_event）；
  // 若缺失视为遗留事件，遵循"宁失勿误"语义直接丢弃
  if (typeof ownerUid !== 'number') {
    return false
  }
  return ownerUid === authStore.activeUid
}
const showAddDialog = ref(false)

// 多账号状态：CloudDl 收紧为 active-only 后 UI 不再依赖账号维度状态。
// ownerFilter / ownerFilterCounts 已移除：
//   离线下载是百度网盘实时数据，仅显示当前活跃账号；切换账号会自动 refresh，不再需要前端做账号过滤。
// 🔥
//   离线下载不再支持跨账号创建，newTaskOwnerUid 已移除。

const displayedTasks = computed(() => tasks.value)

const showPathSelector = ref(false)
const showLocalPathSelector = ref(false)
const showAutoDownloadPicker = ref(false)

// 详情弹窗状态
const showDetailDialog = ref(false)
const detailLoading = ref(false)
const detailTask = ref<CloudDlTaskInfo | null>(null)

// 添加任务表单 - 使用明确的类型定义避免 undefined 问题
interface AddTaskFormData {
  source_url: string
  save_path: string
  save_path_fs_id: number
  auto_download: boolean
  local_download_path: string
  ask_download_path: boolean
}

const addForm = ref<AddTaskFormData>({
  source_url: '',
  save_path: '/',
  save_path_fs_id: 0,
  auto_download: false,
  local_download_path: '',
  ask_download_path: false,
})

// 临时路径选择
const tempSavePath = ref('/')
const tempSavePathFsId = ref(0)

// 自动下载相关
const autoDownloadDefaultDir = ref('')
const pendingAutoDownloadTask = ref<CloudDlTaskInfo | null>(null)

// 自动下载配置存储（task_id -> config）
const autoDownloadConfigs = ref<Map<number, { localPath: string; askEachTime: boolean }>>(new Map())

// 计算属性
const activeCount = computed(() => {
  return tasks.value.filter(task => task.status === CloudDlTaskStatus.Running).length
})

const activeCountType = computed(() => {
  if (activeCount.value === 0) return 'info'
  if (activeCount.value <= 3) return 'success'
  return 'warning'
})

// WebSocket 订阅
const { isRefreshing, refresh } = useCloudDlWebSocket({
  onStatusChanged: handleStatusChanged,
  onTaskCompleted: handleTaskCompleted,
  onProgressUpdate: handleProgressUpdate,
  onTaskListRefreshed: handleTaskListRefreshed,
})

// 事件处理函数
function handleStatusChanged(event: CloudDlStatusChangedEvent) {
  // 按 owner_uid 过滤当前活跃账号
  if (!isCurrentCloudDlEvent(event)) return
  const index = tasks.value.findIndex(t => t.task_id === event.task_id)
  if (index !== -1) {
    tasks.value[index] = event.task
  }
  // 同步更新详情弹窗中的任务
  if (detailTask.value && detailTask.value.task_id === event.task_id) {
    detailTask.value = event.task
  }
}

function handleTaskCompleted(event: CloudDlTaskCompletedEvent) {
  // 按 owner_uid 过滤当前活跃账号
  // 重要：跨账号事件不应触发自动下载弹窗（pendingAutoDownloadTask 走当前
  // active 账号的 createBatchDownload，会把 A 账号任务下载到 B 账号本地路径）
  if (!isCurrentCloudDlEvent(event)) return

  const index = tasks.value.findIndex(t => t.task_id === event.task_id)
  if (index !== -1) {
    tasks.value[index] = event.task
  }

  // 检查是否需要自动下载
  // 注意：当 askEachTime 为 false 时，后端已经自动执行了下载，前端不需要重复执行
  // 前端只在 askEachTime 为 true 时弹窗让用户选择目录
  const config = autoDownloadConfigs.value.get(event.task_id)
  if (config) {
    if (config.askEachTime) {
      // 弹窗询问下载目录（后端不会自动下载，需要前端处理）
      pendingAutoDownloadTask.value = event.task
      autoDownloadDefaultDir.value = config.localPath || ''
      showAutoDownloadPicker.value = true
    }
    // 注意：当 askEachTime 为 false 时，后端已经执行了自动下载，前端不需要再调用 triggerAutoDownload
    // 清除配置
    autoDownloadConfigs.value.delete(event.task_id)
  }
}

function handleProgressUpdate(event: CloudDlProgressUpdateEvent) {
  // 按 owner_uid 过滤当前活跃账号
  if (!isCurrentCloudDlEvent(event)) return
  const index = tasks.value.findIndex(t => t.task_id === event.task_id)
  if (index !== -1) {
    tasks.value[index].finished_size = event.finished_size
    tasks.value[index].file_size = event.file_size
  }
  // 同步更新详情弹窗中的任务进度
  if (detailTask.value && detailTask.value.task_id === event.task_id) {
    detailTask.value.finished_size = event.finished_size
    detailTask.value.file_size = event.file_size
  }
}

function handleTaskListRefreshed(event: CloudDlTaskListRefreshedEvent) {
  // 按 owner_uid 过滤当前活跃账号
  if (!isCurrentCloudDlEvent(event)) return
  tasks.value = event.tasks
}

// 获取进度百分比
function getProgress(task: CloudDlTaskInfo): number {
  return calculateProgress(task)
}

// 获取任务卡片状态类名
function getTaskCardClass(status: number): string {
  switch (status) {
    case CloudDlTaskStatus.Success:
      return 'status-success'
    case CloudDlTaskStatus.Running:
      return 'status-running'
    case CloudDlTaskStatus.SystemError:
    case CloudDlTaskStatus.ResourceNotFound:
    case CloudDlTaskStatus.Timeout:
    case CloudDlTaskStatus.DownloadFailed:
    case CloudDlTaskStatus.InsufficientSpace:
      return 'status-error'
    default:
      return ''
  }
}

// 显示任务详情
async function handleShowDetail(task: CloudDlTaskInfo) {
  // 递增版本号，旧账号 in-flight queryTask 回包会被下方 check 丢弃
  const version = ++cloudDlDetailRequestVersion
  showDetailDialog.value = true
  detailLoading.value = true
  detailTask.value = null

  try {
    const detail = await queryTask(task.task_id)
    if (version !== cloudDlDetailRequestVersion) return
    detailTask.value = detail
  } catch (error: any) {
    if (version !== cloudDlDetailRequestVersion) return
    ElMessage.error('获取任务详情失败: ' + (error.message || error))
    // 如果获取详情失败，使用列表中的基本信息
    detailTask.value = task
  } finally {
    if (version === cloudDlDetailRequestVersion) {
      detailLoading.value = false
    }
  }
}

// 从详情弹窗取消任务
async function handleCancelFromDetail() {
  if (!detailTask.value) return

  try {
    await ElMessageBox.confirm(
        `确定要取消任务 "${detailTask.value.task_name}" 吗？`,
        '取消确认',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
    )

    await cancelTask(detailTask.value.task_id)
    ElMessage.success('任务已取消')
    showDetailDialog.value = false
    await handleRefresh()
  } catch (error: any) {
    if (error !== 'cancel') {
      ElMessage.error('取消任务失败: ' + (error.message || error))
    }
  }
}

// 从详情弹窗删除任务
async function handleDeleteFromDetail() {
  if (!detailTask.value) return

  try {
    await ElMessageBox.confirm(
        `确定要删除任务 "${detailTask.value.task_name}" 吗？`,
        '删除确认',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
    )

    await deleteTask(detailTask.value.task_id)
    ElMessage.success('任务已删除')
    showDetailDialog.value = false
    await handleRefresh()
  } catch (error: any) {
    if (error !== 'cancel') {
      ElMessage.error('删除任务失败: ' + (error.message || error))
    }
  }
}

// 刷新任务列表
async function handleRefresh() {
  // 请求版本号防御
  const version = ++cloudDlListRequestVersion
  try {
    const result = await refresh()
    if (version !== cloudDlListRequestVersion) return
    tasks.value = result
  } catch (error: any) {
    if (version !== cloudDlListRequestVersion) return
    ElMessage.error('刷新失败: ' + (error.message || error))
  }
}

// 加载任务列表
async function loadTasks() {
  // 请求版本号防御
  const version = ++cloudDlListRequestVersion
  loading.value = true
  try {
    const response = await listTasks()
    if (version !== cloudDlListRequestVersion) return
    tasks.value = response.tasks
  } catch (error: any) {
    if (version !== cloudDlListRequestVersion) return
    ElMessage.error('加载任务列表失败: ' + (error.message || error))
  } finally {
    if (version === cloudDlListRequestVersion) {
      loading.value = false
    }
  }
}

// 重置添加表单
function resetAddForm() {
  addForm.value = {
    source_url: '',
    save_path: '/',
    save_path_fs_id: 0,
    auto_download: false,
    local_download_path: '',
    ask_download_path: false,
  }
  // 🔥 不再设置归属账号（后端强制使用 active_uid）
}

// 路径选择器打开
function handlePathSelectorOpen() {
  // 优先使用当前表单中的路径，否则使用最近保存的网盘路径
  if (addForm.value.save_path && addForm.value.save_path !== '/') {
    tempSavePath.value = addForm.value.save_path
    tempSavePathFsId.value = addForm.value.save_path_fs_id
  } else if (transferConfig.value?.recent_save_path) {
    // 使用转存配置中的最近保存路径
    tempSavePath.value = transferConfig.value.recent_save_path
    tempSavePathFsId.value = transferConfig.value.recent_save_fs_id || 0
  } else {
    tempSavePath.value = '/'
    tempSavePathFsId.value = 0
  }
}

// 确认保存路径
function confirmSavePath() {
  addForm.value.save_path = tempSavePath.value
  addForm.value.save_path_fs_id = tempSavePathFsId.value
  showPathSelector.value = false
}

// 本地路径选择
function handleLocalPathSelect(path: string) {
  addForm.value.local_download_path = path
  // 更新最近下载目录
  updateRecentDirDebounced({ dir_type: 'download', path })
  if (downloadConfig.value) {
    downloadConfig.value.recent_directory = path
  }
}

// 添加任务
async function handleAddTask() {
  const sourceUrl = addForm.value.source_url.trim()
  if (!sourceUrl) {
    ElMessage.warning('请输入下载链接')
    return
  }

  adding.value = true
  try {
    const response = await addTask({
      source_url: sourceUrl,
      save_path: addForm.value.save_path || '/',
      auto_download: addForm.value.auto_download,
      // 🔥 不再传 owner_uid，后端强制使用 active_uid
      local_download_path: addForm.value.local_download_path || undefined,
      ask_download_path: addForm.value.ask_download_path,
    })

    ElMessage.success(`任务添加成功，任务ID: ${response.task_id}`)

    // 更新最近保存的网盘路径
    if (addForm.value.save_path && addForm.value.save_path !== '/') {
      updateTransferConfig({
        recent_save_fs_id: addForm.value.save_path_fs_id,
        recent_save_path: addForm.value.save_path,
      }).catch(err => console.error('更新最近保存路径失败:', err))

      if (transferConfig.value) {
        transferConfig.value.recent_save_fs_id = addForm.value.save_path_fs_id
        transferConfig.value.recent_save_path = addForm.value.save_path
      }
    }

    // 自动下载配置已通过 API 传递给后端，后端会注册到监听服务
    // 前端也保存一份用于 UI 显示
    if (addForm.value.auto_download) {
      autoDownloadConfigs.value.set(response.task_id, {
        localPath: addForm.value.local_download_path,
        askEachTime: addForm.value.ask_download_path,
      })
    }

    showAddDialog.value = false
    resetAddForm()

    // 刷新任务列表
    await handleRefresh()
  } catch (error: any) {
    ElMessage.error('添加任务失败: ' + (error.message || error))
  } finally {
    adding.value = false
  }
}

// 取消任务
async function handleCancel(task: CloudDlTaskInfo) {
  try {
    await ElMessageBox.confirm(
        `确定要取消任务 "${task.task_name}" 吗？`,
        '取消确认',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
    )

    await cancelTask(task.task_id)
    ElMessage.success('任务已取消')
    await handleRefresh()
  } catch (error: any) {
    if (error !== 'cancel') {
      ElMessage.error('取消任务失败: ' + (error.message || error))
    }
  }
}

// 删除任务
async function handleDelete(task: CloudDlTaskInfo) {
  try {
    await ElMessageBox.confirm(
        `确定要删除任务 "${task.task_name}" 吗？`,
        '删除确认',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
    )

    await deleteTask(task.task_id)
    ElMessage.success('任务已删除')
    await handleRefresh()
  } catch (error: any) {
    if (error !== 'cancel') {
      ElMessage.error('删除任务失败: ' + (error.message || error))
    }
  }
}

// 自动下载确认
function handleAutoDownloadConfirm(payload: { path: string; setAsDefault: boolean }) {
  if (pendingAutoDownloadTask.value) {
    triggerAutoDownload(pendingAutoDownloadTask.value, payload.path)
    pendingAutoDownloadTask.value = null
    // 更新最近下载目录
    updateRecentDirDebounced({ dir_type: 'download', path: payload.path })
    if (downloadConfig.value) {
      downloadConfig.value.recent_directory = payload.path
    }
  }
}

// 使用默认目录自动下载
function handleAutoDownloadUseDefault() {
  if (pendingAutoDownloadTask.value && autoDownloadDefaultDir.value) {
    triggerAutoDownload(pendingAutoDownloadTask.value, autoDownloadDefaultDir.value)
    pendingAutoDownloadTask.value = null
  }
}

// 触发自动下载
async function triggerAutoDownload(task: CloudDlTaskInfo, localPath: string) {
  try {
    // 构建下载项
    const items: BatchDownloadItem[] = task.file_list.map(file => ({
      fs_id: 0, // 离线下载完成的文件需要通过路径获取 fs_id
      path: `${task.save_path}/${file.file_name}`,
      name: file.file_name,
      is_dir: false,
      size: file.file_size,
    }))

    if (items.length === 0) {
      ElMessage.warning('没有可下载的文件')
      return
    }

    const response = await createBatchDownload({
      items,
      target_dir: localPath,
    })

    if (response.task_ids.length > 0 || response.folder_task_ids.length > 0) {
      ElMessage.success(`已创建 ${response.task_ids.length + response.folder_task_ids.length} 个下载任务`)
    }

    if (response.failed.length > 0) {
      ElMessage.warning(`${response.failed.length} 个文件下载失败`)
    }
  } catch (error: any) {
    ElMessage.error('自动下载失败: ' + (error.message || error))
  }
}

// 多账号切换处理器
// 🔥 切账号时只需重拉列表，无需手动 unsubscribe / resubscribe。
// 后端 `helpers::set_active_uid` 已经在 `AccountEvent::Switched` 广播之前调
// `state.rebind_cloud_dl_ws_subscribers(new_uid)`，把当前所有订阅 `cloud_dl` 的连接
// 计数从旧 monitor 转到新 monitor，并 stamp 新 uid 到事件 `owner_uid` 字段。
// 前端 WS 订阅维持 `cloud_dl` 通用主题不变，事件流无缝切到新账号的 monitor。
//
// 递增 cloudDlListRequestVersion 并清空 tasks，让旧账号
// in-flight `listTasks/refresh` 回包被内部 version check 丢弃，避免覆盖新账号视图。
//
// 同时关闭详情弹窗、清空 detailTask / autoDownloadConfigs /
// pendingAutoDownloadTask / showAutoDownloadPicker，否则旧账号事件 → 上轮在路上的
// 详情可能在新账号视图弹出 / 触发跨账号自动下载。
function handleActiveChanged() {
  cloudDlListRequestVersion++
  cloudDlDetailRequestVersion++ // 丢弃旧账号详情请求回包
  tasks.value = []
  showDetailDialog.value = false
  detailTask.value = null
  pendingAutoDownloadTask.value = null
  showAutoDownloadPicker.value = false
  autoDownloadDefaultDir.value = ''
  autoDownloadConfigs.value.clear()
  loadTasks()
}

// 生命周期
onMounted(async () => {
  // 加载配置
  try {
    const config = await getConfig()
    downloadConfig.value = config.download
    uploadConfig.value = config.upload
    transferConfig.value = config.transfer || null
  } catch (error: any) {
    console.error('加载配置失败:', error)
  }

  // 加载任务列表
  loadTasks()

  // 多账号切换事件订阅
  window.addEventListener('multi-account:active-changed', handleActiveChanged)
})

onUnmounted(() => {
  window.removeEventListener('multi-account:active-changed', handleActiveChanged)
})
</script>

<style scoped lang="scss">
.cloud-dl-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 20px;
  background: var(--app-bg);
  overflow: hidden;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
  flex-shrink: 0;

  .header-left {
    display: flex;
    align-items: center;
    gap: 16px;

    h2 {
      margin: 0;
      font-size: 20px;
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
  background: var(--app-surface);
  border-radius: 8px;
  padding: 16px;
  overflow: auto;
}

// =====================
// 任务详情弹窗样式
// =====================
.detail-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  color: var(--app-text-secondary);

  .el-icon {
    font-size: 32px;
    margin-bottom: 12px;
  }
}

.task-detail {
  .detail-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
    margin-bottom: 20px;
    padding-bottom: 16px;
    border-bottom: 1px solid var(--app-divider);

    .detail-title {
      flex: 1;
      margin: 0;
      font-size: 18px;
      font-weight: 600;
      color: var(--app-text-primary);
      word-break: break-all;
      line-height: 1.4;
    }
  }

  .detail-progress {
    margin-bottom: 20px;
    padding: 16px;
    background: var(--el-color-warning-light-9);
    border-radius: 8px;

    .progress-info {
      margin-top: 8px;
      text-align: center;
      font-size: 13px;
      color: var(--el-color-warning);
    }
  }

  .detail-section {
    margin-bottom: 20px;

    h4 {
      margin: 0 0 12px 0;
      font-size: 14px;
      font-weight: 600;
      color: var(--app-text-secondary);
    }
  }

  .detail-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 12px;

    .detail-item {
      display: flex;
      flex-direction: column;
      gap: 4px;
      padding: 10px 12px;
      background: var(--app-surface-hover);
      border-radius: 6px;

      .detail-label {
        font-size: 12px;
        color: var(--app-text-secondary);
      }

      .detail-value {
        font-size: 14px;
        color: var(--app-text-primary);
        word-break: break-all;

        &.path {
          color: var(--app-accent);
        }
      }
    }
  }

  .source-url {
    padding: 12px;
    background: var(--app-surface-hover);
    border-radius: 6px;
    font-size: 13px;
    color: var(--app-text-secondary);
    word-break: break-all;
    line-height: 1.5;
    max-height: 100px;
    overflow-y: auto;
  }

  .file-list {
    max-height: 200px;
    overflow-y: auto;
    border: 1px solid var(--app-divider);
    border-radius: 6px;

    .file-item {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 10px 12px;
      border-bottom: 1px solid var(--app-divider);

      &:last-child {
        border-bottom: none;
      }

      .el-icon {
        color: var(--app-text-secondary);
        flex-shrink: 0;
      }

      .file-name {
        flex: 1;
        font-size: 13px;
        color: var(--app-text-primary);
        word-break: break-all;
      }

      .file-size {
        flex-shrink: 0;
        font-size: 12px;
        color: var(--app-text-secondary);
      }
    }
  }
}

// 进度文本
.progress-text {
  font-size: 12px;
  color: var(--app-text-secondary);
}

// 添加任务对话框样式
.path-selector {
  display: flex;
  gap: 8px;
  width: 100%;

  .el-input {
    flex: 1;
  }
}

.auto-download-hint {
  margin-left: 12px;
  font-size: 13px;
  color: var(--app-text-secondary);
}

.ask-path-checkbox {
  margin-top: 8px;
  display: block;
}

// =====================
// 对话框样式优化
// =====================

// 对话框基础样式
:deep(.el-dialog) {
  border-radius: 12px;
  overflow: hidden;
  box-shadow: var(--app-shadow-lg);

  // 对话框头部
  .el-dialog__header {
    background: var(--app-surface);
    border-bottom: 1px solid var(--app-divider);
    padding: 18px 24px;
    margin-right: 0;

    .el-dialog__title {
      font-size: 17px;
      font-weight: 600;
      color: var(--app-text-primary);
      letter-spacing: 0.3px;
    }

    .el-dialog__headerbtn {
      top: 18px;
      right: 20px;
      width: 28px;
      height: 28px;

      .el-dialog__close {
        font-size: 16px;
        color: var(--app-text-secondary);
        transition: all 0.3s cubic-bezier(0.25, 0.1, 0.25, 1);

        &:hover {
          color: var(--app-accent);
          transform: rotate(90deg);
        }
      }
    }
  }

  // 对话框内容区
  .el-dialog__body {
    padding: 24px;
    background: var(--app-surface);
  }

  // 对话框底部
  .el-dialog__footer {
    background: var(--app-surface-hover);
    border-top: 1px solid var(--app-divider);
    padding: 14px 24px;
  }
}

// 表单项样式
:deep(.el-form-item) {
  margin-bottom: 22px;

  &:last-child {
    margin-bottom: 0;
  }

  .el-form-item__label {
    font-weight: 500;
    color: var(--app-text-secondary);
    font-size: 14px;

    &::before {
      color: var(--el-color-danger);
    }
  }

  .el-form-item__content {
    line-height: 1.5;
  }
}

// 输入框样式
:deep(.el-input) {
  .el-input__wrapper {
    border-radius: 6px;
    transition: all 0.3s cubic-bezier(0.25, 0.1, 0.25, 1);
    box-shadow: 0 0 0 1px var(--el-border-color) inset;

    &:hover {
      box-shadow: 0 0 0 1px var(--app-text-tertiary) inset;
    }

    &.is-focus {
      box-shadow: 0 0 0 1px var(--app-accent) inset;
    }
  }

  .el-input__inner {
    font-size: 14px;
    color: var(--app-text-primary);

    &::placeholder {
      color: var(--app-text-tertiary);
    }
  }
}

// 文本域样式
:deep(.el-textarea) {
  .el-textarea__inner {
    font-family: inherit;
    font-size: 14px;
    line-height: 1.6;
    border-radius: 6px;
    padding: 10px 12px;
    transition: all 0.3s cubic-bezier(0.25, 0.1, 0.25, 1);

    &::placeholder {
      color: var(--app-text-tertiary);
    }

    &:hover {
      border-color: var(--app-text-tertiary);
    }

    &:focus {
      border-color: var(--app-accent);
      box-shadow: 0 0 0 2px var(--app-accent-bg);
    }
  }
}

// 开关组件样式
:deep(.el-switch) {
  --el-switch-on-color: var(--app-accent);
  --el-switch-off-color: var(--el-border-color);

  .el-switch__core {
    border-radius: 12px;
    transition: all 0.3s cubic-bezier(0.25, 0.1, 0.25, 1)
  }
}

// 复选框样式
:deep(.el-checkbox) {
  .el-checkbox__label {
    font-size: 13px;
    color: var(--app-text-secondary);
  }

  .el-checkbox__input.is-checked + .el-checkbox__label {
    color: var(--app-accent);
  }
}

// 对话框底部按钮样式
.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;

  .el-button {
    min-width: 80px;
    border-radius: 6px;
    font-weight: 500;
    transition: all 0.3s cubic-bezier(0.25, 0.1, 0.25, 1);

    &--primary {
      box-shadow: var(--app-shadow-sm);

      &:hover {
        box-shadow: var(--app-shadow-md);
        transform: translateY(-1px);
      }

      &:active {
        transform: translateY(0);
      }
    }

    &--default {
      &:hover {
        border-color: var(--app-accent);
        color: var(--app-accent);
      }
    }
  }
}

// =====================
// 减少动画模式适配
// =====================
@media (prefers-reduced-motion: reduce) {
  :deep(.el-dialog__close) {
    transition: none;

    &:hover {
      transform: none;
    }
  }

  .dialog-footer .el-button {
    transition: none;

    &:hover {
      transform: none;
    }
  }
}
</style>
