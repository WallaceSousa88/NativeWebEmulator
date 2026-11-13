import { defineConfig } from 'vite';

export default defineConfig({
  // Permite que Vite copie e sirva arquivos .wasm como assets estáticos
  assetsInclude: ['**/*.wasm'],

  server: {
    port: 5173,
    // Headers necessários para SharedArrayBuffer e isolamento de origem
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
  },

  // Garante que arquivos .wasm em /public não sejam processados pelo bundler
  optimizeDeps: {
    exclude: [],
  },
});
