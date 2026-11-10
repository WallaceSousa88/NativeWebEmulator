# NativeWebEmulator

> Ecossistema de emulação de consoles retro executado 100% no navegador (Client-Side, zero dependências de backend), combinando **WebAssembly (Wasm)** em **Rust** para hardware de retro-consoles e **WebGL2 / WebGPU** para shaders e aceleração gráfica.

---

## 🎯 Status: Step 0 Concluído (Pipeline Wasm + WebGL2 + AudioWorklet)

O protótipo de validação de pipeline (**Step 0**) valida com sucesso:
- **Zero-Copy Memory Streaming:** Upload do framebuffer direto da memória linear do Wasm (`Uint8Array` sobre `wasm.memory.buffer`) para textura GPU via `gl.texSubImage2D`, sem alocações no loop do browser (0 GC churn).
- **Shader Pipeline (CRT/Scanlines):** Shaders em GLSL ES 3.00 com scanlines, curvatura de tubo CRT, máscara de fósforo RGB e aberração cromática com opção de alternar para pixels crisp nativos.
- **Áudio de Baixa Latência:** `AudioWorkletProcessor` consumindo amostras float32 geradas pelo core em tempo real a 44.1 kHz estéreo sem estalos (*crackles*).
- **Sub-milissegundo Wasm Execution:** Ciclo de emulação processado em ~0.1 ms por frame a 60 FPS estáveis.
- **Input Responsivo:** Mapeamento unificado via teclado e Gamepad API transmitido como máscara de bits de 16 bits para o core.

---

## 🏗️ Arquitetura do Repositório

```text
native-web-emulator/
├── crates/
│   ├── core-traits/         # Interface comum abstrata para cores de emulação (Rust)
│   └── step0-mock-core/     # Core de validação de pipeline (compilado para Wasm32 puro)
├── web/                     # Frontend estático em TypeScript + Vite
│   ├── public/
│   │   ├── audio/           # AudioWorkletProcessor em JS dedicado
│   │   └── wasm/            # Binário step0_mock_core.wasm gerado pelo Rust
│   └── src/
│       ├── core/            # Host Wasm e bindings de memória linear
│       ├── renderer/        # Renderizador WebGL2 e pipeline de shaders CRT
│       ├── audio/           # Controlador de Web Audio API
│       ├── input/           # Gerenciador de teclado e Gamepad API
│       └── main.ts          # Loop de sincronização e telemetria
├── scripts/                 # Scripts utilitários de automação cross-platform
└── Cargo.toml               # Workspace Rust
```

---

## 🚀 Como Executar Localmente

### Pré-requisitos
- **Node.js** (v18+) e **NPM**
- **Rust** com target Wasm: `rustup target add wasm32-unknown-unknown`

### Executando o Dev Server
```bash
# 1. Compilar o core Rust em WebAssembly
npm run build:wasm

# 2. Iniciar o servidor de desenvolvimento
npm run dev
```

Abra o endereço indicado pelo Vite no seu navegador (normalmente `http://localhost:5173/`).

### Testando os Controles
- **Setas** ou **W / A / S / D**: Movem o sprite de teste na tela (D-Pad).
- **X** / **L**: Botão A (Sprite Verde / Tom 880 Hz).
- **Z** / **K**: Botão B (Sprite Vermelho / Turbo / Tom 587 Hz).
- **C** / **I**: Botão Y (Sprite Amarelo / Tom 523 Hz).
- **V** / **O**: Botão X (Sprite Roxo / Tom 659 Hz).
- **Gamepad USB / Bluetooth**: Conecte qualquer controle (ex: Xbox, DualShock, 8BitDo) e os botões serão mapeados automaticamente.
- **Botões na Tela**: Alternar shader CRT (CRT vs Pixels Puros), mutar áudio e modo tela cheia.

---

## 🗺️ Próximas Fases:
- [x] **Step 0:** Validação de canal Wasm <-> WebGL2 <-> AudioWorklet.
- [ ] **Fase 1 (SNES Core):**
  - Implementação da CPU Ricoh 5A22 (WDC 65c816).
  - PPU do SNES (Modos de BG 0 a 7, Sprites / OAM, Janelas e Color Math).
  - APU do SNES (Sony SPC700 + DSP).
  - Mapeamento de Memória (LoROM / HiROM / SRAM).
- [ ] **Fase 2:** Mega Drive (Motorola 68000, Z80, VDP, YM2612).
- [ ] **Fase 3:** PlayStation 1 (MIPS R3000A, GTE, SPU, GPU).
