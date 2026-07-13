<template>
  <div class="shares-container">
    <!-- 页面标题和操作栏 -->
    <div class="page-header">
      <div class="header-left">
        <h2>我的分享</h2>
        <el-tag :type="activeCountType" size="large">
          {{ total }} 条分享记录
        </el-tag>
      </div>
      <div class="header-right">
        <el-button @click="refreshList" :loading="loading">
          <el-icon><Refresh /></el-icon>
          刷新
        </el-button>
        <el-button
            v-if="selectedIds.length > 0"
            type="danger"
            @click="handleBatchCancel"
        >
          <el-icon><Delete /></el-icon>
          批量取消 ({{ selectedIds.length }})
        </el-button>
      </div>
    </div>

    <!-- PC端表格视图 -->
    <el-table
        :data="shareList"
        v-loading="loading"
        @selection-change="handleSelectionChange"
        @row-contextmenu="handleRowContextMenu"
        class="share-table"
    >
      <el-table-column type="selection" width="50" />
      <el-table-column label="文件" prop="typicalPath" min-width="200">
        <template #default="{ row }">
          <span class="file-name">{{ getFileName(row.typicalPath) }}</span>
        </template>
      </el-table-column>
      <el-table-column label="链接" prop="shortlink" min-width="180">
        <template #default="{ row }">
          <el-link type="primary" :href="row.shortlink" target="_blank">
            {{ row.shortlink }}
          </el-link>
        </template>
      </el-table-column>
      <el-table-column label="浏览" prop="viewCount" width="80" align="center" />
      <el-table-column label="状态" width="100" align="center">
        <template #default="{ row }">
          <el-tag :type="getStatusType(row.status)" size="small">
            {{ getStatusText(row.status) }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="过期时间" width="160">
        <template #default="{ row }">
          {{ formatExpireTime(row.expiredTime) }}
        </template>
      </el-table-column>
      <el-table-column label="操作" width="220" fixed="right">
        <template #default="{ row }">
          <el-button size="small" @click="handleGetDetail(row)" :icon="Key">
            提取码
          </el-button>
          <el-button size="small" @click="handleCopyLink(row)" :icon="Link">
            复制
          </el-button>
          <el-button size="small" type="danger" @click="handleCancel(row)" :icon="Delete">
            取消
          </el-button>
        </template>
      </el-table-column>
      <template #empty>
        <div @contextmenu.prevent></div>
      </template>
    </el-table>



    <!-- 分页 -->
    <div class="pagination-wrapper" v-if="total > 0">
      <el-pagination
          v-model:current-page="currentPage"
          :page-size="pageSize"
          :total="total"
          layout="total, prev, pager, next"
          @current-change="handlePageChange"
      />
    </div>

    <!-- 分享详情对话框 -->
    <el-dialog v-model="detailVisible" title="分享详情" width="400px">
      <div class="share-detail-pc">
        <div class="detail-row">
          <span class="label">链接:</span>
          <span class="value">{{ currentShareRecord?.shortlink }}</span>
        </div>
        <div class="detail-row">
          <span class="label">提取码:</span>
          <span class="value pwd-highlight">{{ currentDetail?.pwd || '无' }}</span>
        </div>
      </div>
      <template #footer>
        <el-button @click="copyDetailLink" :icon="Link">复制链接</el-button>
        <el-button type="primary" @click="copyDetailAll" :icon="DocumentCopy">复制链接和提取码</el-button>
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
        <el-icon :size="14"><Document /></el-icon>
        <span class="truncate">{{ getFileName(contextMenuTarget.typicalPath) }}</span>
      </div>
      <div class="context-menu-divider"></div>
      <div class="context-menu-item" @click="handleContextMenuAction('copy')">
        <el-icon><Link /></el-icon>
        <span>复制链接</span>
      </div>
      <div class="context-menu-item" @click="handleContextMenuAction('detail')">
        <el-icon><Key /></el-icon>
        <span>查看提取码</span>
      </div>
      <div class="context-menu-divider"></div>
      <div class="context-menu-item danger" @click="handleContextMenuAction('cancel')">
        <el-icon><Delete /></el-icon>
        <span>取消分享</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Refresh,
  Delete,
  Key,
  Link,
  DocumentCopy,
} from '@element-plus/icons-vue'
import {
  getShareList,
  getShareDetail,
  cancelShare,
  type ShareRecord,
  type ShareDetailData,
} from '@/api/share'

// 状态
const loading = ref(false)
const shareList = ref<ShareRecord[]>([])
const total = ref(0)

// 请求版本号 — 切账号 / 切页 时丢弃旧请求回包
//
// 防御场景：A 账号 getShareList 在路上 → 用户切到 B 账号 → B 的 list 先回 →
// A 的 list 后回会覆盖 B 视图。后续操作按 B active 路由，可能 404 / 误操作 B 资源。
let shareListRequestVersion = 0

// 详情请求独立版本号 — 同上语义，挡 getShareDetail 旧回包
let shareDetailRequestVersion = 0
const currentPage = ref(1)
const pageSize = 20
const selectedIds = ref<number[]>([])

// 详情弹窗状态
const detailVisible = ref(false)
const currentDetail = ref<ShareDetailData | null>(null)
const currentShareRecord = ref<ShareRecord | null>(null)

// 计算属性
const activeCountType = computed(() => {
  if (total.value === 0) return 'info'
  return 'success'
})

// 获取文件名
function getFileName(path: string): string {
  if (!path) return '未知文件'
  const parts = path.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || path
}

// 获取状态类型
function getStatusType(status: number): 'success' | 'danger' | 'warning' | 'info' {
  switch (status) {
    case 0:
      return 'success'
    case 1:
      return 'danger'
    default:
      return 'warning'
  }
}

// 获取状态文本
function getStatusText(status: number): string {
  switch (status) {
    case 0:
      return '正常'
    case 1:
      return '已失效'
    case 3:
      return '审核中'
    default:
      return '未知'
  }
}

// 格式化过期时间
function formatExpireTime(timestamp: number): string {
  if (!timestamp || timestamp === 0) {
    return '永久有效'
  }

  // 判断时间戳是秒还是毫秒
  // 如果时间戳大于 10000000000（约 2286 年的秒级时间戳），则认为是毫秒
  const isMilliseconds = timestamp > 10000000000
  const date = new Date(isMilliseconds ? timestamp : timestamp * 1000)
  const now = new Date()

  if (date < now) {
    return '已过期'
  }
  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

// 刷新列表
async function refreshList() {
  // 请求版本号防御
  const version = ++shareListRequestVersion
  loading.value = true
  try {
    const data = await getShareList(currentPage.value)
    if (version !== shareListRequestVersion) {
      // 已被新请求覆盖（切账号 / 切页等）→ 丢弃本次回包
      return
    }
    shareList.value = data.list
    total.value = data.total
  } catch (error: any) {
    if (version !== shareListRequestVersion) return
    console.error('获取分享列表失败:', error)
    ElMessage.error(error.message || '获取分享列表失败')
  } finally {
    if (version === shareListRequestVersion) {
      loading.value = false
    }
  }
}

// 页码变化
function handlePageChange(page: number) {
  currentPage.value = page
  refreshList()
}

// 表格选择变化
function handleSelectionChange(selection: ShareRecord[]) {
  selectedIds.value = selection.map(item => item.shareId)
}

// 移动端切换选择
function toggleSelect(shareId: number) {
  const index = selectedIds.value.indexOf(shareId)
  if (index === -1) {
    selectedIds.value.push(shareId)
  } else {
    selectedIds.value.splice(index, 1)
  }
}

// 获取分享详情
async function handleGetDetail(record: ShareRecord) {
  // 详情请求版本号防御
  const version = ++shareDetailRequestVersion
  try {
    currentShareRecord.value = record
    const detail = await getShareDetail(record.shareId)
    if (version !== shareDetailRequestVersion) return
    currentDetail.value = detail
    detailVisible.value = true
  } catch (error: any) {
    if (version !== shareDetailRequestVersion) return
    console.error('获取分享详情失败:', error)
    ElMessage.error(error.message || '获取分享详情失败')
  }
}

// 复制链接
async function handleCopyLink(record: ShareRecord) {
  try {
    await navigator.clipboard.writeText(record.shortlink)
    ElMessage.success('链接已复制到剪贴板')
  } catch {
    ElMessage.error('复制失败，请手动复制')
  }
}

// 复制详情链接
async function copyDetailLink() {
  if (!currentShareRecord.value) return
  try {
    await navigator.clipboard.writeText(currentShareRecord.value.shortlink)
    ElMessage.success('链接已复制到剪贴板')
  } catch {
    ElMessage.error('复制失败，请手动复制')
  }
}

// 复制详情链接和提取码
async function copyDetailAll() {
  if (!currentShareRecord.value || !currentDetail.value) return
  const pwd = currentDetail.value.pwd || ''
  // 使用列表中的完整链接，提取码从详情接口获取
  const link = currentShareRecord.value.shortlink
  const linkWithPwd = pwd ? `${link}?pwd=${pwd}` : link
  const text = pwd
      ? `链接: ${linkWithPwd}\n提取码: ${pwd}`
      : `链接: ${linkWithPwd}`
  try {
    await navigator.clipboard.writeText(text)
    ElMessage.success('链接和提取码已复制到剪贴板')
  } catch {
    ElMessage.error('复制失败，请手动复制')
  }
}

// 取消单个分享
async function handleCancel(record: ShareRecord) {
  try {
    await ElMessageBox.confirm(
        `确定要取消分享 "${getFileName(record.typicalPath)}" 吗？`,
        '取消分享',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
    )
    await cancelShare([record.shareId])
    ElMessage.success('分享已取消')
    refreshList()
  } catch (error: any) {
    if (error !== 'cancel') {
      console.error('取消分享失败:', error)
      ElMessage.error(error.message || '取消分享失败')
    }
  }
}

// 批量取消分享
async function handleBatchCancel() {
  if (selectedIds.value.length === 0) return

  try {
    await ElMessageBox.confirm(
        `确定要取消选中的 ${selectedIds.value.length} 个分享吗？`,
        '批量取消分享',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
    )
    await cancelShare(selectedIds.value)
    ElMessage.success(`已取消 ${selectedIds.value.length} 个分享`)
    selectedIds.value = []
    refreshList()
  } catch (error: any) {
    if (error !== 'cancel') {
      console.error('批量取消分享失败:', error)
      ElMessage.error(error.message || '批量取消分享失败')
    }
  }
}

// 右键菜单
const contextMenuVisible = ref(false)
const contextMenuTarget = ref<ShareRecord | null>(null)
const contextMenuPosition = ref({ x: 0, y: 0 })
const contextMenuStyle = computed(() => ({
  left: `${contextMenuPosition.value.x}px`,
  top: `${contextMenuPosition.value.y}px`
}))

function handleRowContextMenu(row: ShareRecord, _column: unknown, event: MouseEvent) {
  event.preventDefault()
  event.stopPropagation()
  contextMenuTarget.value = row
  // 估算菜单高度（4个菜单项 ≈ 180px），当靠近底部时向上弹出
  const menuHeight = 180
  const x = event.clientX
  const y = event.clientY
  let top = y
  if (y + menuHeight > window.innerHeight) {
    top = Math.max(8, y - menuHeight)
  }
  contextMenuPosition.value = { x, y: top }
  contextMenuVisible.value = true
}

function closeContextMenu() {
  contextMenuVisible.value = false
  contextMenuTarget.value = null
}

async function handleContextMenuAction(action: string) {
  const target = contextMenuTarget.value
  if (!target) return
  closeContextMenu()

  switch (action) {
    case 'copy':
      await handleCopyLink(target)
      break
    case 'detail':
      await handleGetDetail(target)
      break
    case 'cancel':
      await handleCancel(target)
      break
  }
}

// 多账号切换处理器——与 DownloadsView/UploadsView/FilesView/...
// 5 个视图一致：收到 active 变更时重拉当前账号的分享列表。
//
// 同时关闭详情弹窗 + 清空 currentShareRecord/currentDetail，
// 否则切到新账号后弹窗仍可能展示上一账号的链接/提取码（信息混淆 — 本身不会
// 造成跨账号删除，因为 handleGetDetail 已按当前 active 账号路由）。
//
// 递增 shareListRequestVersion 并清空 shareList，
// 让旧账号 in-flight `getShareList` 回包被 refreshList 内部 version check 丢弃，
// 避免覆盖新账号视图。
function handleActiveChanged() {
  shareListRequestVersion++
  shareDetailRequestVersion++ // 丢弃旧账号详情请求回包
  shareList.value = []
  total.value = 0
  detailVisible.value = false
  currentShareRecord.value = null
  currentDetail.value = null
  selectedIds.value = []
  refreshList()
}

// 组件挂载时加载数据
onMounted(() => {
  refreshList()
  window.addEventListener('multi-account:active-changed', handleActiveChanged)
  document.addEventListener('mousedown', handleGlobalClick)
})

onBeforeUnmount(() => {
  window.removeEventListener('multi-account:active-changed', handleActiveChanged)
  document.removeEventListener('mousedown', handleGlobalClick)
})

// 全局点击关闭右键菜单
function handleGlobalClick(event: MouseEvent) {
  if (contextMenuVisible.value) {
    const menuEl = document.querySelector('.shares-container .context-menu')
    if (menuEl && menuEl.contains(event.target as Node)) {
      return
    }
    closeContextMenu()
  }
}
</script>


<style scoped lang="scss">
/* =====================
   容器样式
   ===================== */
.shares-container {
  padding: 20px;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--app-bg);
}

/* =====================
   页面头部
   ===================== */
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
  flex-wrap: wrap;
  gap: 12px;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }
}

.header-right {
  display: flex;
  gap: 8px;
}

/* =====================
   PC端表格样式
   ===================== */
.share-table {
  flex: 1;

  .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

/* =====================
   分页样式
   ===================== */
.pagination-wrapper {
  padding: 16px 0;
  display: flex;
  justify-content: center;
}

/* =====================
   PC端分享详情
   ===================== */
.share-detail-pc {
  .detail-row {
    margin-bottom: 16px;
    display: flex;
    align-items: flex-start;

    &:last-child {
      margin-bottom: 0;
    }
  }

  .label {
    color: var(--el-text-color-secondary);
    width: 70px;
    flex-shrink: 0;
  }

  .value {
    word-break: break-all;
  }

  .pwd-highlight {
    color: var(--el-color-primary);
    font-weight: bold;
    font-size: 18px;
  }
}

:deep(.el-dialog) {
  border-radius: 14px;
  box-shadow: var(--app-shadow-lg);
  .el-dialog__header {
    border-bottom: 1px solid var(--app-divider);
  }
  .el-dialog__footer {
    border-top: 1px solid var(--app-divider);
  }
}

:deep(.el-table) {
  --el-table-border-color: var(--app-divider);
  --el-table-header-bg-color: transparent;
  --el-table-row-hover-bg-color: var(--app-surface-hover);
}

// 右键菜单样式
.context-menu {
  position: fixed;
  background: var(--app-surface);
  border: 1px solid var(--app-divider);
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  padding: 4px 0;
  min-width: 160px;
  z-index: 2000;
  user-select: none;

  .context-menu-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    font-size: 13px;
    font-weight: 500;
    color: var(--app-text-primary);
    border-bottom: 1px solid var(--app-divider);

    .truncate {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      max-width: 200px;
    }
  }

  .context-menu-divider {
    height: 1px;
    background: var(--app-divider);
    margin: 4px 0;
  }

  .context-menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    font-size: 13px;
    color: var(--app-text-primary);
    cursor: pointer;
    transition: background 0.15s ease;

    &:hover {
      background: var(--app-surface-hover);
    }

    &.danger {
      color: var(--el-color-danger);

      &:hover {
        background: var(--el-color-danger-light-9);
      }
    }

    .el-icon {
      font-size: 14px;
      flex-shrink: 0;
    }
  }
}
</style>
