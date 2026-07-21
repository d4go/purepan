const DEFAULT_MAX_WAIT_MS = 30_000
const DEFAULT_REQUEST_TIMEOUT_MS = 2_000
const DEFAULT_INITIAL_RETRY_DELAY_MS = 100
const DEFAULT_MAX_RETRY_DELAY_MS = 1_000

export interface BackendReadyOptions {
  /** 最长等待时间；超时后把启动失败交给现有错误页处理。 */
  maxWaitMs?: number
  /** 单次健康检查超时。 */
  requestTimeoutMs?: number
  /** 首次重试间隔。 */
  initialRetryDelayMs?: number
  /** 指数退避上限。 */
  maxRetryDelayMs?: number
  /** 测试注入；生产环境使用 window.fetch。 */
  fetchImpl?: typeof fetch
}

/**
 * 从 API base URL 推导后端健康检查地址。
 *
 * - 生产环境：http://127.0.0.1:18888/api/v1 -> http://127.0.0.1:18888/health
 * - 开发环境：/api/v1 -> /health（由 Vite 代理）
 */
export function backendHealthUrl(apiBaseUrl: string): string {
  const trimmed = apiBaseUrl.replace(/\/+$/, '')
  const backendRoot = trimmed.replace(/\/api\/v1$/, '')
  return `${backendRoot}/health`
}

function delay(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms))
}

/**
 * 等待内嵌 Axum 服务真正开始监听。
 *
 * Tauri 会并行启动 WebView 与后端，而后端绑定端口前还要完成会话加载和管理器
 * 初始化。启动期的认证请求必须经过这个屏障，不能把 connection refused 当成
 * “未登录”。
 */
export async function waitForBackendReady(options: BackendReadyOptions = {}): Promise<void> {
  const maxWaitMs = options.maxWaitMs ?? DEFAULT_MAX_WAIT_MS
  const requestTimeoutMs = options.requestTimeoutMs ?? DEFAULT_REQUEST_TIMEOUT_MS
  const maxRetryDelayMs = options.maxRetryDelayMs ?? DEFAULT_MAX_RETRY_DELAY_MS
  const fetchImpl = options.fetchImpl ?? fetch
  const url = backendHealthUrl(import.meta.env.VITE_API_BASE_URL || '/api/v1')

  const startedAt = Date.now()
  let retryDelayMs = options.initialRetryDelayMs ?? DEFAULT_INITIAL_RETRY_DELAY_MS
  let lastError: unknown

  // 即使 maxWaitMs=0 也执行一次探测，便于调用方做快速检查。
  do {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), requestTimeoutMs)

    try {
      const response = await fetchImpl(url, {
        method: 'GET',
        cache: 'no-store',
        signal: controller.signal,
      })
      if (response.ok) return
      lastError = new Error(`健康检查返回 ${response.status}`)
    } catch (error) {
      lastError = error
    } finally {
      clearTimeout(timeout)
    }

    const elapsedMs = Date.now() - startedAt
    const remainingMs = maxWaitMs - elapsedMs
    if (remainingMs <= 0) break

    await delay(Math.min(retryDelayMs, remainingMs))
    retryDelayMs = Math.min(Math.max(retryDelayMs * 2, 1), maxRetryDelayMs)
  } while (Date.now() - startedAt <= maxWaitMs)

  console.error('[startup] 等待后端就绪超时:', lastError)
  throw new Error(`后端服务在 ${maxWaitMs}ms 内未就绪`)
}
