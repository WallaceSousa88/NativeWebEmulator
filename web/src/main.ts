import { WasmHost } from './core/WasmHost';
import { SnesHost } from './core/SnesHost';
import { WebGL2Renderer } from './renderer/WebGL2Renderer';
import { AudioController } from './audio/AudioController';
import { InputManager } from './input/InputManager';

class EmulatorApplication {
  private step0Host: WasmHost;
  private snesHost: SnesHost;
  private activeCore: 'snes' | 'step0' = 'snes';

  private renderer!: WebGL2Renderer;
  private audioController: AudioController;
  private inputManager: InputManager;

  // UI Elements
  private canvas: HTMLCanvasElement;
  private canvasContainer: HTMLElement;
  private audioOverlay: HTMLElement;
  private dropOverlay: HTMLElement;
  private selectCore: HTMLSelectElement;
  private fileRomInput: HTMLInputElement;
  private btnLoadRom: HTMLButtonElement;
  private btnStartAudio: HTMLButtonElement;
  private btnToggleCrt: HTMLButtonElement;
  private btnToggleAudio: HTMLButtonElement;
  private btnFullscreen: HTMLButtonElement;
  private crtStatusLabel: HTMLElement;
  private audioStatusLabel: HTMLElement;

  // Cartridge info elements
  private valRomStatus: HTMLElement;
  private valRomType: HTMLElement;
  private valResetVector: HTMLElement;
  private valRomSize: HTMLElement;

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
    this.canvasContainer = document.getElementById('canvas-container') as HTMLElement;
    this.audioOverlay = document.getElementById('audio-overlay') as HTMLElement;
    this.dropOverlay = document.getElementById('drop-overlay') as HTMLElement;
    this.selectCore = document.getElementById('select-core') as HTMLSelectElement;
    this.fileRomInput = document.getElementById('file-rom') as HTMLInputElement;
    this.btnLoadRom = document.getElementById('btn-load-rom') as HTMLButtonElement;
    this.btnStartAudio = document.getElementById('btn-start-audio') as HTMLButtonElement;
    this.btnToggleCrt = document.getElementById('btn-toggle-crt') as HTMLButtonElement;
    this.btnToggleAudio = document.getElementById('btn-toggle-audio') as HTMLButtonElement;
    this.btnFullscreen = document.getElementById('btn-fullscreen') as HTMLButtonElement;
    this.crtStatusLabel = document.getElementById('crt-status') as HTMLElement;
    this.audioStatusLabel = document.getElementById('audio-status') as HTMLElement;

    this.valRomStatus = document.getElementById('val-rom-status') as HTMLElement;
    this.valRomType = document.getElementById('val-rom-type') as HTMLElement;
    this.valResetVector = document.getElementById('val-reset-vector') as HTMLElement;
    this.valRomSize = document.getElementById('val-rom-size') as HTMLElement;

    this.fpsBadge = document.getElementById('fps-badge') as HTMLElement;
    this.valFps = document.getElementById('val-fps') as HTMLElement;
    this.valFrameTime = document.getElementById('val-frametime') as HTMLElement;
    this.valWasmTime = document.getElementById('val-wasmtime') as HTMLElement;
    this.valWasmMem = document.getElementById('val-wasmmem') as HTMLElement;
    this.valInputDevice = document.getElementById('val-input-device') as HTMLElement;

    this.step0Host = new WasmHost();
    this.snesHost = new SnesHost();
    this.audioController = new AudioController();
    this.inputManager = new InputManager();

    this.setupUI();
  }

  public async start(): Promise<void> {
    try {
      console.log('[NativeWebEmulator] Loading Wasm Cores...');
      await Promise.all([
        this.snesHost.init('/wasm/snes_core.wasm'),
        this.step0Host.init('/wasm/step0_mock_core.wasm'),
      ]);

      const { width, height } = this.snesHost.getDimensions();
      console.log(`[NativeWebEmulator] SNES Core initialized (${width}x${height})`);

      // WebGL2 Renderer
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
    // Start Audio
    this.btnStartAudio.addEventListener('click', async () => {
      await this.audioController.init();
      await this.audioController.resume();
      this.audioOverlay.classList.add('hidden');
      this.audioStatusLabel.textContent = 'ATIVO';
    });

    // Toggle CRT
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

    // Core Selector
    this.selectCore.addEventListener('change', () => {
      this.activeCore = this.selectCore.value as 'snes' | 'step0';
      if (this.activeCore === 'step0') {
        this.valRomStatus.textContent = 'Modo Validação Step 0 Ativo';
      } else {
        this.valRomStatus.textContent = this.snesHost.isRomLoaded()
          ? 'ROM Carregada e Ativa'
          : 'Aguardando ROM...';
      }
    });

    // Load ROM via File Input
    this.btnLoadRom.addEventListener('click', () => {
      this.fileRomInput.click();
    });

    this.fileRomInput.addEventListener('change', async () => {
      const file = this.fileRomInput.files?.[0];
      if (file) {
        await this.handleRomFile(file);
      }
    });

    // Drag and Drop
    this.canvasContainer.addEventListener('dragover', (e) => {
      e.preventDefault();
      this.dropOverlay.classList.add('active');
    });

    this.canvasContainer.addEventListener('dragleave', () => {
      this.dropOverlay.classList.remove('active');
    });

    this.canvasContainer.addEventListener('drop', async (e) => {
      e.preventDefault();
      this.dropOverlay.classList.remove('active');
      const file = e.dataTransfer?.files?.[0];
      if (file) {
        await this.handleRomFile(file);
      }
    });

    // Fullscreen
    this.btnFullscreen.addEventListener('click', () => {
      if (!document.fullscreenElement) {
        this.canvasContainer.requestFullscreen().catch((err) => console.error(err));
      } else {
        document.exitFullscreen().catch((err) => console.error(err));
      }
    });
  }

  private async handleRomFile(file: File): Promise<void> {
    try {
      const arrayBuffer = await file.arrayBuffer();
      const bytes = new Uint8Array(arrayBuffer);

      const info = this.snesHost.loadRom(bytes);
      this.activeCore = 'snes';
      this.selectCore.value = 'snes';

      this.valRomStatus.textContent = `Ativo: ${file.name}`;
      this.valRomType.textContent = info.romType;
      this.valResetVector.textContent = info.resetVectorHex;
      this.valRomSize.textContent = `${(info.sizeBytes / 1024).toFixed(0)} KB`;

      console.log(`[NativeWebEmulator] ROM Carregada: ${file.name}`, info);
    } catch (err) {
      console.error('Erro ao carregar ROM:', err);
      alert(`Erro ao abrir ROM: ${err}`);
    }
  }

  private loop(now: number): void {
    const deltaMs = now - this.lastTime;
    this.lastTime = now;

    // 1. Process Input
    const inputMask = this.inputManager.getButtonMask(0);

    let wasmElapsed = 0;
    let pixelView: Uint8Array;
    let audioView: Float32Array;
    let heapSize = 0;

    if (this.activeCore === 'snes') {
      this.snesHost.setInput(0, inputMask);

      const wasmStart = performance.now();
      this.snesHost.stepFrame();
      wasmElapsed = performance.now() - wasmStart;

      pixelView = this.snesHost.getFramebufferView();
      audioView = this.snesHost.getAudioSamplesView();
      if (audioView.length > 0) {
        this.audioController.pushSamples(audioView);
        this.snesHost.consumeAudioSamples(audioView.length / 2);
      }
      heapSize = this.snesHost.getMemorySizeBytes();
    } else {
      this.step0Host.setInput(0, inputMask);

      const wasmStart = performance.now();
      this.step0Host.stepFrame();
      wasmElapsed = performance.now() - wasmStart;

      pixelView = this.step0Host.getFramebufferView();
      audioView = this.step0Host.getAudioSamplesView();
      if (audioView.length > 0) {
        this.audioController.pushSamples(audioView);
        this.step0Host.consumeAudioSamples(audioView.length / 2);
      }
      heapSize = this.step0Host.getMemorySizeBytes();
    }

    // 2. Render Framebuffer to WebGL2 Texture (Zero-Copy)
    this.renderer.renderFrame(pixelView, now / 1000.0);

    // 3. Telemetry
    this.frameCount++;
    this.currentFrameTime = deltaMs;
    this.currentWasmTime = wasmElapsed;

    if (now - this.fpsUpdateTimer > 500) {
      this.currentFps = (this.frameCount * 1000) / (now - this.fpsUpdateTimer);
      this.frameCount = 0;
      this.fpsUpdateTimer = now;
      this.updateTelemetryHUD(heapSize);
    }

    requestAnimationFrame((t) => this.loop(t));
  }

  private updateTelemetryHUD(heapSize: number): void {
    const fpsText = `${this.currentFps.toFixed(1)} FPS`;
    this.fpsBadge.textContent = fpsText;
    this.valFps.textContent = fpsText;
    this.valFrameTime.textContent = `${this.currentFrameTime.toFixed(2)} ms`;
    this.valWasmTime.textContent = `${this.currentWasmTime.toFixed(3)} ms`;
    this.valWasmMem.textContent = `${(heapSize / 1024).toFixed(1)} KB`;
    this.valInputDevice.textContent = this.inputManager.getGamepadStatus();
  }
}

// Bootstrap
window.addEventListener('DOMContentLoaded', () => {
  const app = new EmulatorApplication();
  app.start();
});
