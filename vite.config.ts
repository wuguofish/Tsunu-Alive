import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => {
  // 從 package.json 讀取版本號
  const { version } = await import('./package.json');

  return {
  plugins: [vue()],

  // 注入 APP 版本號到前端
  define: {
    __APP_VERSION__: JSON.stringify(version),
  },

  // Tauri 桌面應用不走網路載入，單一 chunk 不影響效能
  build: {
    chunkSizeWarningLimit: 1500,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
};
});
