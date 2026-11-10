import { WasmHost } from './core/WasmHost';
import { WebGL2Renderer } from './renderer/WebGL2Renderer';
import { AudioController } from './audio/AudioController';
import { InputManager } from './input/InputManager';

class EmulatorApplication {
  private wasmHost: WasmHost;
  private renderer!: WebGL2Renderer;
  private audioController: AudioController;
  private inputManager: InputManager;

  // UI Elements
  private canvas: HTMLCanvasElement;
  private audioOverlay: HTMLElement;
  private btnStartAudio: HTMLButtonElement;
  private btnToggleCrt: HTMLButtonElement;
  private btnToggleAudio: HTMLButtonElement;
  private btnFullscreen: HTMLButtonElement;
  private crtStatusLabel: HTMLElement;
  private audioStatusLabel: HTMLElement;

  // Telemetry elements
  private fpsBadge: HTMLElement;
  private valFps: HTMLElement;
  private valFrameTime: HTMLElement;
  private valWasmTime: HTMLElement;
  private valWasmMem: HTMLElement;
  private valInputDevice: HTMLElement;

  // Performance metrics
  private lastTime: number = 0;
  private frameCount: number = 0;
  private fpsUpdateTimer: number = 0;
  private currentFps: number = 60;
  private currentFrameTime: number = 16.6;
  private currentWasmTime: number = 0.1;

  constructor() {
    this.canvas = document.getElementById('emulator-canvas') as HTMLCanvasElement;
    this.audioOverlay = document.getElementById('audio-overlay') as HTMLElement;
    this.btnStartAudio = document.getElementById('btn-start-audio') as HTMLButtonElement;
    this.btnToggleCrt = document.getElementById('btn-toggle-crt') as HTMLButtonElement;
    this.btnToggleAudio = document.getElementById('btn-toggle-audio') as HTMLButtonElement;
    this.btnFullscreen = document.getElementById('btn-fullscreen') as HTMLButtonElement;
    this.crtStatusLabel = document.getElementById('crt-status') as HTMLElement;
    this.audioStatusLabel = document.getElementById('audio-status') as HTMLElement;

    this.fpsBadge = document.getElementById('fps-badge') as HTMLElement;
    this.valFps = document.getElementById('val-fps') as HTMLElement;
    this.valFrameTime = document.getElementById('val-frametime') as HTMLElement;
    this.valWasmTime = document.getElementById('val-wasmtime') as HTMLElement;
    this.valWasmMem = document.getElementById('val-wasmmem') as HTMLElement;
    this.valInputDevice = document.getElementById('val-input-device') as HTMLElement;

    this.wasmHost = new WasmHost();
    this.audioController = new AudioController();
    this.inputManager = new InputManager();

    this.setupUI();
  }

  public async start(): Promise<void> {
    try {
      console.log('[NativeWebEmulator] Loading Rust Wasm Core...');
      await this.wasmHost.init('/wasm/step0_mock_core.wasm');

      const { width, height } = this.wasmHost.getDimensions();
      console.log(`[NativeWebEmulator] Core initialized with dimensions: ${width}x${height}`);

      // Initialize WebGL2 Renderer
      this.renderer = new WebGL2Renderer(this.canvas, width, height);

      // Start main loop
      this.lastTime = performance.now();
      requestAnimationFrame((t) => this.loop(t));
    } catch (err) {
      console.error('[NativeWebEmulator] Initialization failed:', err);
      alert(`Falha ao carregar emulador: ${err}`);
    }
  }

  private setupUI(): void {
    // Start Audio interaction button
    this.btnStartAudio.addEventListener('click', async () => {
      await this.audioController.init();
      await this.audioController.resume();
      this.audioOverlay.classList.add('hidden');
      this.audioStatusLabel.textContent = 'ATIVO';
    });

    // Toggle CRT Shader
    this.btnToggleCrt.addEventListener('click', () => {
      this.renderer.crtEnabled = !this.renderer.crtEnabled;
      this.crtStatusLabel.textContent = this.renderer.crtEnabled ? 'LIGADO' : 'DESLIGADO';
    });

    // Toggle Audio
    this.btnToggleAudio.addEventListener('click', async () => {
      await this.audioController.init();
      await this.audioController.resume();
      const isMuted = this.audioController.toggleMute();
      this.audioStatusLabel.textContent = isMuted ? 'MUTADO' : 'ATIVO';
    });

    // Fullscreen
    this.btnFullscreen.addEventListener('click', () => {
      const container = document.getElementById('canvas-container');
      if (container) {
        if (!document.fullscreenElement) {
          container.requestFullscreen().catch((err) => console.error(err));
        } else {
          document.exitFullscreen().catch((err) => console.error(err));
        }
      }
    });
  }

  private loop(now: number): void {
    const deltaMs = now - this.lastTime;
    this.lastTime = now;

    // 1. Send controller inputs to Wasm
    const inputMask = this.inputManager.getButtonMask(0);
    this.wasmHost.setInput(0, inputMask);

    // 2. Step emulator frame inside Wasm & benchmark
    const wasmStart = performance.now();
    this.wasmHost.stepFrame();
    const wasmElapsed = performance.now() - wasmStart;

    // 3. Zero-Copy Framebuffer upload to WebGL2 Texture
    const pixelView = this.wasmHost.getFramebufferView();
    this.renderer.renderFrame(pixelView, now / 1000.0);

    // 4. Stream audio samples to AudioWorklet
    const audioView = this.wasmHost.getAudioSamplesView();
    if (audioView.length > 0) {
      this.audioController.pushSamples(audioView);
      this.wasmHost.consumeAudioSamples(audioView.length / 2); // Consume stereo pairs
    }

    // 5. Telemetry calculation
    this.frameCount++;
    this.currentFrameTime = deltaMs;
    this.currentWasmTime = wasmElapsed;

    if (now - this.fpsUpdateTimer > 500) {
      this.currentFps = (this.frameCount * 1000) / (now - this.fpsUpdateTimer);
      this.frameCount = 0;
      this.fpsUpdateTimer = now;
      this.updateTelemetryHUD();
    }

    requestAnimationFrame((t) => this.loop(t));
  }

  private updateTelemetryHUD(): void {
    const fpsText = `${this.currentFps.toFixed(1)} FPS`;
    this.fpsBadge.textContent = fpsText;
    this.valFps.textContent = fpsText;
    this.valFrameTime.textContent = `${this.currentFrameTime.toFixed(2)} ms`;
    this.valWasmTime.textContent = `${this.currentWasmTime.toFixed(3)} ms`;
    this.valWasmMem.textContent = `${(this.wasmHost.getMemorySizeBytes() / 1024).toFixed(1)} KB`;
    this.valInputDevice.textContent = this.inputManager.getGamepadStatus();
  }
}

// Bootstrap
window.addEventListener('DOMContentLoaded', () => {
  const app = new EmulatorApplication();
  app.start();
});
