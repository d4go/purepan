import { describe, expect, it, vi } from 'vitest'
import { backendHealthUrl, waitForBackendReady } from '../backendReady'

describe('backendHealthUrl', () => {
  it('从生产环境 API 地址推导健康检查地址', () => {
    expect(backendHealthUrl('http://127.0.0.1:18888/api/v1'))
      .toBe('http://127.0.0.1:18888/health')
  })

  it('从开发环境相对 API 地址推导健康检查地址', () => {
    expect(backendHealthUrl('/api/v1')).toBe('/health')
  })
})

describe('waitForBackendReady', () => {
  it('连接失败时退避重试，直到健康检查成功', async () => {
    const fetchImpl = vi.fn<typeof fetch>()
      .mockRejectedValueOnce(new TypeError('Failed to fetch'))
      .mockResolvedValueOnce(new Response('', { status: 502 }))
      .mockResolvedValueOnce(new Response('{"status":"ok"}', { status: 200 }))

    await waitForBackendReady({
      fetchImpl,
      maxWaitMs: 1_000,
      initialRetryDelayMs: 0,
      maxRetryDelayMs: 0,
    })

    expect(fetchImpl).toHaveBeenCalledTimes(3)
    expect(fetchImpl).toHaveBeenLastCalledWith('/health', expect.objectContaining({
      method: 'GET',
      cache: 'no-store',
    }))
  })

  it('等待超过上限时明确失败', async () => {
    const fetchImpl = vi.fn<typeof fetch>()
      .mockRejectedValue(new TypeError('Failed to fetch'))
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined)

    try {
      await expect(waitForBackendReady({ fetchImpl, maxWaitMs: 0 }))
        .rejects.toThrow('后端服务在 0ms 内未就绪')
      expect(fetchImpl).toHaveBeenCalledTimes(1)
    } finally {
      consoleError.mockRestore()
    }
  })
})
