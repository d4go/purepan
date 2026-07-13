/**
 * 窗口尺寸管理工具
 *
 * 在 Tauri 环境下控制窗口大小：
 *   - 登录模式：400 × 600，不可缩放
 *   - 主界面模式：1280 × 860，可缩放，最小 960 × 640
 *
 * 浏览器开发模式下为 no-op。
 */

const LOGIN_W = 400
const LOGIN_H = 600
const MAIN_W = 1280
const MAIN_H = 860
const MAIN_MIN_W = 960
const MAIN_MIN_H = 640

function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/** 获取当前 Tauri 窗口实例（非 Tauri 环境返回 null） */
async function getAppWindow() {
  if (!isTauri()) return null
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    const { LogicalSize } = await import('@tauri-apps/api/dpi')
    return { win: getCurrentWindow(), LogicalSize }
  } catch {
    return null
  }
}

/** 切换到登录窗口尺寸（小窗口、不可缩放） */
export async function transitionToLogin(): Promise<void> {
  const ctx = await getAppWindow()
  if (!ctx) return
  const { win, LogicalSize } = ctx
  try {
    await win.setMaximizable(false)
    await win.setResizable(false)
    await win.setMinSize(undefined)
    await win.setSize(new LogicalSize(LOGIN_W, LOGIN_H))
    // 延迟一小段时间让窗口完成尺寸调整后再居中，避免在 Windows 高 DPI 下位置偏移
    await new Promise<void>(resolve => setTimeout(resolve, 80))
    await win.center()
  } catch (e) {
    console.warn('[windowManager] transitionToLogin failed:', e)
  }
}

/** 切换到主界面窗口尺寸（大窗口、可缩放） */
export async function transitionToMain(): Promise<void> {
  const ctx = await getAppWindow()
  if (!ctx) return
  const { win, LogicalSize } = ctx
  try {
    // 先启用缩放，再设置最小尺寸和大小
    await win.setMaximizable(true)
    await win.setResizable(true)
    await win.setMinSize(new LogicalSize(MAIN_MIN_W, MAIN_MIN_H))
    await win.setSize(new LogicalSize(MAIN_W, MAIN_H))
    // 延迟一小段时间让窗口完成尺寸调整后再居中，避免在 Windows 高 DPI 下位置偏移
    await new Promise<void>(resolve => setTimeout(resolve, 80))
    await win.center()
  } catch (e) {
    console.warn('[windowManager] transitionToMain failed:', e)
  }
}
