/// <reference types="vitest/config" />
import { defineConfig, type Rollup } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { BaseSequencer, type TestSpecification } from "vitest/node";

// Run test files one after another, sorted by path
class PathSequencer extends BaseSequencer {
    async sort(files: TestSpecification[]) {
        return [...files].sort((a, b) => a.moduleId.localeCompare(b.moduleId));
    }
}

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
    plugins: [
        react({
            babel: {
                plugins: ["babel-plugin-react-compiler"],
            },
        }),
    ],
    resolve: {
        alias: {
            "@": path.resolve(__dirname, "./src"),
        },
    },

    // Second entry point, loaded by the hidden "print" window
    build: {
        rollupOptions: {
            input: {
                main: path.resolve(__dirname, "index.html"),
                report: path.resolve(__dirname, "report.html"),
            },
            output: {
                // Code shared by both entry points is split into its own chunk; name it "commons" instead of after a random module
                chunkFileNames: (chunk: Rollup.PreRenderedChunk) =>
                    chunk.isDynamicEntry
                        ? "assets/[name]-[hash].js"
                        : "assets/commons-[hash].js",
            },
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
        // Pre-transform both entry points (and their imports) while cargo builds, as the React Compiler makes the first load slow
        warmup: {
            clientFiles: ["./src/main.tsx", "./src/report.tsx"],
        },
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
