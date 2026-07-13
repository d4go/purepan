/**
 * Web 访问认证状态管理
 * 实现认证状态管理、令牌存储、自动刷新逻辑
 */

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import {
  webAuthApi,
  type LoginResponse,
  type AuthStatusResponse,
  type AuthConfigResponse,
  type RefreshResponse
} from '@/api/webAuth'

// 本地存储键名
const STORAGE_KEYS = {
  ACCESS_TOKEN: 'web_auth_access_token',
  REFRESH_TOKEN: 'web_auth_refresh_token',
  ACCESS_EXPIRES_AT: 'web_auth_access_expires_at',
  REFRESH_EXPIRES_AT: 'web_auth_refresh_expires_at'
}

// 令牌刷新提前量（5分钟，单位毫秒）
const TOKEN_REFRESH_BUFFER = 5 * 60 * 1000

export const useWebAuthStore = defineStore('webAuth', () => {
  // ============ 状态 ============

  /** 是否已通过 Web 认证 */
  const isAuthenticated = ref(false)

  /** Access Token */
  const accessToken = ref<string | null>(null)

  /** Refresh Token */
  const refreshToken = ref<string | null>(null)

  /** Access Token 过期时间（Unix 时间戳，秒） */
  const accessExpiresAt = ref<number | null>(null)

  /** Refresh Token 过期时间（Unix 时间戳，秒） */
  const refreshExpiresAt = ref<number | null>(null)

  /** 认证配置 */
  const authConfig = ref<AuthStatusResponse | null>(null)

  /** 登录流程状态 */
  const loginStep = ref<'password' | 'totp' | 'complete'>('password')

  /** 密码验证后的临时令牌（用于两步验证） */
  const pendingLoginToken = ref<string | null>(null)

  /** 是否正在加载 */
  const isLoading = ref(false)

  /** 错误信息 */
  const error = ref<string | null>(null)

  /** 速率限制剩余时间（秒） */
  const lockoutRemaining = ref<number | null>(null)

  /** 自动刷新定时器 */
  let refreshTimer: ReturnType<typeof setTimeout> | null = null

  // ============ 计算属性 ============

  /** 是否启用了 Web 认证 */
  const isAuthEnabled = computed(() => authConfig.value?.enabled ?? false)

  /** 当前认证模式 */
  const authMode = computed(() => authConfig.value?.mode ?? 'none')

  /** 是否需要密码 */
  const requiresPassword = computed(() => {
    const mode = authMode.value
    return mode === 'password' || mode === 'password_totp'
  })

  /** 是否需要 TOTP */
  const requiresTotp = computed(() => {
    const mode = authMode.value
    return mode === 'totp' || mode === 'password_totp'
  })

  /** Access Token 是否即将过期（5分钟内） */
  const isAccessTokenExpiringSoon = computed(() => {
    if (!accessExpiresAt.value) return true
    const now = Date.now()
    const expiresAt = accessExpiresAt.value * 1000
    return expiresAt - now < TOKEN_REFRESH_BUFFER
  })

  // ============ 私有方法 ============

  /** 从本地存储加载令牌 */
  async function loadTokensFromStorage() {
    accessToken.value = localStorage.getItem(STORAGE_KEYS.ACCESS_TOKEN)
    refreshToken.value = localStorage.getItem(STORAGE_KEYS.REFRESH_TOKEN)

    const accessExp = localStorage.getItem(STORAGE_KEYS.ACCESS_EXPIRES_AT)
    const refreshExp = localStorage.getItem(STORAGE_KEYS.REFRESH_EXPIRES_AT)

    accessExpiresAt.value = accessExp ? parseInt(accessExp, 10) : null
    refreshExpiresAt.value = refreshExp ? parseInt(refreshExp, 10) : null

    // 同步到 API 客户端
    webAuthApi.setAccessToken(accessToken.value)
    webAuthApi.setRefreshToken(refreshToken.value)

    // 检查令牌是否有效（仅检查本地过期时间，不请求服务器）
    if (accessToken.value && accessExpiresAt.value) {
      const now = Math.floor(Date.now() / 1000)
      if (accessExpiresAt.value > now) {
        // 令牌未过期，假设有效（如果服务器不认可，后续请求会返回 419 自动处理）
        isAuthenticated.value = true
        scheduleTokenRefresh()
      } else if (refreshToken.value && refreshExpiresAt.value && refreshExpiresAt.value > now) {
        // Access Token 过期但 Refresh Token 有效，尝试刷新
        const refreshed = await refreshTokens()
        if (!refreshed) {
          clearTokens()
        }
      } else {
        // 所有令牌都过期，清除
        clearTokens()
      }
    }
  }

  /** 保存令牌到本地存储 */
  function saveTokensToStorage() {
    if (accessToken.value) {
      localStorage.setItem(STORAGE_KEYS.ACCESS_TOKEN, accessToken.value)
    } else {
      localStorage.removeItem(STORAGE_KEYS.ACCESS_TOKEN)
    }

    if (refreshToken.value) {
      localStorage.setItem(STORAGE_KEYS.REFRESH_TOKEN, refreshToken.value)
    } else {
      localStorage.removeItem(STORAGE_KEYS.REFRESH_TOKEN)
    }

    if (accessExpiresAt.value) {
      localStorage.setItem(STORAGE_KEYS.ACCESS_EXPIRES_AT, accessExpiresAt.value.toString())
    } else {
      localStorage.removeItem(STORAGE_KEYS.ACCESS_EXPIRES_AT)
    }

    if (refreshExpiresAt.value) {
      localStorage.setItem(STORAGE_KEYS.REFRESH_EXPIRES_AT, refreshExpiresAt.value.toString())
    } else {
      localStorage.removeItem(STORAGE_KEYS.REFRESH_EXPIRES_AT)
    }
  }

  /** 清除令牌 */
  function clearTokens() {
    accessToken.value = null
    refreshToken.value = null
    accessExpiresAt.value = null
    refreshExpiresAt.value = null
    isAuthenticated.value = false
    pendingLoginToken.value = null
    loginStep.value = 'password'

    // 清除本地存储
    localStorage.removeItem(STORAGE_KEYS.ACCESS_TOKEN)
    localStorage.removeItem(STORAGE_KEYS.REFRESH_TOKEN)
    localStorage.removeItem(STORAGE_KEYS.ACCESS_EXPIRES_AT)
    localStorage.removeItem(STORAGE_KEYS.REFRESH_EXPIRES_AT)

    // 同步到 API 客户端
    webAuthApi.setAccessToken(null)
    webAuthApi.setRefreshToken(null)

    // 清除刷新定时器
    if (refreshTimer) {
      clearTimeout(refreshTimer)
      refreshTimer = null
    }
  }

  /** 设置令牌 */
  function setTokens(response: LoginResponse | RefreshResponse) {
    if ('access_token' in response && response.access_token) {
      accessToken.value = response.access_token
      webAuthApi.setAccessToken(response.access_token)
    }

    if ('refresh_token' in response && response.refresh_token) {
      refreshToken.value = response.refresh_token
      webAuthApi.setRefreshToken(response.refresh_token)
    }

    if ('access_expires_at' in response && response.access_expires_at) {
      accessExpiresAt.value = response.access_expires_at
    }

    if ('refresh_expires_at' in response && response.refresh_expires_at) {
      refreshExpiresAt.value = response.refresh_expires_at
    }

    saveTokensToStorage()
    scheduleTokenRefresh()
  }

  /** 安排令牌刷新 */
  function scheduleTokenRefresh() {
    if (refreshTimer) {
      clearTimeout(refreshTimer)
      refreshTimer = null
    }

    if (!accessExpiresAt.value || !refreshToken.value) return

    const now = Date.now()
    const expiresAt = accessExpiresAt.value * 1000
    // 在过期前 5 分钟刷新
    const refreshAt = expiresAt - TOKEN_REFRESH_BUFFER

    if (refreshAt > now) {
      const delay = refreshAt - now
      refreshTimer = setTimeout(() => {
        refreshTokens()
      }, delay)
    } else {
      // 已经需要刷新
      refreshTokens()
    }
  }

  // ============ 公共方法 ============

  /** 初始化：加载令牌并设置回调 */
  async function initialize() {
    // 设置 API 客户端回调
    webAuthApi.setTokenRefreshCallback((tokens: RefreshResponse) => {
      setTokens(tokens)
    })

    webAuthApi.setAuthFailureCallback(() => {
      clearTokens()
      // 跳转到登录页
      if (!window.location.hash.includes('/web-login')) {
        window.location.hash = '#/web-login'
      }
    })

    // 从本地存储加载令牌并验证
    await loadTokensFromStorage()
  }

  /** 检查认证状态 */
  async function checkAuthStatus(): Promise<AuthStatusResponse> {
    isLoading.value = true
    error.value = null

    try {
      const status = await webAuthApi.getStatus()
      authConfig.value = status

      // 如果认证未启用，直接标记为已认证
      if (!status.enabled) {
        isAuthenticated.value = true
      }

      return status
    } catch (err: any) {
      error.value = err.message || '获取认证状态失败'
      throw err
    } finally {
      isLoading.value = false
    }
  }

  /** 密码登录 */
  async function loginWithPassword(password: string): Promise<LoginResponse> {
    isLoading.value = true
    error.value = null
    lockoutRemaining.value = null

    try {
      const response = await webAuthApi.login({ password })

      if (response.status === 'success') {
        setTokens(response)
        isAuthenticated.value = true
        loginStep.value = 'complete'
      } else if (response.status === 'need_totp') {
        pendingLoginToken.value = response.pending_token || null
        loginStep.value = 'totp'
      } else if (response.status === 'error') {
        error.value = response.error || '登录失败'
        if (response.lockout_remaining) {
          lockoutRemaining.value = response.lockout_remaining
        }
      }

      return response
    } catch (err: any) {
      const errorData = err.response?.data
      error.value = errorData?.message || err.message || '登录失败'
      if (errorData?.details?.lockout_remaining) {
        lockoutRemaining.value = errorData.details.lockout_remaining
      }
      throw err
    } finally {
      isLoading.value = false
    }
  }

  /** TOTP 验证 */
  async function verifyTotp(code: string): Promise<LoginResponse> {
    isLoading.value = true
    error.value = null

    try {
      const response = await webAuthApi.login({
        totp_code: code,
        pending_token: pendingLoginToken.value || undefined
      })

      if (response.status === 'success') {
        setTokens(response)
        isAuthenticated.value = true
        loginStep.value = 'complete'
        pendingLoginToken.value = null
      } else if (response.status === 'error') {
        error.value = response.error || 'TOTP 验证失败'
      }

      return response
    } catch (err: any) {
      error.value = err.response?.data?.message || err.message || 'TOTP 验证失败'
      throw err
    } finally {
      isLoading.value = false
    }
  }

  /** 恢复码登录 */
  async function loginWithRecoveryCode(code: string): Promise<LoginResponse> {
    isLoading.value = true
    error.value = null

    try {
      const response = await webAuthApi.login({
        recovery_code: code,
        pending_token: pendingLoginToken.value || undefined
      })

      if (response.status === 'success') {
        setTokens(response)
        isAuthenticated.value = true
        loginStep.value = 'complete'
        pendingLoginToken.value = null
      } else if (response.status === 'error') {
        error.value = response.error || '恢复码验证失败'
      }

      return response
    } catch (err: any) {
      error.value = err.response?.data?.message || err.message || '恢复码验证失败'
      throw err
    } finally {
      isLoading.value = false
    }
  }

  /** 刷新令牌 */
  async function refreshTokens(): Promise<boolean> {
    if (!refreshToken.value) {
      clearTokens()
      return false
    }

    try {
      const response = await webAuthApi.refreshTokens({ refresh_token: refreshToken.value })
      setTokens(response)
      isAuthenticated.value = true
      return true
    } catch (err) {
      console.error('令牌刷新失败:', err)
      clearTokens()
      return false
    }
  }

  /** 登出 */
  async function logout(): Promise<void> {
    isLoading.value = true

    try {
      await webAuthApi.logout()
    } catch (err) {
      console.error('登出请求失败:', err)
    } finally {
      clearTokens()
      isLoading.value = false
    }
  }

  /** 重置登录流程 */
  function resetLoginFlow() {
    loginStep.value = 'password'
    pendingLoginToken.value = null
    error.value = null
    lockoutRemaining.value = null
  }

  /** 获取认证配置详情 */
  async function getAuthConfig(): Promise<AuthConfigResponse> {
    return await webAuthApi.getConfig()
  }

  return {
    // 状态
    isAuthenticated,
    accessToken,
    refreshToken,
    accessExpiresAt,
    refreshExpiresAt,
    authConfig,
    loginStep,
    pendingLoginToken,
    isLoading,
    error,
    lockoutRemaining,

    // 计算属性
    isAuthEnabled,
    authMode,
    requiresPassword,
    requiresTotp,
    isAccessTokenExpiringSoon,

    // 方法
    initialize,
    checkAuthStatus,
    loginWithPassword,
    verifyTotp,
    loginWithRecoveryCode,
    refreshTokens,
    logout,
    resetLoginFlow,
    getAuthConfig,
    clearTokens
  }
})
