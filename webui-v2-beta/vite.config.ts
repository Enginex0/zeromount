import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'

export default defineConfig({
  base: './',
  plugins: [solid()],
  build: {
    target: 'esnext',
    outDir: '../module/webroot-beta',
    emptyOutDir: true,
    minify: 'esbuild',
    rollupOptions: {
      external: ['kernelsu'],
    },
  },
  optimizeDeps: {
    exclude: ['kernelsu'],
  },
  server: {
    port: 5173,
    host: true,
  },
})
