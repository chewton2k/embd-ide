import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte()],
  build: {
    target: 'es2022',
    sourcemap: 'hidden',
    minify: 'esbuild',
    cssMinify: true,
    reportCompressedSize: false,
    /*
     * Code-splitting strategy.
     *
     * Without this, every dependency lands in the single main chunk
     * (10+ MB after minification). Splitting heavy and rarely-used
     * libraries into named vendor chunks gives us:
     *
     *  1. Parallel download / parse — modern HTTP allows the browser
     *     to fetch many small chunks concurrently.
     *  2. Long-term cache stability — vendor chunks change far less
     *     often than app code, so users re-download less between
     *     deploys.
     *  3. Lazy/no-cost loading for features that may never run in a
     *     given session (e.g. PDF rendering, Mermaid diagrams).
     *
     * The grouping is by feature surface rather than by package name
     * (e.g. all CodeMirror packages in one chunk because they're
     * always loaded together as the editor surface).
     */
    rollupOptions: {
      output: {
        manualChunks(id: string) {
          if (!id.includes('node_modules')) return undefined;

          // CodeMirror — editor surface (large, always loaded with editor)
          if (id.includes('@codemirror') ||
              id.includes('codemirror-lang-prolog') ||
              id.includes('codemirror-lang-scheme')) {
            return 'vendor-codemirror';
          }

          // xterm.js — terminal surface (loaded only when terminal opens)
          if (id.includes('@xterm')) {
            return 'vendor-xterm';
          }

          // Tauri SDK + plugins
          if (id.includes('@tauri-apps') || id.includes('@crabnebula/tauri-plugin-drag')) {
            return 'vendor-tauri';
          }

          // PDF rendering
          if (id.includes('pdfjs-dist')) {
            return 'vendor-pdf';
          }

          // Iconify runtime
          if (id.includes('@iconify/svelte')) {
            return 'vendor-iconify';
          }

          // Lucide icons — small per-icon, but the whole set adds up
          if (id.includes('lucide-svelte')) {
            return 'vendor-lucide';
          }

          // Markdown / sanitization stack used by the AI chat renderer
          if (id.includes('marked') || id.includes('dompurify')) {
            return 'vendor-markdown';
          }

          // Svelte runtime + ecosystem. Path is anchored to
          // `/node_modules/svelte/` so it only matches the actual
          // runtime, not packages that happen to ship a `svelte/`
          // sub-directory.
          if (id.includes('/node_modules/svelte/') ||
              id.includes('bits-ui') ||
              id.includes('class-variance-authority') ||
              id.includes('clsx') ||
              id.includes('tailwind-merge')) {
            return 'vendor-svelte';
          }

          // Fallback: every other node_module dep lands in a generic
          // vendor chunk. As new heavy deps are added (anything > 200 KB),
          // give them their own named chunk above so the catch-all
          // doesn't silently re-bloat over time.
          return 'vendor';
        },
      },
    },
    /*
     * Just above the largest legitimate chunk
     * (`vendor-icons-simple` at ~4.7 MB) so the inherently large
     * Iconify collections don't fire warnings, while any FUTURE
     * accidental bloat over 5 MB still surfaces during build.
     *
     * The icon chunks themselves are lazy-loaded (see main.ts) and
     * shipped locally inside the Tauri bundle, so there's no network
     * download cost at runtime — only on-disk and parse-time.
     */
    chunkSizeWarningLimit: 5000,
  },
  esbuild: {
    drop: ['debugger'],
  },
})
