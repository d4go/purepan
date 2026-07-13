import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  // Tauri 需要禁止 Vite 清屏，避免覆盖 Tauri CLI 输出
  clearScreen: false,
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    }
  },
  build: {
    rollupOptions: {
      output: {
        // 拆分大体积第三方依赖，避免全部打进单个 index chunk（原 ~1.2MB，触发
        // Vite 500KB 警告）。vue 全家桶与 element-plus 各自成块，利于浏览器并行
        // 加载与长效缓存（业务代码改动不会让 vendor chunk 失效）。
        manualChunks: {
          vue: ['vue', 'vue-router', 'pinia'],
          'element-plus': ['element-plus', '@element-plus/icons-vue']
        }
      }
    }
  },
  server: {
    // 固定端口和 host
    port: 5173,
    strictPort: true,
    host: '127.0.0.1',
    // HTTP + WS 代理到后端 Axum 服务器
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:18888',
        changeOrigin: true,
      },
    },
  }
})
