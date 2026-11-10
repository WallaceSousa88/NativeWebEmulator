import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const source = path.resolve(__dirname, '../target/wasm32-unknown-unknown/release/step0_mock_core.wasm');
const destDir = path.resolve(__dirname, '../web/public/wasm');
const destination = path.join(destDir, 'step0_mock_core.wasm');

if (!fs.existsSync(destDir)) {
  fs.mkdirSync(destDir, { recursive: true });
}

if (fs.existsSync(source)) {
  fs.copyFileSync(source, destination);
  console.log(`[copy-wasm] Copied ${source} -> ${destination} (${fs.statSync(destination).size} bytes)`);
} else {
  console.error(`[copy-wasm] Source wasm not found at: ${source}`);
  process.exit(1);
}
