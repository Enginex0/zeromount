import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'
import { readdirSync, readFileSync, writeFileSync } from 'fs'
import { join } from 'path'

// Bundle all locale JSON files into a single file for boot-time inlining
function localeBundlePlugin() {
  return {
    name: 'locale-bundle',
    closeBundle() {
      const localeDir = join(__dirname, 'src/locales');
      const outDir = join(__dirname, '../module/webroot');
      const bundle: Record<string, Record<string, string>> = {};
      for (const file of readdirSync(localeDir)) {
        if (!file.endsWith('.json')) continue;
        const code = file.replace('.json', '');
        bundle[code] = JSON.parse(readFileSync(join(localeDir, file), 'utf-8'));
      }
      writeFileSync(join(outDir, 'locales-bundle.json'), JSON.stringify(bundle));
    },
  };
}

export default defineConfig({
  base: './',
  plugins: [solid(), localeBundlePlugin()],
  build: {
    target: 'esnext',
    outDir: '../module/webroot',
    emptyOutDir: true,
    minify: 'esbuild',
  },
  server: {
    port: 5173,
    host: true,
  },
})
