export interface CoreExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  core_init: () => void;
  core_step_frame: () => void;
  core_get_fb_ptr: () => number;
  core_get_fb_len: () => number;
  core_get_width: () => number;
  core_get_height: () => number;
  core_get_audio_ptr: () => number;
  core_get_audio_samples_available: () => number;
  core_consume_audio_samples: (count: number) => void;
  core_set_input: (port: number, mask: number) => void;
}

export class WasmHost {
  private instance!: WebAssembly.Instance;
  private exports!: CoreExports;
  private width: number = 256;
  private height: number = 224;

  public async init(wasmUrl: string = '/wasm/step0_mock_core.wasm'): Promise<void> {
    const response = await fetch(wasmUrl);
    if (!response.ok) {
      throw new Error(`Failed to fetch Wasm module from ${wasmUrl}: ${response.statusText}`);
    }

    const wasmBinary = await response.arrayBuffer();
    const result = await WebAssembly.instantiate(wasmBinary, {
      env: {},
    });

    this.instance = result.instance;
    this.exports = this.instance.exports as unknown as CoreExports;

    // Initialize core
    this.exports.core_init();
    this.width = this.exports.core_get_width();
    this.height = this.exports.core_get_height();
  }

  public stepFrame(): void {
    this.exports.core_step_frame();
  }

  /**
   * Returns a zero-copy Uint8Array view directly into Wasm linear memory for the framebuffer.
   * Note: The view is only valid until the next Wasm memory growth (which is static for our core).
   */
  public getFramebufferView(): Uint8Array {
    const ptr = this.exports.core_get_fb_ptr();
    const len = this.exports.core_get_fb_len();
    return new Uint8Array(this.exports.memory.buffer, ptr, len);
  }

  /**
   * Returns a zero-copy Float32Array view directly into Wasm linear memory for audio samples.
   */
  public getAudioSamplesView(): Float32Array {
    const ptr = this.exports.core_get_audio_ptr();
    const count = this.exports.core_get_audio_samples_available();
    // Stereo interleaved: count * 2 floats
    return new Float32Array(this.exports.memory.buffer, ptr, count * 2);
  }

  public consumeAudioSamples(count: number): void {
    this.exports.core_consume_audio_samples(count);
  }

  public setInput(port: number, buttonMask: number): void {
    this.exports.core_set_input(port, buttonMask);
  }

  public getDimensions(): { width: number; height: number } {
    return { width: this.width, height: this.height };
  }

  public getMemorySizeBytes(): number {
    return this.exports.memory.buffer.byteLength;
  }
}
