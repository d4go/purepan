<template>
  <div class="app-shell">
    <TitleBar :maximizable="true" />
    <el-container class="main-layout">
    <!-- 侧边栏 -->
    <el-aside width="240px" class="sidebar glass-sidebar">
      <!-- 用户信息区（点击查看个人信息） -->
      <div class="sidebar__user" @click="profileDialogVisible = true">
        <el-avatar :src="authStore.avatar" :size="40" class="sidebar__user-avatar">
          <el-icon><UserFilled /></el-icon>
        </el-avatar>
        <div class="sidebar__user-info">
          <span class="sidebar__user-name">{{ authStore.username || '未登录' }}</span>
          <span class="sidebar__user-status">
            <span class="status-dot"></span>
            已连接
          </span>
        </div>
      </div>

      <!-- 导航菜单 -->
      <el-menu
          :default-active="activeMenu"
          class="sidebar-menu"
          router
      >
        <!-- 核心功能 -->
        <el-menu-item index="/files">
          <el-icon><Files /></el-icon>
          <template #title>网盘管理</template>
        </el-menu-item>

        <el-menu-item index="/local">
          <el-icon><Folder /></el-icon>
          <template #title>本地文件</template>
        </el-menu-item>

        <!-- 传输 -->
        <el-menu-item index="/downloads">
          <el-icon><Download /></el-icon>
          <template #title>下载管理</template>
        </el-menu-item>

        <el-menu-item index="/uploads">
          <el-icon><Upload /></el-icon>
          <template #title>上传管理</template>
        </el-menu-item>

        <el-menu-item index="/transfers">
          <el-icon><Share /></el-icon>
          <template #title>转存管理</template>
        </el-menu-item>

        <el-menu-item index="/cloud-dl">
          <el-icon><Link /></el-icon>
          <template #title>离线下载</template>
        </el-menu-item>

        <!-- 同步与备份 -->
        <el-menu-item index="/share-sync">
          <el-icon><Connection /></el-icon>
          <template #title>分享同步</template>
        </el-menu-item>

        <el-menu-item index="/autobackup">
          <el-icon><Refresh /></el-icon>
          <template #title>自动备份</template>
        </el-menu-item>

        <el-menu-item index="/shares">
          <el-icon><Share /></el-icon>
          <template #title>分享管理</template>
        </el-menu-item>

        <!-- 设置 -->
        <el-menu-item index="/settings">
          <el-icon><Setting /></el-icon>
          <template #title>系统设置</template>
        </el-menu-item>
      </el-menu>

      <!-- 存储空间卡片 -->
      <div class="sidebar__storage" v-if="storageTotal > 0">
        <div class="storage-header">
          <span class="storage-title">存储空间</span>
        </div>
        <div class="storage-usage">
          <span class="storage-used">{{ formatFileSize(storageUsed) }}</span>
          <span class="storage-separator">/</span>
          <span class="storage-total">{{ formatFileSize(storageTotal) }}</span>
        </div>
        <div class="storage-progress">
          <div class="storage-progress-bar" :style="{ width: storagePercent + '%' }"></div>
        </div>
      </div>
    </el-aside>

    <!-- 主内容区 -->
    <el-container class="main-container">
      <!-- 内容区 -->
      <el-main class="main-content">
        <router-view v-slot="{ Component }">
          <transition name="fade-slide" mode="out-in" @after-enter="handleViewEntered">
            <component :is="Component" />
          </transition>
        </router-view>
      </el-main>
    </el-container>

    <!-- 个人信息弹窗 -->
    <UserProfileDialog v-model="profileDialogVisible" :user="authStore.currentUser" />
  </el-container>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessageBox, ElMessage } from 'element-plus'
import { useAuthStore } from '@/stores/auth'
import { useWebAuthStore } from '@/stores/webAuth'
import UserProfileDialog from '@/components/UserProfileDialog.vue'
import TitleBar from '@/components/TitleBar.vue'
import { transitionToMain, transitionToLogin } from '@/utils/windowManager'
import { resetAuthCheckFlags } from '@/router'
import {
  FolderOpened,
  Folder,
  Files,
  Download,
  Upload,
  Setting,
  Share,
  Refresh,
  Link,
  Connection,
  UserFilled,
} from '@element-plus/icons-vue'

const route = useRoute()
const router = useRouter()
const authStore = useAuthStore()
const webAuthStore = useWebAuthStore()

// 状态
const profileDialogVisible = ref(false)

// 计算属性
const activeMenu = computed(() => route.path)

// 存储空间信息
const storageTotal = computed(() => authStore.currentUser?.total_space ?? 0)
const storageUsed = computed(() => authStore.currentUser?.used_space ?? 0)
const storagePercent = computed(() => {
  if (storageTotal.value <= 0) return 0
  return Math.min(100, Math.round((storageUsed.value / storageTotal.value) * 100))
})

// 格式化文件大小
function formatFileSize(bytes: number): string {
  if (!bytes || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  const value = bytes / Math.pow(1024, i)
  if (i >= 3) {
    return value.toFixed(2) + ' ' + units[i]
  }
  return value.toFixed(1) + ' ' + units[i]
}

// 路由切换进入动画结束后，触发一次 resize
function handleViewEntered() {
  window.dispatchEvent(new Event('resize'))
}

// 退出百度账号
async function handleLogout() {
  try {
    await ElMessageBox.confirm('确定要退出当前账号吗？', '退出确认', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    })
    await authStore.logout()
    // 重置路由守卫标志，确保能正确跳转
    resetAuthCheckFlags()
    ElMessage.success('已退出当前账号')
    // 先切换窗口尺寸，再跳转路由
    await transitionToLogin()
    // 使用 replace 避免历史记录堆积
    await router.replace('/login')
  } catch (error) {
    if (error !== 'cancel') {
      console.error('退出登录失败:', error)
    }
  }
}

// Web 认证登出
async function handleWebLogout() {
  try {
    await ElMessageBox.confirm('确定要退出 Web 认证吗？退出后需要重新登录才能访问。', '退出确认', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    })
    await webAuthStore.logout()
    // 重置路由守卫标志，确保能正确跳转
    resetAuthCheckFlags()
    ElMessage.success('已退出 Web 认证')
    // 先切换窗口尺寸，再跳转路由
    await transitionToLogin()
    // 使用 replace 避免历史记录堆积
    await router.replace('/web-login')
  } catch (error) {
    if (error !== 'cancel') {
      console.error('退出 Web 认证失败:', error)
    }
  }
}

onMounted(async () => {
  // 确保窗口处于主界面尺寸
  await transitionToMain()
})

onUnmounted(() => {
})
</script>

<style scoped lang="scss">
.app-shell {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100vh;
  overflow: hidden;
}

.main-layout {
  flex: 1;
  display: flex;
  flex-direction: row;
  overflow: hidden;
}

.main-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
  background: var(--app-bg);
}

.main-content {
  padding: 0 !important;
  overflow: hidden;
  display: flex;
  flex-direction: column;

  > :deep(*) {
    flex: 1 1 0;
    min-height: 0;
  }
}

.sidebar {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  flex-shrink: 0;
  background: var(--app-surface);
  border-right: 1px solid var(--app-divider);

  // 用户信息区
  .sidebar__user {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 20px 16px 16px;
    cursor: pointer;
    border-radius: var(--app-radius-sm);
    transition: background 0.2s ease;

    &:hover {
      background: var(--app-surface-hover);
    }

    .sidebar__user-avatar {
      flex-shrink: 0;
    }

    .sidebar__user-info {
      display: flex;
      flex-direction: column;
      gap: 2px;
      min-width: 0;
    }

    .sidebar__user-name {
      font-size: 15px;
      font-weight: 600;
      color: var(--app-text-primary);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .sidebar__user-status {
      display: flex;
      align-items: center;
      gap: 4px;
      font-size: 13px;
      color: var(--app-success);

      .status-dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: var(--app-success);
        flex-shrink: 0;
      }
    }
  }

  // 导航菜单
  .sidebar-menu {
    flex: 1;
    border-right: none;
    background: transparent;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 4px 0;

    :deep(.el-menu-item) {
      color: #495057;
      height: 44px;
      line-height: 44px;
      margin: 2px 8px;
      border-radius: var(--app-radius-sm);
      transition: all 0.2s ease;
      font-size: 15px;
      font-weight: 500;

      &:hover {
        background-color: var(--app-surface-hover) !important;
        color: var(--app-text-primary);
      }

      &.is-active {
        background-color: var(--app-accent-bg) !important;
        color: var(--app-accent);
        font-weight: 600;
      }

      .el-icon {
        color: inherit;
        font-size: 20px;
        margin-right: 8px;
      }
    }
  }

  // 存储空间卡片
  .sidebar__storage {
    margin: 12px;
    padding: 16px;
    background: var(--app-surface);
    border-radius: var(--app-radius-lg);
    box-shadow: var(--app-shadow-card);
    border: 1px solid var(--app-border-light);

    .storage-header {
      margin-bottom: 8px;

      .storage-title {
        font-size: 13px;
        color: var(--app-text-secondary);
        font-weight: 500;
      }
    }

    .storage-usage {
      display: flex;
      align-items: baseline;
      gap: 4px;
      margin-bottom: 10px;

      .storage-used {
        font-size: 15px;
        font-weight: 600;
        color: var(--app-text-primary);
      }

      .storage-separator {
        font-size: 13px;
        color: var(--app-text-tertiary);
      }

      .storage-total {
        font-size: 13px;
        color: var(--app-text-secondary);
      }
    }

    .storage-progress {
      height: 6px;
      background: var(--app-surface-active);
      border-radius: 999px;
      overflow: hidden;

      .storage-progress-bar {
        height: 100%;
        background: var(--app-accent);
        border-radius: 999px;
        transition: width 0.3s ease;
      }
    }
  }
}

// =====================
// 过渡动画
// =====================
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.fade-slide-enter-active,
.fade-slide-leave-active {
  transition: all 0.2s ease;
}

.fade-slide-enter-from {
  opacity: 0;
  transform: translateX(-8px) scale(0.99);
}

.fade-slide-leave-to {
  opacity: 0;
  transform: translateX(8px) scale(0.99);
}
</style>
