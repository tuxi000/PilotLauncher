import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Tauri 推荐的 Vite 配置：固定端口、不清屏、产物面向 WebView2（Chromium）
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    target: 'chrome105',
    minify: 'esbuild',
    sourcemap: false,
    outDir: 'dist',
    emptyOutDir: true,
  },
})
