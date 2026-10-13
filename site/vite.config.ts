import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Sürüm tek kaynaktan: crates/cli/Cargo.toml. Çalışma anında download/latest.json bunu ezer.
const cargo = readFileSync(fileURLToPath(new URL("../crates/cli/Cargo.toml", import.meta.url)), "utf8");
const version = /^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1] ?? "0.0.0";

export default defineConfig({
  plugins: [react()],
  // Göreli yollar: site hem suleyman-kara.github.io/shock-convert/ altında hem de
  // kendi alan adının kökünde aynı derlemeyle çalışır.
  base: "./",
  define: { __APP_VERSION__: JSON.stringify(version) },
});
