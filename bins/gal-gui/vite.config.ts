import { env } from 'process'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_', 'RUST_'],
  build: {
    target: ['es2021', 'chrome97', 'safari13'],
    minify: !env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!env.TAURI_DEBUG,
    chunkSizeWarningLimit: 1000,
  },
  server: {
    fs: {
      strict: false
    }
  }
})
