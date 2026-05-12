import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte()],
  build: {
    target: 'es2022',
    minify: 'esbuild',
    cssMinify: true,
    reportCompressedSize: false,
  },
  esbuild: {
    drop: ['console', 'debugger'],
  },
})
