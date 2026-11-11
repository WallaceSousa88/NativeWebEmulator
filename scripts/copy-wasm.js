import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const targetDir = path.resolve(__dirname, '../target/wasm32-unknown-unknown/release');
const destDir = path.resolve(__dirname, '../web/public/wasm');

if (!fs.existsSync(destDir)) {
  fs.mkdirSync(destDir, { recursive: true });
}

const wasmFiles = ['step0_mock_core.wasm', 'snes_core.wasm'];

for (const file of wasmFiles) {
  const src = path.join(targetDir, file);
  const dest = path.join(destDir, file);
  if (fs.existsSync(src)) {
    fs.copyFileSync(src, dest);
    console.log(`[copy-wasm] Copied ${file} -> ${dest} (${fs.statSync(dest).size} bytes)`);
  }
}
