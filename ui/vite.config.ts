import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  // The third-party notices (every licence text, about 0.5 MB) are loaded only when Settings, About opens them.
  build: { chunkSizeWarningLimit: 700 },
  test: { include: ['src/**/*.test.ts'] },
});
