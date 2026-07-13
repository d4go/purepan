<template>
  <div class="web-login-window">
    <TitleBar :maximizable="false" :close-minimizes="false" />

    <div class="web-login-content">
      <!-- 标题 -->
      <div class="web-login-header">
        <el-icon :size="28" color="var(--app-accent)">
          <Lock />
        </el-icon>
        <h1>Web 访问认证</h1>
        <p class="subtitle">{{ stepSubtitle }}</p>
      </div>

      <!-- 登录表单 -->
      <div class="login-form">
        <!-- 密码输入步骤 -->
        <div v-if="showPasswordStep" class="form-step">
          <el-form @submit.prevent="handlePasswordSubmit">
            <el-form-item>
              <el-input
                ref="passwordInputRef"
                v-model="password"
                type="password"
                placeholder="请输入访问密码"
                size="large"
                show-password
                :disabled="isLoading || isLocked"
                @keyup.enter="handlePasswordSubmit"
                autocomplete="current-password"
              >
                <template #prefix>
                  <el-icon><Lock /></el-icon>
                </template>
              </el-input>
            </el-form-item>

            <el-button
              type="primary"
              size="large"
              :loading="isLoading"
              :disabled="isLocked"
              @click="handlePasswordSubmit"
              class="submit-btn"
            >
              {{ requiresOnlyPassword ? '登录' : '下一步' }}
            </el-button>
          </el-form>
        </div>

        <!-- TOTP 输入步骤 -->
        <div v-if="showTotpStep" class="form-step">
          <div v-if="loginStep === 'totp' && requiresPassword" class="back-link">
            <el-button link type="info" @click="handleBackToPassword">
              <el-icon><ArrowLeft /></el-icon>
              返回密码输入
            </el-button>
          </div>

          <el-form @submit.prevent="handleTotpSubmit">
            <el-form-item>
              <el-input
                ref="totpInputRef"
                v-model="totpCode"
                placeholder="请输入 6 位验证码"
                size="large"
                maxlength="6"
                :disabled="isLoading || isLocked"
                @keyup.enter="handleTotpSubmit"
                inputmode="numeric"
                pattern="[0-9]*"
                autocomplete="one-time-code"
              >
                <template #prefix>
                  <el-icon><Key /></el-icon>
                </template>
              </el-input>
            </el-form-item>

            <el-button
              type="primary"
              size="large"
              :loading="isLoading"
              :disabled="isLocked"
              @click="handleTotpSubmit"
              class="submit-btn"
            >
              验证
            </el-button>

            <div class="recovery-link">
              <el-button link type="primary" @click="showRecoveryInput = true">
                <el-icon><Ticket /></el-icon>
                使用恢复码登录
              </el-button>
            </div>
          </el-form>
        </div>

        <!-- 恢复码输入对话框 -->
        <el-dialog
          v-model="showRecoveryInput"
          title="使用恢复码登录"
          width="360px"
          :close-on-click-modal="false"
        >
          <p class="recovery-hint">请输入您保存的恢复码，每个恢复码只能使用一次</p>
          <el-form @submit.prevent="handleRecoverySubmit">
            <el-form-item>
              <el-input
                ref="recoveryInputRef"
                v-model="recoveryCode"
                placeholder="XXXX-XXXX"
                size="large"
                :disabled="isLoading"
                autocomplete="off"
              >
                <template #prefix>
                  <el-icon><Ticket /></el-icon>
                </template>
              </el-input>
            </el-form-item>
          </el-form>
          <template #footer>
            <el-button @click="showRecoveryInput = false">取消</el-button>
            <el-button type="primary" :loading="isLoading" @click="handleRecoverySubmit">
              验证
            </el-button>
          </template>
        </el-dialog>

        <!-- 错误提示 -->
        <transition name="fade">
          <div v-if="error" class="error-message">
            <el-alert :title="error" type="error" :closable="true" show-icon @close="clearError" />
          </div>
        </transition>

        <!-- 速率限制提示 -->
        <transition name="fade">
          <div v-if="lockoutRemaining && lockoutRemaining > 0" class="lockout-message">
            <el-alert
              :title="`请求过于频繁，请在 ${lockoutRemaining} 秒后重试`"
              type="warning"
              :closable="false"
              show-icon
            />
            <div class="lockout-countdown">
              <el-progress
                :percentage="lockoutProgress"
                :show-text="false"
                :stroke-width="4"
                status="warning"
              />
            </div>
          </div>
        </transition>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { useWebAuthStore } from '@/stores/webAuth'
import { transitionToLogin } from '@/utils/windowManager'
import TitleBar from '@/components/TitleBar.vue'
import { Lock, Key, Ticket, ArrowLeft } from '@element-plus/icons-vue'

const router = useRouter()
const webAuthStore = useWebAuthStore()

// 输入框引用
const passwordInputRef = ref<InstanceType<typeof import('element-plus')['ElInput']> | null>(null)
const totpInputRef = ref<InstanceType<typeof import('element-plus')['ElInput']> | null>(null)
const recoveryInputRef = ref<InstanceType<typeof import('element-plus')['ElInput']> | null>(null)

// 表单状态
const password = ref('')
const totpCode = ref('')
const recoveryCode = ref('')
const showRecoveryInput = ref(false)

// 速率限制倒计时
const lockoutCountdown = ref<number | null>(null)
let lockoutTimer: ReturnType<typeof setInterval> | null = null

// 从 store 获取状态
const loginStep = computed(() => webAuthStore.loginStep)
const isLoading = computed(() => webAuthStore.isLoading)
const error = computed(() => webAuthStore.error)
const lockoutRemaining = computed(() => lockoutCountdown.value ?? webAuthStore.lockoutRemaining)
const requiresPassword = computed(() => webAuthStore.requiresPassword)
const requiresTotp = computed(() => webAuthStore.requiresTotp)
const requiresOnlyPassword = computed(() => requiresPassword.value && !requiresTotp.value)
const requiresOnlyTotp = computed(() => !requiresPassword.value && requiresTotp.value)

const isLocked = computed(() => lockoutRemaining.value !== null && lockoutRemaining.value > 0)

const lockoutProgress = computed(() => {
  if (!lockoutRemaining.value) return 0
  const maxLockout = 900
  return Math.max(0, Math.min(100, (lockoutRemaining.value / maxLockout) * 100))
})

const showPasswordStep = computed(() => {
  return loginStep.value === 'password' && requiresPassword.value
})

const showTotpStep = computed(() => {
  if (loginStep.value === 'totp') return true
  if (loginStep.value === 'password' && requiresOnlyTotp.value) return true
  return false
})

const stepSubtitle = computed(() => {
  if (showPasswordStep.value) return '请输入访问密码'
  if (showTotpStep.value) {
    if (loginStep.value === 'totp') return '请输入双因素验证码'
    return '请输入验证码以访问系统'
  }
  return '请输入凭证以访问系统'
})

function clearError() {
  webAuthStore.$patch({ error: null })
}

function startLockoutCountdown(seconds: number) {
  stopLockoutCountdown()
  lockoutCountdown.value = seconds
  lockoutTimer = setInterval(() => {
    if (lockoutCountdown.value !== null && lockoutCountdown.value > 0) {
      lockoutCountdown.value--
    } else {
      stopLockoutCountdown()
    }
  }, 1000)
}

function stopLockoutCountdown() {
  if (lockoutTimer) {
    clearInterval(lockoutTimer)
    lockoutTimer = null
  }
  lockoutCountdown.value = null
}

function handleBackToPassword() {
  webAuthStore.resetLoginFlow()
  password.value = ''
  totpCode.value = ''
  nextTick(() => {
    passwordInputRef.value?.focus()
  })
}

async function handlePasswordSubmit() {
  if (!password.value) {
    ElMessage.warning('请输入密码')
    return
  }
  if (isLocked.value) return

  try {
    const response = await webAuthStore.loginWithPassword(password.value)
    if (response.status === 'success') {
      ElMessage.success('登录成功')
      router.push('/login')
    } else if (response.status === 'need_totp') {
      nextTick(() => {
        totpInputRef.value?.focus()
      })
    } else if (response.lockout_remaining) {
      startLockoutCountdown(response.lockout_remaining)
    }
  } catch (err: any) {
    const lockout = err.response?.data?.details?.lockout_remaining
    if (lockout) {
      startLockoutCountdown(lockout)
    }
  }
}

async function handleTotpSubmit() {
  if (!totpCode.value || totpCode.value.length !== 6) {
    ElMessage.warning('请输入 6 位验证码')
    return
  }
  if (isLocked.value) return

  try {
    const response = await webAuthStore.verifyTotp(totpCode.value)
    if (response.status === 'success') {
      ElMessage.success('登录成功')
      router.push('/login')
    }
  } catch (err) {
    totpCode.value = ''
  }
}

async function handleRecoverySubmit() {
  if (!recoveryCode.value) {
    ElMessage.warning('请输入恢复码')
    return
  }

  try {
    const response = await webAuthStore.loginWithRecoveryCode(recoveryCode.value)
    if (response.status === 'success') {
      ElMessage.success('登录成功')
      showRecoveryInput.value = false
      router.push('/login')
    }
  } catch (err) {
    recoveryCode.value = ''
  }
}

watch(() => webAuthStore.isAuthenticated, (isAuth) => {
  if (isAuth) {
    router.push('/login')
  }
})

watch(showRecoveryInput, (show) => {
  if (show) {
    nextTick(() => {
      recoveryInputRef.value?.focus()
    })
  } else {
    recoveryCode.value = ''
  }
})

watch(() => webAuthStore.lockoutRemaining, (newVal) => {
  if (newVal && newVal > 0) {
    startLockoutCountdown(newVal)
  }
})

onMounted(async () => {
  await transitionToLogin()
  webAuthStore.initialize()

  try {
    await webAuthStore.checkAuthStatus()
    if (!webAuthStore.isAuthEnabled || webAuthStore.isAuthenticated) {
      router.push('/login')
      return
    }
  } catch (err) {
    console.error('获取认证状态失败:', err)
  }

  webAuthStore.resetLoginFlow()

  nextTick(() => {
    if (showPasswordStep.value) {
      passwordInputRef.value?.focus()
    } else if (showTotpStep.value) {
      totpInputRef.value?.focus()
    }
  })
})

onUnmounted(() => {
  stopLockoutCountdown()
})
</script>

<style scoped lang="scss">
.web-login-window {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100vh;
  background: var(--app-surface);
  overflow: hidden;
}

.web-login-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 20px 24px 16px;
  overflow-y: auto;
}

.web-login-header {
  text-align: center;
  margin-bottom: 20px;

  h1 {
    margin: 8px 0 4px;
    font-size: 16px;
    font-weight: 600;
    color: var(--app-text-primary);
  }

  .subtitle {
    margin: 0;
    font-size: 12px;
    color: var(--app-text-secondary);
  }
}

.login-form {
  .form-step {
    margin-bottom: 16px;
  }

  .back-link {
    margin-bottom: 12px;
    .el-button {
      padding: 0;
      font-size: 13px;
      .el-icon { margin-right: 4px; }
    }
  }

  .submit-btn {
    width: 100%;
    margin-top: 8px;
    height: 40px;
    font-size: 14px;
    border-radius: 8px;
  }

  .recovery-link {
    text-align: center;
    margin-top: 12px;
    .el-button {
      font-size: 13px;
      .el-icon { margin-right: 4px; }
    }
  }

  .error-message,
  .lockout-message {
    margin-top: 12px;
  }

  .lockout-countdown {
    margin-top: 6px;
  }
}

.recovery-hint {
  margin: 0 0 12px;
  font-size: 13px;
  color: var(--app-text-secondary);
  line-height: 1.5;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s cubic-bezier(0.25, 0.1, 0.25, 1);
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
