export interface SnesExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  snes_init: () => void;
  snes_alloc: (len: number) => number;
  snes_dealloc: (ptr: number, len: number) => void;
  snes_load_rom: (ptr: number, len: number) => number;
  snes_step_frame: () => void;
  snes_get_fb_ptr: () => number;
  snes_get_fb_len: () => number;
  snes_get_width: () => number;
  snes_get_height: () => number;
  snes_get_audio_ptr: () => number;
  snes_get_audio_samples_available: () => number;
  snes_consume_audio_samples: (count: number) => void;
  snes_set_input: (port: number, mask: number) => void;
  snes_get_rom_type: () => number;
  snes_get_rom_reset_vector: () => number;
  snes_get_pc: () => number;
  snes_get_pbr: () => number;
  snes_get_dbr: () => number;
  snes_get_a: () => number;
  snes_get_x: () => number;
  snes_get_y: () => number;
  snes_get_sp: () => number;
  snes_get_p: () => number;
  snes_get_inidisp: () => number;
  snes_get_last_opcode: () => number;
  snes_render_test_pattern: () => void;
  snes_is_rom_loaded: () => number;
}

export interface RomInfo {
  romType: 'LoROM' | 'HiROM';
  resetVectorHex: string;
  sizeBytes: number;
}

export interface CpuDebugState {
  pc: string;
  pbr: string;
  dbr: string;
  a: string;
  x: string;
  y: string;
  sp: string;
  p: string;
  inidisp: number;
  lastOpcode: string;
}

export class SnesHost {
  private instance!: WebAssembly.Instance;
  private exports!: SnesExports;
  private width: number = 256;
  private height: number = 224;
  private romLoaded: boolean = false;
  private frameCount: number = 0;

  public async init(wasmUrl: string = '/wasm/snes_core.wasm'): Promise<void> {
    console.log(`[SnesHost] Fetching WASM from: ${wasmUrl}`);

    // Tenta instanciação via streaming (mais eficiente, requer MIME correto)
    try {
      const result = await WebAssembly.instantiateStreaming(fetch(wasmUrl), {
        env: {},
      });
      this.instance = result.instance;
      console.log('[SnesHost] instantiateStreaming OK');
    } catch (streamErr) {
      console.warn('[SnesHost] instantiateStreaming falhou, usando fallback arraybuffer:', streamErr);
      // Fallback: carrega como arraybuffer
      const response = await fetch(wasmUrl);
      if (!response.ok) {
        throw new Error(`Falha ao buscar SNES Wasm: ${response.status} ${response.statusText}`);
      }
      const wasmBinary = await response.arrayBuffer();
      console.log(`[SnesHost] WASM carregado: ${wasmBinary.byteLength} bytes`);
      const result = await WebAssembly.instantiate(wasmBinary, { env: {} });
      this.instance = result.instance;
      console.log('[SnesHost] WebAssembly.instantiate OK');
    }

    this.exports = this.instance.exports as unknown as SnesExports;

    // Verifica exports essenciais
    const requiredExports = [
      'snes_init', 'snes_alloc', 'snes_dealloc', 'snes_load_rom',
      'snes_step_frame', 'snes_get_fb_ptr', 'snes_get_fb_len',
      'snes_get_width', 'snes_get_height', 'memory',
    ];
    for (const exp of requiredExports) {
      if (!(exp in this.exports)) {
        throw new Error(`Export faltando no WASM: '${exp}'`);
      }
    }

    console.log('[SnesHost] Todos os exports validados. Chamando snes_init()...');
    this.exports.snes_init();

    this.width = this.exports.snes_get_width();
    this.height = this.exports.snes_get_height();
    const memBytes = this.exports.memory.buffer.byteLength;

    console.log(`[SnesHost] Inicializado: ${this.width}x${this.height}, memória Wasm: ${(memBytes / 1024).toFixed(1)} KB`);

    // Verifica ponteiro do framebuffer logo após init
    const fbPtr = this.exports.snes_get_fb_ptr();
    const fbLen = this.exports.snes_get_fb_len();
    console.log(`[SnesHost] FB ptr=0x${fbPtr.toString(16)}, len=${fbLen} (esperado ${this.width * this.height * 4})`);
  }

  public loadRom(romBuffer: Uint8Array): RomInfo {
    const len = romBuffer.length;
    console.log(`[SnesHost] loadRom: ${len} bytes`);

    const ptr = this.exports.snes_alloc(len);
    console.log(`[SnesHost] snes_alloc(${len}) → ptr=0x${ptr.toString(16)}`);

    if (ptr === 0) {
      throw new Error('snes_alloc retornou null pointer! Memória Wasm insuficiente.');
    }

    // Copia bytes da ROM para a memória Wasm
    const wasmHeap = new Uint8Array(this.exports.memory.buffer, ptr, len);
    wasmHeap.set(romBuffer);

    console.log('[SnesHost] ROM copiada para memória Wasm. Chamando snes_load_rom...');
    const status = this.exports.snes_load_rom(ptr, len);
    this.exports.snes_dealloc(ptr, len);

    console.log(`[SnesHost] snes_load_rom status=${status}`);
    if (status !== 0) {
      throw new Error(`Falha ao parsear cabeçalho SNES (status=${status}). ROM inválida ou formato não suportado.`);
    }

    this.romLoaded = true;
    this.frameCount = 0;

    const romType = this.exports.snes_get_rom_type() === 1 ? 'HiROM' : 'LoROM';
    const resetVector = this.exports.snes_get_rom_reset_vector();
    const inidisp = this.exports.snes_get_inidisp();

    console.log(`[SnesHost] ROM OK! Tipo=${romType}, Reset=$${resetVector.toString(16).toUpperCase()}, INIDISP=0x${inidisp.toString(16)}`);

    // Verifica framebuffer após load
    const fbPtr = this.exports.snes_get_fb_ptr();
    const fbLen = this.exports.snes_get_fb_len();
    console.log(`[SnesHost] Pós-load FB ptr=0x${fbPtr.toString(16)}, len=${fbLen}`);

    return {
      romType,
      resetVectorHex: `$${resetVector.toString(16).toUpperCase().padStart(4, '0')}`,
      sizeBytes: len,
    };
  }

  public isRomLoaded(): boolean {
    return this.romLoaded;
  }

  /** Renderiza um padrão colorido no framebuffer para validar o pipeline WebGL sem ROM. */
  public renderTestPattern(): void {
    this.exports.snes_render_test_pattern();
  }

  public stepFrame(): void {
    if (!this.romLoaded) return;
    this.exports.snes_step_frame();
    this.frameCount++;

    // Log de diagnóstico nos primeiros frames
    if (this.frameCount <= 5 || this.frameCount % 60 === 0) {
      const inidisp = this.exports.snes_get_inidisp();
      const pc = this.exports.snes_get_pc();
      const pbr = this.exports.snes_get_pbr();
      const fbPtr = this.exports.snes_get_fb_ptr();
      const fbLen = this.exports.snes_get_fb_len();

      // Lê primeiros pixels do framebuffer
      let pixelSample = 'N/A';
      if (fbPtr > 0 && fbLen > 0 && fbPtr + fbLen <= this.exports.memory.buffer.byteLength) {
        const fb = new Uint8Array(this.exports.memory.buffer, fbPtr, Math.min(fbLen, 16));
        pixelSample = Array.from(fb.slice(0, 8)).map(b => b.toString(16).padStart(2,'0')).join(' ');
      }

      console.log(
        `[SnesHost] Frame#${this.frameCount} | PC=${pbr.toString(16)}:${pc.toString(16)} | INIDISP=0x${inidisp.toString(16)} | FB[0..7]=${pixelSample} | ptr=0x${fbPtr.toString(16)}`
      );
    }
  }

  public getFramebufferView(): Uint8Array {
    const ptr = this.exports.snes_get_fb_ptr();
    const len = this.exports.snes_get_fb_len();
    const memSize = this.exports.memory.buffer.byteLength;

    if (ptr === 0 || len === 0 || ptr + len > memSize) {
      console.warn(`[SnesHost] getFramebufferView: ponteiro inválido! ptr=0x${ptr.toString(16)}, len=${len}, memSize=${memSize}`);
      // Retorna buffer preto seguro
      return new Uint8Array(this.width * this.height * 4);
    }

    return new Uint8Array(this.exports.memory.buffer, ptr, len);
  }

  public getAudioSamplesView(): Float32Array {
    const ptr = this.exports.snes_get_audio_ptr();
    const count = this.exports.snes_get_audio_samples_available();
    if (ptr === 0 || count === 0) return new Float32Array(0);
    return new Float32Array(this.exports.memory.buffer, ptr, count * 2);
  }

  public consumeAudioSamples(count: number): void {
    this.exports.snes_consume_audio_samples(count);
  }

  public setInput(port: number, buttonMask: number): void {
    this.exports.snes_set_input(port, buttonMask);
  }

  public getDimensions(): { width: number; height: number } {
    return { width: this.width, height: this.height };
  }

  public getMemorySizeBytes(): number {
    return this.exports.memory.buffer.byteLength;
  }

  public getCpuDebugState(): CpuDebugState {
    const pc = this.exports.snes_get_pc().toString(16).toUpperCase().padStart(4, '0');
    const pbr = this.exports.snes_get_pbr().toString(16).toUpperCase().padStart(2, '0');
    const dbr = this.exports.snes_get_dbr().toString(16).toUpperCase().padStart(2, '0');
    const a = this.exports.snes_get_a().toString(16).toUpperCase().padStart(4, '0');
    const x = this.exports.snes_get_x().toString(16).toUpperCase().padStart(4, '0');
    const y = this.exports.snes_get_y().toString(16).toUpperCase().padStart(4, '0');
    const sp = this.exports.snes_get_sp().toString(16).toUpperCase().padStart(4, '0');
    const p = this.exports.snes_get_p().toString(16).toUpperCase().padStart(2, '0');
    const inidisp = this.exports.snes_get_inidisp();
    const lastOpcode = this.exports.snes_get_last_opcode().toString(16).toUpperCase().padStart(2, '0');

    return {
      pc: `$${pbr}:${pc}`,
      pbr: `$${pbr}`,
      dbr: `$${dbr}`,
      a: `$${a}`,
      x: `$${x}`,
      y: `$${y}`,
      sp: `$${sp}`,
      p: `$${p}`,
      inidisp,
      lastOpcode: `$${lastOpcode}`,
    };
  }
}
