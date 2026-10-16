/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { BaseSequencer, type TestSpecification } from "vitest/node";

// Run test files one after another, sorted by path
class PathSequencer extends BaseSequencer {
    async sort(files: TestSpecification[]) {
        return [...files].sort((a, b) => a.moduleId.localeCompare(b.moduleId));
    }
}

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
    plugins: [react()],
    resolve: {
        alias: {
            "@": path.resolve(__dirname, "./src"),
        },
    },

    test: {
        environment: "jsdom",
        include: ["src/test/**/*.test.{ts,tsx}"],
        setupFiles: ["src/test/setup.ts"],
        fileParallelism: false,
        isolate: false,
        sequence: {
            sequencer: PathSequencer,
        },
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
            ignored: [
                "**/rusqlite_orm/**",
                "**/src-tauri/**",
                "**/src-tauri-macros/**",
            ],
        },
    },
}));
