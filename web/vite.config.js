import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

export default defineConfig({
  plugins: [svelte()],
  build: {
    target: 'es2020',
    sourcemap: false,
    reportCompressedSize: false,
    modulePreload: {
      polyfill: false,
    },
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) return undefined
          if (id.includes('jspdf')) return 'pdf-export'
          if (id.includes('html2canvas')) return 'capture-export'
          if (id.includes('@xterm')) return 'terminal'
          if (id.includes('svelte')) return 'svelte-vendor'
          return 'vendor'
        },
        entryFileNames: 'assets/[name]-[hash].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash][extname]',
      },
    },
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3399',
      '/ws': {
        target: 'ws://localhost:3399',
        ws: true,
      },
    },
  },
})
