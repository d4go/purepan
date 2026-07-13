import { createApp, defineComponent, h, nextTick } from 'vue'
import { createPinia } from 'pinia'

import './styles/main.scss'
import { initTheme } from './composables/useTheme'
import appIconUrl from '../../src-tauri/icons/icon.png'

initTheme()

const pinia = createPinia()

// 首屏不依赖 Element Plus。它先交出一个可绘制帧，正式 UI 和所有启动服务
// 随后再并行加载，避免同步依赖把 Tauri 主窗口卡在白屏阶段。
const StartupShell = defineComponent({
  name: 'StartupShell',
  setup() {
    return () => h('div', { class: 'startup-window' }, [
      h('main', { class: 'startup-shell', 'aria-live': 'polite' }, [
        h('img', {
          class: 'startup-shell__icon',
          src: appIconUrl,
          alt: 'PurePan',
          width: 96,
          height: 96,
        }),
        h('h1', 'PurePan'),
        h('p', '正在启动安全、纯净的网盘体验'),
        h('div', { class: 'startup-shell__loader', role: 'progressbar', 'aria-label': '正在加载' }),
      ]),
    ])
  },
})

const startupApp = createApp(StartupShell)
startupApp.use(pinia)
startupApp.mount('#app')

function afterPaint(): Promise<void> {
  return nextTick().then(() => new Promise(resolve => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  }))
}

async function revealMainWindow(): Promise<void> {
  if (!('__TAURI_INTERNALS__' in window)) return

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('frontend_ready')
  } catch (error) {
    console.warn('[startup] 窗口就绪通知失败，使用前端兜底显示:', error)
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().show()
  }
}

async function start(): Promise<void> {
  // 两者都在首帧后启动：业务初始化不会阻塞 Vue 的首次绘制。
  await afterPaint()
  await revealMainWindow()

  const [{ initializeInBackground }, application] = await Promise.all([
    import('./startup/bootstrap'),
    import('./startup/application'),
  ])

  const startup = await initializeInBackground(pinia)
  await application.prepareApplication(pinia, startup.target)

  // 所有异步组件和路由均已准备好，交换根应用不会产生中间白帧。
  startupApp.unmount()
  application.mountApplication(pinia)
}

void start().catch(async (error) => {
  console.error('[startup] 启动失败:', error)
  const application = await import('./startup/application')
  await application.prepareApplication(pinia, '/login')
  startupApp.unmount()
  application.mountApplication(pinia)
})
