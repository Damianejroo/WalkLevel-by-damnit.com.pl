import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

const wejscie = (plik: string) => fileURLToPath(new URL(plik, import.meta.url));

// Tauri 2: stały port (tauri.conf.json devUrl), bez czyszczenia konsoli (komunikaty cargo).
// Trzy okna = trzy wejścia (docs/contract.md rozdz. 7): główne, powiadomienia nad grą, przycisk.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  preview: { port: 4173, strictPort: true },
  build: {
    target: 'es2023',
    rolldownOptions: {
      input: {
        main: wejscie('./index.html'),
        popup: wejscie('./popup.html'),
        bubble: wejscie('./bubble.html'),
      },
    },
  },
});
