import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

function normalizeBase(base: string | undefined): string {
  const value = (base ?? "/").trim();
  if (!value || value === "/") {
    return "/";
  }
  return `/${value.replace(/^\/+|\/+$/g, "")}/`;
}

export default defineConfig({
  base: normalizeBase(process.env.VITE_BASE),
  plugins: [react()],
  server: {
    port: 5174
  }
});
