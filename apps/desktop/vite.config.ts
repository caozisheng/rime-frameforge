import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  server: { host: '127.0.0.1', port: 1420, strictPort: true },
  plugins: [react()],
  worker: { format: 'es' },
  resolve: {
    alias: {
      '@rime/web-runtime': new URL('../../web/src', import.meta.url).pathname,
    },
  },
});
