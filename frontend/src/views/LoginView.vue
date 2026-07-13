<template>
  <div class="login-window">
    <TitleBar :maximizable="false" :close-minimizes="false" />

    <!-- 登录过渡遮罩：已登录用户自动跳转时显示，避免白屏 -->
    <div v-if="isTransitioning" class="login-transition-overlay">
      <el-icon class="is-loading" :size="36"><Loading /></el-icon>
      <p class="transition-text">正在登录...</p>
    </div>

    <div class="login-content">
      <!-- 多账号 add 模式横幅 -->
      <el-alert
        v-if="isAddAccountMode"
        type="info"
        show-icon
        :closable="false"
        class="add-account-banner"
      >
        <template #default>
          <div class="add-account-banner__content">
            <span>添加新账号（已有 <strong>{{ authStore.accounts.length }}</strong> 个）</span>
            <el-button text type="primary" size="small" @click="router.push({ path: '/settings', query: { tab: 'accounts' } })">
              返回
            </el-button>
          </div>
        </template>
      </el-alert>

      <!-- 登录方式切换 Tab -->
      <div class="login-tabs">
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'qrcode' }"
          @click="switchTab('qrcode')"
        >
          <el-icon :size="15"><Camera /></el-icon>
          <span>扫码登录</span>
        </button>
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'cookie' }"
          @click="switchTab('cookie')"
        >
          <el-icon :size="15"><Key /></el-icon>
          <span>Cookie 登录</span>
        </button>
      </div>

      <!-- ===== 二维码区域 ===== -->
      <div v-show="activeTab === 'qrcode'" class="qrcode-section">
        <div v-if="loading" class="qrcode-loading">
          <el-icon class="is-loading" :size="28">
            <Loading />
          </el-icon>
          <p>正在生成二维码...</p>
        </div>

        <div v-else-if="error" class="qrcode-error">
          <el-icon :size="40" color="var(--app-error)">
            <CircleClose />
          </el-icon>
          <p class="error-text">{{ error }}</p>
          <el-button type="primary" size="small" @click="refreshQRCode">
            <el-icon><Refresh /></el-icon>
            重新获取
          </el-button>
        </div>

        <div v-else-if="qrcode" class="qrcode-content">
          <!-- 二维码图片 -->
          <div class="qrcode-image">
            <img :src="qrcodeUrl" alt="登录二维码" />

            <!-- 过期遮罩 -->
            <div v-if="isExpired" class="qrcode-mask expired">
              <el-icon :size="36" color="#ffffff">
                <RefreshRight />
              </el-icon>
              <p>二维码已过期</p>
              <el-button type="primary" size="small" @click="refreshQRCode">
                刷新二维码
              </el-button>
            </div>

            <!-- 扫描成功遮罩 -->
            <div v-else-if="isScanned" class="qrcode-mask scanned">
              <el-icon :size="40" color="var(--app-success)" class="scan-pulse-icon">
                <SuccessFilled />
              </el-icon>
              <div class="confirm-prompt">
                <el-icon :size="16"><Iphone /></el-icon>
                <span>请在手机上确认登录</span>
              </div>
            </div>
          </div>

          <!-- 扫码提示 -->
          <div class="scan-tip" :class="{ 'scan-tip--scanned': isScanned }">
            <template v-if="!isScanned">
              <el-icon :size="16" color="var(--app-accent)"><Camera /></el-icon>
              <span>请使用百度 APP 扫描二维码登录</span>
            </template>
            <template v-else>
              <el-icon :size="16" color="var(--app-success)"><Iphone /></el-icon>
              <span>请在手机百度 APP 中点击"确认登录"</span>
            </template>
          </div>

          <!-- 倒计时 -->
          <div class="countdown">
            <span>{{ countdown }}秒后自动刷新</span>
          </div>
        </div>
      </div>

      <!-- ===== Cookie 登录区域 ===== -->
      <div v-show="activeTab === 'cookie'" class="cookie-section">
        <!-- 可折叠的 Cookie 获取说明 -->
        <div class="cookie-tips" :class="{ expanded: cookieTipsExpanded }">
          <div class="cookie-tips__header" @click="cookieTipsExpanded = !cookieTipsExpanded">
            <el-icon :size="14" color="var(--app-accent)"><InfoFilled /></el-icon>
            <span>如何获取 Cookie？</span>
            <el-icon class="cookie-tips__arrow" :class="{ rotated: cookieTipsExpanded }">
              <ArrowDown />
            </el-icon>
          </div>
          <div v-show="cookieTipsExpanded" class="cookie-tips__body">
            <ol>
              <li>浏览器打开 <strong>pan.baidu.com</strong> 并登录</li>
              <li>按 <strong>F12</strong> → <strong>Network</strong> 标签页</li>
              <li>刷新页面，点击任意请求</li>
              <li>在 <strong>Headers → Request Headers</strong> 中找到 <code>cookie</code></li>
              <li>复制完整值，粘贴到下方输入框</li>
            </ol>
          </div>
        </div>

        <!-- Cookie 输入框 -->
        <el-input
          v-model="cookieInput"
          type="textarea"
          :rows="4"
          placeholder="粘贴完整 Cookie，例如：BDUSS=xxxx; PTOKEN=yyyy; STOKEN=zzzz"
          resize="none"
          :disabled="cookieLoading"
        />

        <!-- 错误信息 -->
        <div v-if="cookieError" class="cookie-error">
          <el-icon :size="13" color="var(--app-error)"><CircleClose /></el-icon>
          <span>{{ cookieError }}</span>
        </div>

        <!-- 登录按钮 -->
        <el-button
          type="primary"
          size="large"
          :loading="cookieLoading"
          :disabled="!cookieInput.trim()"
          class="cookie-login-btn"
          @click="loginWithCookie"
        >
          <el-icon v-if="!cookieLoading"><Key /></el-icon>
          {{ cookieLoading ? '登录中...' : '使用 Cookie 登录' }}
        </el-button>
      </div>

      <!-- 底部 -->
      <div class="login-footer">
        <span>v0.1.0</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { ElMessage } from 'element-plus'
import { useAuthStore } from '@/stores/auth'
import { transitionToLogin, transitionToMain } from '@/utils/windowManager'
import TitleBar from '@/components/TitleBar.vue'
import {
  Loading,
  CircleClose,
  Refresh,
  RefreshRight,
  Camera,
  SuccessFilled,
  Warning,
  InfoFilled,
  ArrowDown,
  Key,
  Iphone,
} from '@element-plus/icons-vue'

const router = useRouter()
const route = useRoute()
const authStore = useAuthStore()

// 多账号"添加账号"模式
const isAddAccountMode = computed(() => route.query.mode === 'add')

// 登录方式
const activeTab = ref<'qrcode' | 'cookie'>('qrcode')

// 状态
const loading = ref(false)
const error = ref('')
const qrcode = computed(() => authStore.qrcode)
const isScanned = ref(false)
const isExpired = ref(false)
const countdown = ref(120)

// Cookie 登录状态
const cookieInput = ref('')
const cookieLoading = ref(false)
const cookieError = ref('')
const cookieTipsExpanded = ref(false)

// 登录过渡状态（已登录用户自动跳转时显示 loading 避免白屏）
const isTransitioning = ref(false)

// 计算二维码URL
const qrcodeUrl = computed(() => {
  if (!qrcode.value) return ''
  return qrcode.value.image_base64
})

// 倒计时定时器
let countdownTimer: number | null = null

function startCountdown() {
  countdown.value = 120
  isExpired.value = false
  if (countdownTimer) clearInterval(countdownTimer)
  countdownTimer = window.setInterval(() => {
    countdown.value--
    if (countdown.value <= 0) {
      stopCountdown()
      isExpired.value = true
      authStore.stopPolling()
    }
  }, 1000)
}

function stopCountdown() {
  if (countdownTimer) {
    clearInterval(countdownTimer)
    countdownTimer = null
  }
}

// 生成二维码
async function generateQRCode() {
  loading.value = true
  error.value = ''
  isScanned.value = false
  isExpired.value = false

  try {
    await authStore.generateQRCode()
    authStore.startPolling(
      async () => {
        stopCountdown()
        try {
          await authStore.fetchAccountList()
        } catch (e) {
          console.warn('[LoginView] 登录成功但刷新账号列表失败:', e)
        }
        if (isAddAccountMode.value) {
          ElMessage.success('账号添加成功')
          router.push({ path: '/settings', query: { tab: 'accounts' } })
        } else {
          ElMessage.success('登录成功')
          await transitionToMain()
          router.push('/files')
        }
      },
      (err: any) => {
        error.value = err.message || '登录失败，请重试'
        stopCountdown()
      },
      () => {
        isScanned.value = true
      },
      isAddAccountMode.value
    )
    startCountdown()
  } catch (err: any) {
    error.value = err.message || '生成二维码失败，请重试'
  } finally {
    loading.value = false
  }
}

async function refreshQRCode() {
  authStore.stopPolling()
  stopCountdown()
  await generateQRCode()
}

function switchTab(tab: 'qrcode' | 'cookie') {
  if (activeTab.value === tab) return
  activeTab.value = tab
  cookieError.value = ''
  if (tab === 'cookie') {
    authStore.stopPolling()
    stopCountdown()
  } else {
    generateQRCode()
  }
}

async function loginWithCookie() {
  cookieError.value = ''
  if (!cookieInput.value.trim()) return

  cookieLoading.value = true
  try {
    const result = await authStore.loginWithCookies(cookieInput.value.trim())
    if (result.message && !result.message.includes('预热完成')) {
      ElMessage({
        type: 'warning',
        message: result.message,
        duration: 8000,
        showClose: true,
      })
    } else {
      ElMessage.success(isAddAccountMode.value ? '账号添加成功' : 'Cookie 登录成功')
    }
    try {
      await authStore.fetchAccountList()
    } catch (e) {
      console.warn('[LoginView] Cookie 登录成功但刷新账号列表失败:', e)
    }
    if (isAddAccountMode.value) {
      router.push({ path: '/settings', query: { tab: 'accounts' } })
    } else {
      await transitionToMain()
      router.push('/files')
    }
  } catch (err: any) {
    cookieError.value = err.message || 'Cookie 登录失败，请检查 Cookie 是否完整有效'
  } finally {
    cookieLoading.value = false
  }
}

onMounted(async () => {
  // 确保窗口处于登录尺寸（窗口初始不可见，等页面加载完成后再由 transitionToLogin 显示）
  await transitionToLogin()

  if (!isAddAccountMode.value && authStore.isLoggedIn) {
    // 已登录用户直接跳转
    isTransitioning.value = true
    await transitionToMain()
    router.push('/files')
    return
  }

  await generateQRCode()
})

onUnmounted(() => {
  authStore.stopPolling()
  stopCountdown()
})
</script>

<style scoped lang="scss">
.login-window {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100vh;
  background: var(--app-surface);
  overflow: hidden;
  position: relative;
}

// 登录过渡遮罩
.login-transition-overlay {
  position: absolute;
  inset: 0;
  z-index: 1000;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  background: var(--app-surface);
  animation: fadeIn 0.2s ease;

  .transition-text {
    margin: 0;
    font-size: 14px;
    color: var(--app-text-secondary);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
}

.login-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 16px 24px 12px;
  overflow-y: auto;
  overflow-x: hidden;
}

// ===== Add Account Banner =====
.add-account-banner {
  margin-bottom: 12px;
  flex-shrink: 0;

  &__content {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
}

// ===== Tabs =====
.login-tabs {
  display: flex;
  gap: 4px;
  margin-bottom: 16px;
  padding: 3px;
  border-radius: 10px;
  background: var(--app-surface-active);
  flex-shrink: 0;
}

.tab-btn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding: 7px 0;
  border: none;
  background: transparent;
  font-size: 13px;
  font-weight: 500;
  color: var(--app-text-secondary);
  cursor: pointer;
  border-radius: 8px;
  transition: all 0.25s cubic-bezier(0.25, 0.1, 0.25, 1);

  &:hover:not(.active) {
    color: var(--app-text-primary);
    background: var(--app-surface-hover);
  }

  &.active {
    background: var(--app-surface);
    color: var(--app-text-primary);
    font-weight: 600;
    box-shadow: var(--app-shadow-sm);
  }
}

// ===== QR Code Section =====
.qrcode-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
}

.qrcode-loading {
  text-align: center;
  p {
    margin-top: 12px;
    color: var(--app-text-secondary);
    font-size: 13px;
  }
}

.qrcode-error {
  text-align: center;
  .error-text {
    margin: 12px 0;
    color: var(--app-error);
    font-size: 13px;
  }
}

.qrcode-content {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
}

.qrcode-image {
  position: relative;
  width: 200px;
  height: 200px;
  padding: 12px;
  background: var(--app-surface);
  border: 1px solid var(--app-border);
  border-radius: 12px;
  margin-bottom: 14px;
  box-shadow: var(--app-shadow-sm);

  img {
    width: 100%;
    height: 100%;
    display: block;
    border-radius: 4px;
  }

  .qrcode-mask {
    position: absolute;
    top: 0; left: 0; right: 0; bottom: 0;
    border-radius: 12px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;

    &.expired {
      background: rgba(0, 0, 0, 0.75);
      backdrop-filter: blur(4px);
      -webkit-backdrop-filter: blur(4px);
      color: #fff;
      p { margin: 10px 0 14px; font-size: 13px; }
    }

    &.scanned {
      background: var(--app-success-bg);
      backdrop-filter: blur(8px);
      -webkit-backdrop-filter: blur(8px);
      color: var(--app-success);
      animation: fadeIn 0.3s ease;

      .scan-pulse-icon {
        animation: scanPulse 1.5s ease-in-out infinite;
      }

      .success-text {
        margin: 8px 0 6px;
        font-size: 13px;
        font-weight: 500;
        color: var(--app-text-secondary);
      }

      .confirm-prompt {
        display: flex;
        align-items: center;
        gap: 6px;
        font-size: 15px;
        font-weight: 600;
        color: var(--app-success);
      }
    }
  }
}

.scan-tip {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 8px 12px;
  background: var(--app-accent-bg);
  border-radius: 8px;
  font-size: 12px;
  color: var(--app-accent);
  margin-bottom: 8px;
  transition: all 0.3s ease;

  &.scan-tip--scanned {
    padding: 10px 16px;
    font-size: 14px;
    font-weight: 600;
    background: var(--app-success-bg);
    color: var(--app-success);
  }
}

@keyframes fadeIn {
  from { opacity: 0; transform: scale(0.95); }
  to { opacity: 1; transform: scale(1); }
}

@keyframes scanPulse {
  0%, 100% { transform: scale(1); opacity: 1; }
  50% { transform: scale(1.12); opacity: 0.75; }
}

.countdown {
  text-align: center;
  font-size: 12px;
  color: var(--app-text-tertiary);
}

// ===== Cookie Section =====
.cookie-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.cookie-tips {
  background: var(--app-info-bg);
  border: 1px solid var(--app-border-light);
  border-radius: 8px;
  overflow: hidden;

  &__header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    font-size: 12px;
    font-weight: 600;
    color: var(--app-accent);
    cursor: pointer;
    user-select: none;

    span { flex: 1; }
  }

  &__arrow {
    transition: transform 0.25s cubic-bezier(0.25, 0.1, 0.25, 1);
    &.rotated { transform: rotate(180deg); }
  }

  &__body {
    padding: 0 12px 10px;
    ol {
      margin: 0;
      padding-left: 18px;
      font-size: 12px;
      line-height: 1.8;
      color: var(--app-text-secondary);

      strong { color: var(--app-text-primary); }
      code {
        background: var(--app-surface-active);
        border-radius: 3px;
        padding: 1px 4px;
        font-size: 11px;
        color: var(--app-accent);
        font-family: 'SF Mono', 'Fira Code', monospace;
      }
    }
  }
}

.cookie-error {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 8px 10px;
  background: var(--app-error-bg);
  border-radius: 6px;
  font-size: 12px;
  color: var(--app-error);
  line-height: 1.5;
  span { flex: 1; }
}

.cookie-login-btn {
  width: 100%;
  height: 40px;
  font-size: 14px;
  border-radius: 8px;
}

// ===== Footer =====
.login-footer {
  text-align: center;
  font-size: 10px;
  color: var(--app-text-tertiary);
  padding-top: 8px;
  flex-shrink: 0;
}
</style>
