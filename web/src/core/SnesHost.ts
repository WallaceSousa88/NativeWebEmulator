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
}

export interface RomInfo {
  romType: 'LoROM' | 'HiROM';
  resetVectorHex: string;
  sizeBytes: number;
}

export class SnesHost {
  private instance!: WebAssembly.Instance;
  private exports!: SnesExports;
  private width: number = 256;
  private height: number = 224;
  private romLoaded: boolean = false;

  public async init(wasmUrl: string = '/wasm/snes_core.wasm'): Promise<void> {
    const response = await fetch(wasmUrl);
    if (!response.ok) {
      throw new Error(`Failed to fetch SNES Wasm module: ${response.statusText}`);
    }

    const wasmBinary = await response.arrayBuffer();
    const result = await WebAssembly.instantiate(wasmBinary, {
      env: {},
    });

    this.instance = result.instance;
    this.exports = this.instance.exports as unknown as SnesExports;

    // Initialize core
    this.exports.snes_init();
    this.width = this.exports.snes_get_width();
    this.height = this.exports.snes_get_height();
  }

  public loadRom(romBuffer: Uint8Array): RomInfo {
    const len = romBuffer.length;
    const ptr = this.exports.snes_alloc(len);

    // Copy ROM bytes into Wasm linear memory
    const wasmHeap = new Uint8Array(this.exports.memory.buffer, ptr, len);
    wasmHeap.set(romBuffer);

    const status = this.exports.snes_load_rom(ptr, len);
    this.exports.snes_dealloc(ptr, len);

    if (status !== 0) {
      throw new Error('Falha ao analisar cabeçalho do SNES ou carregar a ROM.');
    }

    this.romLoaded = true;
    const romType = this.exports.snes_get_rom_type() === 1 ? 'HiROM' : 'LoROM';
    const resetVector = this.exports.snes_get_rom_reset_vector();

    return {
      romType,
      resetVectorHex: `$${resetVector.toString(16).toUpperCase().padStart(4, '0')}`,
      sizeBytes: len,
    };
  }

  public isRomLoaded(): boolean {
    return this.romLoaded;
  }

  public stepFrame(): void {
    if (!this.romLoaded) return;
    this.exports.snes_step_frame();
  }

  public getFramebufferView(): Uint8Array {
    const ptr = this.exports.snes_get_fb_ptr();
    const len = this.exports.snes_get_fb_len();
    return new Uint8Array(this.exports.memory.buffer, ptr, len);
  }

  public getAudioSamplesView(): Float32Array {
    const ptr = this.exports.snes_get_audio_ptr();
    const count = this.exports.snes_get_audio_samples_available();
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
}
