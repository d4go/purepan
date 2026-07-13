/**
 * 自定义 axios IPC 适配器
 *
 * 将前端的 axios HTTP 请求通过 Tauri IPC (invoke) 转发到 Rust 侧的 ipc_http 命令，
 * 再由 Rust 代理到本地 Axum 服务器。前端不再直接发 HTTP 请求。
 *
 * 所有的 axios 拦截器（Web Auth 令牌、419 刷新、错误提示、健康监测）逻辑不受影响，
 * 因为本适配器返回的对象结构与 axios 原生 adapter 完全一致。
 */

import axios, { type AxiosAdapter, type AxiosResponse, type InternalAxiosRequestConfig, type AxiosRequestConfig } from 'axios'
import { invoke } from '@tauri-apps/api/core'

/**
 * IPC HTTP 适配器
 *
 * axios 在调用 adapter 前，已经执行了：
 * 1. buildFullPath(baseURL, url) — 合并 baseURL 和 url
 * 2. transformRequest(data) — 序列化请求体（通常转为 JSON 字符串）
 * 3. buildURL(url, params) — 将 params 拼接到 URL 查询串
 *
 * 因此 adapter 收到的 config.url 已经是完整路径（如 /api/v1/auth/qrcode/status?sign=xxx），
 * config.data 可能是 JSON 字符串。
 */
export const ipcAdapter: AxiosAdapter = async (config: InternalAxiosRequestConfig): Promise<AxiosResponse> => {
  // 1. 标准化 URL：去掉 protocol+host（prod 环境 baseURL 可能是绝对地址），只保留 path
  let path = config.url || ''
  if (path.startsWith('http://') || path.startsWith('https://')) {
    try {
      const u = new URL(path)
      path = u.pathname + u.search
    } catch {
      // URL 解析失败，保持原样
    }
  }

  // 1.5 补全 /api/v1 前缀：axios 忽略以 / 开头的 URL 的 baseURL，
  // 导致所有 API path 缺少 /api/v1。在此自动补齐（避免回退到静态文件服务报 405）。
  if (!path.startsWith('/api/')) {
    path = '/api/v1' + path
  }

  // 2. 提取 headers（AxiosHeaders 实例 -> plain object）
  const headers: Record<string, string> = {}
  const rawHeaders = config.headers
  if (rawHeaders) {
    // AxiosHeaders 实例有 toJSON() 方法
    const headerObj =
      typeof (rawHeaders as any).toJSON === 'function'
        ? (rawHeaders as any).toJSON()
        : rawHeaders
    if (headerObj && typeof headerObj === 'object') {
      for (const [key, value] of Object.entries(headerObj)) {
        if (value !== undefined && value !== null && value !== false) {
          headers[key] = String(value)
        }
      }
    }
  }

  // 3. 处理请求体：axios transformRequest 可能已序列化为 JSON 字符串，需还原为对象
  let body: unknown = config.data
  if (typeof body === 'string' && body.length > 0) {
    try {
      body = JSON.parse(body)
    } catch {
      // 非 JSON 字符串，保持原样传递
    }
  }
  // undefined / null 不传 body
  if (body === undefined || body === null) {
    body = undefined
  }

  // 4. 调用 Tauri IPC
  let status: number
  let respBody: unknown
  try {
    const resp = await invoke<{ status: number; body: unknown }>('ipc_http', {
      method: (config.method || 'GET').toUpperCase(),
      path,
      body,
      headers,
    })
    status = resp.status
    respBody = resp.body
  } catch (e) {
    // invoke 失败 = 后端连接错误（服务器未启动 / 端口不可达）
    // 构造 axios-like error（无 response 字段），isBackendDownError 会检测到
    const error: any = {
      response: undefined,
      message: typeof e === 'string' ? e : String(e),
      config,
      isAxiosError: true,
    }
    throw error
  }

  // 5. 构造 axios-like response
  const response: AxiosResponse = {
    data: respBody,
    status,
    statusText: '',
    headers: {},
    config,
  }

  // 6. 非 2xx 状态码：抛出 axios-like error
  //    拦截器靠 error.response.status 判断（如 419 Web Auth 过期）
  if (status < 200 || status >= 300) {
    const error: any = {
      response,
      message: `Request failed with status code ${status}`,
      config,
      isAxiosError: true,
    }
    throw error
  }

  return response
}

/**
 * 便捷工厂：创建带 IPC 适配器的 axios 实例
 *
 * 用法（替代 axios.create）：
 *   const client = ipcAxiosCreate({ baseURL: '/api/v1', timeout: 30000 })
 */
export function ipcAxiosCreate(config?: AxiosRequestConfig): import('axios').AxiosInstance {
  return axios.create({ ...config, adapter: ipcAdapter })
}
