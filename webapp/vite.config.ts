import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";

export default defineConfig({
  plugins: [solidPlugin()],
  server: {
    port: 3000,
    proxy: {
      "/api": "http://127.0.0.1:5636",
      "/sse": "http://127.0.0.1:5636",
    },
  },
  build: {
    target: "esnext",
    sourcemap: true,
  },
  base: "./",
});
