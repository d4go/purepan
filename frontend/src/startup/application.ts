import { createApp, nextTick } from 'vue'
import type { Pinia } from 'pinia'
import { setActivePinia } from 'pinia'
import ElementPlus from 'element-plus'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'

import App from '@/App.vue'
import router, { primeAuthCheckFlags } from '@/router'

let targetPath = '/login'

export async function prepareApplication(pinia: Pinia, target: string): Promise<void> {
  setActivePinia(pinia)
  targetPath = target
  primeAuthCheckFlags()
  await router.replace(targetPath)
}

export function mountApplication(pinia: Pinia): void {
  const app = createApp(App)

  for (const [name, component] of Object.entries(ElementPlusIconsVue)) {
    app.component(name, component)
  }

  app.use(pinia)
  app.use(router)
  app.use(ElementPlus)
  app.mount('#app')

  void nextTick().then(() => {
    window.dispatchEvent(new Event('resize'))
  })
}
