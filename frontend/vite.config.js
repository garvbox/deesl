import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import fs from 'fs'
import path from 'path'

export default defineConfig({
  plugins: [
    vue(),
    {
      name: 'copy-static-index',
      closeBundle() {
        // Copy static index.html after build completes
        const src = path.resolve(__dirname, 'static/index.html')
        const dest = path.resolve(__dirname, '../src/pkg/index.html')
        if (fs.existsSync(src)) {
          fs.copyFileSync(src, dest)
        }
      }
    }
  ],
  base: '/',
  build: {
    outDir: '../src/pkg',
    emptyOutDir: false,
    manifest: 'assets/manifest.json',
    rollupOptions: {
      input: {
        main: path.resolve(__dirname, 'src/main.js')
      }
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:8000',
        changeOrigin: true,
      },
    },
  },
})