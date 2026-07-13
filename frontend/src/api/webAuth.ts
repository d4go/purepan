/**
 * Web 访问认证 API 客户端
 * 实现登录、刷新、登出、配置管理等 API 调用
 */

import { type AxiosInstance, type AxiosError } from 'axios'
import axios from 'axios'

// ============ 类型定义 ============

/** 认证模式 */
export type AuthMode = 'none' | 'password' | 'totp' | 'password_totp'

/** 登录请求 */
export interface LoginRequest {
  password?: string
  totp_code?: string
  recovery_code?: string
  pending_token?: string
}

/** 登录响应 */
export interface LoginResponse {
  status: 'success' | 'need_totp' | 'error'
  access_token?: string
  refresh_token?: string
  access_expires_at?: number
  refresh_expires_at?: number
  pending_token?: string
  error?: string
  lockout_remaining?: number
}

/** 刷新令牌请求 */
export interface RefreshRequest {
  refresh_token: string
}

/** 刷新令牌响应 */
export interface RefreshResponse {
  access_token: string
  refresh_token: string
  access_expires_at: number
  refresh_expires_at: number
}

/** 认证状态响应 */
export interface AuthStatusResponse {
  enabled: boolean
  mode: AuthMode
  authenticated: boolean
}

/** 认证配置响应 */
export interface AuthConfigResponse {
  enabled: boolean
  mode: AuthMode
  password_set: boolean
  totp_enabled: boolean
  recovery_codes_count: number
}

/** 设置密码请求 */
export interface SetPasswordRequest {
  password: string
  current_password?: string
}

/** TOTP 设置响应 */
export interface TotpSetupResponse {
  secret: string
  qr_code: string
  issuer: string
  account: string
}

/** TOTP 验证请求 */
export interface TotpVerifyRequest {
  code: string
  secret?: string
}

/** TOTP 禁用请求 */
export interface TotpDisableRequest {
  code?: string
  recovery_code?: string
}

/** 重新生成恢复码请求 */
export interface RegenerateCodesRequest {
  totp_code: string
}

/** 重新生成恢复码响应 */
export interface RegenerateCodesResponse {
  codes: string[]
}

/** 更新配置请求 */
export interface UpdateConfigRequest {
  enabled?: boolean
  mode?: AuthMode
}

/** API 错误响应 */
export interface ApiErrorResponse {
  code: number
  error: string
  message: string
  details?: {
    lockout_remaining?: number
  }
}

// ============ 令牌刷新回调类型 ============

export type TokenRefreshCallback = (tokens: RefreshResponse) => void
export type AuthFailureCallback = () => void

// Web 认证专用 HTTP 状态码（与后端保持一致）
// 419 表示 Web 认证令牌过期，区别于 401 百度账号认证失败
const WEB_AUTH_EXPIRED_STATUS = 419

// ============ API 客户端类 ============

class WebAuthApiClient {
  private client: AxiosInstance
  private accessToken: string | null = null
  private refreshToken: string | null = null
  private isRefreshing = false
  private refreshSubscribers: Array<(token: string) => void> = []
  private onTokenRefresh: TokenRefreshCallback | null = null
  private onAuthFailure: AuthFailureCallback | null = null

  constructor() {
    this.client = axios.create({
      baseURL: `${import.meta.env.VITE_API_BASE_URL}/web-auth`,
      timeout: 30000,
      headers: {
        'Content-Type': 'application/json'
      }
    })

    // 路径修正：axios 对以 / 开头的 URL 会忽略 baseURL，导致 Tauri 生产环境请求发到 webview
    // 而非后端。去掉前导 / 使其成为相对路径，让 axios 正确拼接 baseURL（已含 /api/v1/web-auth 前缀）。
    this.client.interceptors.request.use((config) => {
      if (config.url && !config.url.startsWith('/api/') && !config.url.startsWith('http')) {
        config.url = config.url.replace(/^\//, '')
      }
      return config
    })

    this.setupInterceptors()
  }

  /** 设置令牌刷新回调 */
  setTokenRefreshCallback(callback: TokenRefreshCallback) {
    this.onTokenRefresh = callback
  }

  /** 设置认证失败回调 */
  setAuthFailureCallback(callback: AuthFailureCallback) {
    this.onAuthFailure = callback
  }

  /** 设置访问令牌 */
  setAccessToken(token: string | null) {
    this.accessToken = token
  }

  /** 设置刷新令牌 */
  setRefreshToken(token: string | null) {
    this.refreshToken = token
  }

  /** 获取当前访问令牌 */
  getAccessToken(): string | null {
    return this.accessToken
  }

  private setupInterceptors() {
    // 请求拦截器：添加 Authorization header
    this.client.interceptors.request.use(
      (config) => {
        if (this.accessToken) {
          config.headers.Authorization = `Bearer ${this.accessToken}`
        }
        return config
      },
      (error) => Promise.reject(error)
    )

    // 响应拦截器：处理 419 错误和令牌刷新
    this.client.interceptors.response.use(
      (response) => response,
      async (error: AxiosError<ApiErrorResponse>) => {
        const originalRequest = error.config

        // 如果是 419 错误且不是刷新令牌请求
        if (
          error.response?.status === WEB_AUTH_EXPIRED_STATUS &&
          originalRequest &&
          !originalRequest.url?.includes('/refresh') &&
          !originalRequest.url?.includes('/login')
        ) {
          // 如果正在刷新，等待刷新完成
          if (this.isRefreshing) {
            return new Promise((resolve) => {
              this.refreshSubscribers.push((token: string) => {
                if (originalRequest.headers) {
                  originalRequest.headers.Authorization = `Bearer ${token}`
                }
                resolve(this.client(originalRequest))
              })
            })
          }

          // 尝试刷新令牌
          if (this.refreshToken) {
            this.isRefreshing = true

            try {
              const tokens = await this.refreshTokens({ refresh_token: this.refreshToken })
              this.accessToken = tokens.access_token
              this.refreshToken = tokens.refresh_token

              // 通知回调
              if (this.onTokenRefresh) {
                this.onTokenRefresh(tokens)
              }

              // 通知等待的请求
              this.refreshSubscribers.forEach((callback) => callback(tokens.access_token))
              this.refreshSubscribers = []

              // 重试原始请求
              if (originalRequest.headers) {
                originalRequest.headers.Authorization = `Bearer ${tokens.access_token}`
              }
              return this.client(originalRequest)
            } catch (refreshError) {
              // 刷新失败，清除令牌并通知
              this.accessToken = null
              this.refreshToken = null
              this.refreshSubscribers = []

              if (this.onAuthFailure) {
                this.onAuthFailure()
              }

              return Promise.reject(refreshError)
            } finally {
              this.isRefreshing = false
            }
          } else {
            // 没有刷新令牌，通知认证失败
            if (this.onAuthFailure) {
              this.onAuthFailure()
            }
          }
        }

        return Promise.reject(error)
      }
    )
  }

  // ============ 认证 API ============

  /** 登录 */
  async login(request: LoginRequest): Promise<LoginResponse> {
    const response = await this.client.post<LoginResponse>('/login', request)
    return response.data
  }

  /** 刷新令牌 */
  async refreshTokens(request: RefreshRequest): Promise<RefreshResponse> {
    const response = await this.client.post<RefreshResponse>('/refresh', request)
    return response.data
  }

  /** 登出 */
  async logout(): Promise<void> {
    await this.client.post('/logout')
    this.accessToken = null
    this.refreshToken = null
  }

  /** 获取认证状态 */
  async getStatus(): Promise<AuthStatusResponse> {
    const response = await this.client.get<AuthStatusResponse>('/status')
    return response.data
  }

  // ============ 配置 API ============

  /** 获取认证配置 */
  async getConfig(): Promise<AuthConfigResponse> {
    const response = await this.client.get<AuthConfigResponse>('/config')
    return response.data
  }

  /** 更新认证配置 */
  async updateConfig(request: UpdateConfigRequest): Promise<void> {
    await this.client.put('/config', request)
  }

  // ============ 密码管理 API ============

  /** 设置/修改密码 */
  async setPassword(request: SetPasswordRequest): Promise<void> {
    await this.client.post('/password/set', request)
  }

  // ============ TOTP 管理 API ============

  /** 获取 TOTP 设置信息 */
  async setupTotp(): Promise<TotpSetupResponse> {
    const response = await this.client.post<TotpSetupResponse>('/totp/setup')
    return response.data
  }

  /** 验证并启用 TOTP */
  async verifyTotp(request: TotpVerifyRequest): Promise<void> {
    await this.client.post('/totp/verify', request)
  }

  /** 禁用 TOTP */
  async disableTotp(request: TotpDisableRequest): Promise<void> {
    await this.client.post('/totp/disable', request)
  }

  // ============ 恢复码管理 API ============

  /** 重新生成恢复码 */
  async regenerateRecoveryCodes(request: RegenerateCodesRequest): Promise<RegenerateCodesResponse> {
    const response = await this.client.post<RegenerateCodesResponse>('/recovery-codes/regenerate', request)
    return response.data
  }
}

// 导出单例实例
export const webAuthApi = new WebAuthApiClient()

// 导出类型和类
export { WebAuthApiClient }
