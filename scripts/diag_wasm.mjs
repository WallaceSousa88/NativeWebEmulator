import { readFileSync } from 'fs';

const wasm = readFileSync('./web/public/wasm/snes_core.wasm');
const mod = await WebAssembly.instantiate(wasm, { env: {} });
const exp = mod.instance.exports;

console.log('=== Exports snes_* disponíveis ===');
const keys = Object.keys(exp);
const snesKeys = keys.filter(k => k.startsWith('snes_'));
console.log(snesKeys.join('\n'));

const hasTestPattern = snesKeys.includes('snes_render_test_pattern');
console.log('\nsnes_render_test_pattern presente:', hasTestPattern);

console.log('\n=== Inicializando ===');
exp.snes_init();
console.log('snes_init() OK');

const w = exp.snes_get_width();
const h = exp.snes_get_height();
console.log(`Dimensões: ${w}x${h}`);

const fbPtr = exp.snes_get_fb_ptr();
const fbLen = exp.snes_get_fb_len();
const memSize = exp.memory.buffer.byteLength;
console.log(`FB ptr=0x${fbPtr.toString(16)}, len=${fbLen}`);
console.log(`Memória Wasm: ${memSize} bytes (${(memSize/1024).toFixed(0)} KB)`);

if (fbPtr === 0 || fbLen === 0 || fbPtr + fbLen > memSize) {
  console.error('ERRO: ponteiro do framebuffer inválido!');
  process.exit(1);
}

// Lê primeiros bytes ANTES do test pattern
const fb = new Uint8Array(exp.memory.buffer, fbPtr, 16);
console.log('\nFB[0..15] ANTES de test pattern (deve ser preto 00 00 00 ff):');
console.log(Array.from(fb).map(b => b.toString(16).padStart(2,'0')).join(' '));

// Renderiza padrão de teste
if (hasTestPattern) {
  exp.snes_render_test_pattern();
  const fb2 = new Uint8Array(exp.memory.buffer, fbPtr, 16);
  console.log('\nFB[0..15] DEPOIS de snes_render_test_pattern:');
  console.log(Array.from(fb2).map(b => b.toString(16).padStart(2,'0')).join(' '));
  
  // pixel (0,0): R=0, G=0, B=128, A=255 → 00 00 80 ff
  // pixel (1,0): R=1, G=0, B=128, A=255 → 01 00 80 ff
  const expected0 = [0, 0, 128, 255];
  const got0 = Array.from(fb2.slice(0, 4));
  const ok = expected0.every((v, i) => v === got0[i]);
  console.log(`\nPixel(0,0) esperado: [${expected0}]`);
  console.log(`Pixel(0,0) obtido:   [${got0}]`);
  console.log(ok ? '✓ FRAMEBUFFER CORRETO!' : '✗ FRAMEBUFFER ERRADO!');
}

console.log('\n=== Fim do diagnóstico ===');
