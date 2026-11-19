import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["src/**/*.test.{ts,tsx}"],
    environment: "jsdom",
    setupFiles: ["./src/__tests__/setup.ts"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/**/*.test.{ts,tsx}", "src/__tests__/**", "src/**/*.d.ts", "src/main.tsx"],
      reporter: ["text", "text-summary", "lcov", "json-summary"],
      reportsDirectory: "coverage/ts",
    },
  },
});
