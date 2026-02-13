import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    conditions: ['browser'],
  },
  build: {
    outDir: 'dist',
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:7400',
        changeOrigin: true,
      },
      '/ws': {
        target: 'ws://localhost:7400',
        ws: true,
        changeOrigin: true,
      },
    },
  },
});
