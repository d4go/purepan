import type { Pinia } from 'pinia'
import { setActivePinia } from 'pinia'
import { waitForBackendReady } from '@/utils/backendReady'

export interface StartupResult {
  target: string
}

const MAIN_ROUTES = new Set([
  '/files', '/local', '/downloads', '/uploads', '/transfers',
  '/autobackup', '/share-sync', '/cloud-dl', '/shares', '/settings',
])

function requestedPath(): string {
  const hash = window.location.hash.replace(/^#/, '')
  return hash.split('?')[0] || '/'
}

/**
 * Store、认证检查和 WebSocket 都从首帧之后的后台任务启动。
 * 认证请求失败时降级到登录页，不阻止正式应用加载。
 */
export async function initializeInBackground(pinia: Pinia): Promise<StartupResult> {
  setActivePinia(pinia)

  const [{ useWebAuthStore }, { useAuthStore }, { connectWebSocket }] = await Promise.all([
    import('@/stores/webAuth'),
    import('@/stores/auth'),
    import('@/utils/websocket'),
  ])

  const webAuthStore = useWebAuthStore(pinia)
  const authStore = useAuthStore(pinia)

  // Tauri 的 WebView 和内嵌 Axum 服务并行启动。必须等端口完成绑定后再发起
  // WebSocket / 认证请求，否则会把启动期 connection refused 误判为未登录。
  await waitForBackendReady()

  // WS 自带重连，不需要等待连接结果。
  connectWebSocket()

  // initialize 会同步装载本地令牌；服务器校验和百度账号检查随后并行进行。
  const initializeWebAuth = webAuthStore.initialize().catch((error) => {
    console.warn('[startup] Web 认证 Store 初始化失败:', error)
  })

  const webAuthCheck = initializeWebAuth.then(async () => {
    try {
      await webAuthStore.checkAuthStatus()
    } catch (error) {
      console.warn('[startup] Web 认证状态检查失败:', error)
    }
  })

  const baiduAuthCheck = initializeWebAuth.then(async () => {
    try {
      await authStore.fetchAccountList()
    } catch (listError) {
      console.warn('[startup] 账号列表检查失败，尝试兼容登录态:', listError)
      try {
        await authStore.fetchUserInfo()
      } catch (userError) {
        console.warn('[startup] 百度登录状态检查失败:', userError)
      }
    }
  })

  await Promise.all([webAuthCheck, baiduAuthCheck])

  if (webAuthStore.isAuthEnabled && !webAuthStore.isAuthenticated) {
    return { target: '/web-login' }
  }

  if (!authStore.isLoggedIn) {
    return { target: '/login' }
  }

  const requested = requestedPath()
  return { target: MAIN_ROUTES.has(requested) ? requested : '/files' }
}
