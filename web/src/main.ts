import * as fflate from 'fflate';
import { WasmHost } from './core/WasmHost';
import { SnesHost } from './core/SnesHost';
import { WebGL2Renderer } from './renderer/WebGL2Renderer';
import { AudioController } from './audio/AudioController';
import { InputManager } from './input/InputManager';

class EmulatorApplication {
  private step0Host: WasmHost;
  private snesHost: SnesHost;
  private activeCore: 'snes' | 'step0' = 'step0'; // começa no Step0 até ROM ser carregada

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
  private valRomName: HTMLElement;
  private valRomType: HTMLElement;
  private valResetVector: HTMLElement;
  private valRomSize: HTMLElement;

  // CPU Debug Inspector elements
  private dbgPc: HTMLElement;
  private dbgA: HTMLElement;
  private dbgXy: HTMLElement;
  private dbgSpP: HTMLElement;
  private dbgOpDisp: HTMLElement;

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

  // Estado de inicialização
  private snesReady: boolean = false;
  private step0Ready: boolean = false;

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
    this.valRomName = document.getElementById('val-rom-name') as HTMLElement;
    this.valRomType = document.getElementById('val-rom-type') as HTMLElement;
    this.valResetVector = document.getElementById('val-reset-vector') as HTMLElement;
    this.valRomSize = document.getElementById('val-rom-size') as HTMLElement;

    this.dbgPc = document.getElementById('dbg-pc') as HTMLElement;
    this.dbgA = document.getElementById('dbg-a') as HTMLElement;
    this.dbgXy = document.getElementById('dbg-xy') as HTMLElement;
    this.dbgSpP = document.getElementById('dbg-sp-p') as HTMLElement;
    this.dbgOpDisp = document.getElementById('dbg-op-disp') as HTMLElement;

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
    console.log('[App] Iniciando carregamento dos cores Wasm...');

    // Inicializa o Step0 (sempre disponível, não requer ROM)
    try {
      await this.step0Host.init('/wasm/step0_mock_core.wasm');
      this.step0Ready = true;
      console.log('[App] Step0 Mock Core OK');
    } catch (err) {
      console.error('[App] Step0 falhou:', err);
    }

    // Inicializa o SNES core
    try {
      await this.snesHost.init('/wasm/snes_core.wasm');
      this.snesReady = true;
      console.log('[App] SNES Core OK');
    } catch (err) {
      console.error('[App] SNES Core falhou:', err);
      this.valRomStatus.textContent = 'Falha ao carregar SNES Core';
    }

    if (!this.step0Ready && !this.snesReady) {
      const msg = 'Falha crítica: nenhum core Wasm carregado.';
      console.error(`[App] ${msg}`);
      alert(msg);
      return;
    }

    // Determina dimensões do renderer
    // Preferência: SNES (256x224), fallback Step0
    let rendW = 256, rendH = 224;
    if (this.snesReady) {
      const dims = this.snesHost.getDimensions();
      rendW = dims.width;
      rendH = dims.height;
    }

    // Inicializa WebGL2 Renderer
    try {
      this.renderer = new WebGL2Renderer(this.canvas, rendW, rendH);
      console.log(`[App] WebGL2 Renderer OK (${rendW}x${rendH})`);
    } catch (err) {
      console.error('[App] WebGL2 falhou:', err);
      alert(`WebGL2 não disponível: ${err}`);
      return;
    }

    // O core padrão é Step0 até uma ROM ser carregada
    this.activeCore = this.step0Ready ? 'step0' : 'snes';
    this.selectCore.value = this.activeCore;
    this.valRomStatus.textContent = 'Aguardando ROM SNES...';

    // Inicia o loop principal
    this.lastTime = performance.now();
    this.fpsUpdateTimer = this.lastTime;
    requestAnimationFrame((t) => this.loop(t));

    console.log('[App] Loop de renderização iniciado. Core ativo:', this.activeCore);
  }

  private setupUI(): void {
    // Start Audio
    this.btnStartAudio.addEventListener('click', async () => {
      try {
        await this.audioController.init();
        await this.audioController.resume();
        this.audioOverlay.classList.add('hidden');
        this.audioStatusLabel.textContent = 'ATIVO';
      } catch (err) {
        console.warn('[App] Audio init falhou (pode ser OK em alguns navegadores):', err);
        this.audioOverlay.classList.add('hidden');
      }
    });

    // Toggle CRT
    this.btnToggleCrt.addEventListener('click', () => {
      if (!this.renderer) return;
      this.renderer.crtEnabled = !this.renderer.crtEnabled;
      this.crtStatusLabel.textContent = this.renderer.crtEnabled ? 'LIGADO' : 'DESLIGADO';
    });

    // Toggle Audio
    this.btnToggleAudio.addEventListener('click', async () => {
      try {
        await this.audioController.init();
        await this.audioController.resume();
        const isMuted = this.audioController.toggleMute();
        this.audioStatusLabel.textContent = isMuted ? 'MUTADO' : 'ATIVO';
      } catch (err) {
        console.warn('[App] Toggle audio falhou:', err);
      }
    });

    // Core Selector
    this.selectCore.addEventListener('change', () => {
      const newCore = this.selectCore.value as 'snes' | 'step0';
      if (newCore === 'snes' && !this.snesReady) {
        alert('SNES Core não está disponível.');
        this.selectCore.value = this.activeCore;
        return;
      }
      if (newCore === 'step0' && !this.step0Ready) {
        alert('Step0 Core não está disponível.');
        this.selectCore.value = this.activeCore;
        return;
      }
      this.activeCore = newCore;
      if (this.activeCore === 'step0') {
        this.valRomStatus.textContent = 'Modo Validação Step 0 Ativo';
      } else {
        this.valRomStatus.textContent = this.snesHost.isRomLoaded()
          ? 'ROM Carregada e Ativa'
          : 'Aguardando ROM...';
      }
      console.log('[App] Core alterado para:', this.activeCore);
    });

    // Load ROM via File Input
    this.btnLoadRom.addEventListener('click', () => {
      this.fileRomInput.click();
    });

    this.fileRomInput.addEventListener('change', async () => {
      const file = this.fileRomInput.files?.[0];
      if (file) {
        await this.handleRomFile(file);
        // Limpa o input para permitir recarregar o mesmo arquivo
        this.fileRomInput.value = '';
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
    if (!this.snesReady) {
      alert('SNES Core não está pronto. Aguarde o carregamento ou recarregue a página.');
      return;
    }

    try {
      this.valRomStatus.textContent = `Carregando: ${file.name}...`;
      console.log(`[App] Carregando arquivo: ${file.name} (${file.size} bytes)`);

      const arrayBuffer = await file.arrayBuffer();
      let romBytes: Uint8Array = new Uint8Array(arrayBuffer);
      let internalName = file.name;

      // Suporte a arquivos .ZIP via fflate
      if (file.name.toLowerCase().endsWith('.zip')) {
        console.log('[App] Descomprimindo ZIP...');
        const unzipped = fflate.unzipSync(new Uint8Array(arrayBuffer));

        // Prioridade: .sfc > .smc > .bin > arquivo maior
        let bestName: string | null = null;
        let bestData: Uint8Array | null = null;
        let bestScore = -1;

        for (const [filename, data] of Object.entries(unzipped)) {
          const lower = filename.toLowerCase();
          let score = 0;
          if (lower.endsWith('.sfc')) score = 3;
          else if (lower.endsWith('.smc')) score = 2;
          else if (lower.endsWith('.bin')) score = 1;

          // Ignora arquivos muito pequenos (<= 512 bytes)
          if (data.length <= 512) continue;

          if (score > bestScore || (score === bestScore && data.length > (bestData?.length ?? 0))) {
            bestScore = score;
            bestName = filename;
            bestData = data;
          }
        }

        if (!bestData || !bestName) {
          throw new Error('Nenhuma ROM válida (.sfc / .smc / .bin) encontrada no ZIP.');
        }

        romBytes = bestData;
        internalName = `${file.name} → ${bestName}`;
        console.log(`[App] ROM extraída: ${bestName} (${romBytes.length} bytes)`);
      }

      console.log(`[App] Enviando ${romBytes.length} bytes para snesHost.loadRom...`);
      const info = this.snesHost.loadRom(romBytes);

      // Muda o core ativo para SNES
      this.activeCore = 'snes';
      this.selectCore.value = 'snes';

      this.valRomStatus.textContent = 'Executando';
      this.valRomName.textContent = internalName;
      this.valRomType.textContent = info.romType;
      this.valResetVector.textContent = info.resetVectorHex;
      this.valRomSize.textContent = `${(info.sizeBytes / 1024).toFixed(0)} KB`;

      console.log('[App] ROM carregada com sucesso!', info);
    } catch (err) {
      console.error('[App] Erro ao carregar ROM:', err);
      this.valRomStatus.textContent = 'Erro ao carregar';
      alert(`Erro ao abrir ROM:\n${err}`);
    }
  }

  private loop(now: number): void {
    const deltaMs = now - this.lastTime;
    this.lastTime = now;

    // Processa Input
    const inputMask = this.inputManager.getButtonMask(0);

    let wasmElapsed = 0;
    let pixelView: Uint8Array;
    let audioView: Float32Array;
    let heapSize = 0;

    if (this.activeCore === 'snes' && this.snesReady) {
      this.snesHost.setInput(0, inputMask);

      const wasmStart = performance.now();
      this.snesHost.stepFrame();
      wasmElapsed = performance.now() - wasmStart;

      pixelView = this.snesHost.getFramebufferView();
      audioView = this.snesHost.getAudioSamplesView();
      heapSize = this.snesHost.getMemorySizeBytes();
    } else if (this.step0Ready) {
      this.step0Host.setInput(0, inputMask);

      const wasmStart = performance.now();
      this.step0Host.stepFrame();
      wasmElapsed = performance.now() - wasmStart;

      pixelView = this.step0Host.getFramebufferView() as Uint8Array;
      audioView = this.step0Host.getAudioSamplesView() as Float32Array;
      heapSize = this.step0Host.getMemorySizeBytes();
    } else {
      // Sem core disponível — buffer preto
      pixelView = new Uint8Array(256 * 224 * 4);
      audioView = new Float32Array(0);
    }

    // Envia amostras de áudio
    if (audioView.length > 0) {
      try {
        this.audioController.pushSamples(audioView);
        if (this.activeCore === 'snes') {
          this.snesHost.consumeAudioSamples(audioView.length / 2);
        } else {
          this.step0Host.consumeAudioSamples(audioView.length / 2);
        }
      } catch (_) {
        // Áudio pode falhar sem contexto inicializado — ignorar silenciosamente
      }
    }

    // Renderiza o framebuffer no WebGL2
    if (this.renderer) {
      this.renderer.renderFrame(pixelView, now / 1000.0);
    }

    // Telemetria
    this.frameCount++;
    this.currentFrameTime = deltaMs;
    this.currentWasmTime = wasmElapsed;

    if (now - this.fpsUpdateTimer > 250) {
      const elapsed = now - this.fpsUpdateTimer;
      this.currentFps = (this.frameCount * 1000) / elapsed;
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

    // Atualiza o CPU Debug Inspector se o core SNES estiver ativo
    if (this.activeCore === 'snes' && this.snesHost.isRomLoaded()) {
      const dbg = this.snesHost.getCpuDebugState();
      this.dbgPc.textContent = dbg.pc;
      this.dbgA.textContent = dbg.a;
      this.dbgXy.textContent = `${dbg.x} / ${dbg.y}`;
      this.dbgSpP.textContent = `${dbg.sp} / ${dbg.p}`;
      const dispText = (dbg.inidisp & 0x80) !== 0 ? 'Force Blank' : `Brilho ${dbg.inidisp & 0x0F}/15`;
      this.dbgOpDisp.textContent = `${dbg.lastOpcode} | ${dispText}`;
    }
  }
}

// Bootstrap
window.addEventListener('DOMContentLoaded', () => {
  const app = new EmulatorApplication();
  app.start();
});
