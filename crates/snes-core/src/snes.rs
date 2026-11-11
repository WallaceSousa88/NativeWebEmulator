use crate::bus::Bus;
use crate::cartridge::{Cartridge, CartridgeHeader};
use crate::cpu::Cpu;
use crate::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH, TOTAL_PIXELS};
use core_traits::{EmulatorCore, PixelFormat, VideoFrameInfo};

pub struct Snes {
    pub cpu: Cpu,
    pub bus: Bus,
    pub frame_count: u32,
    audio_buffer: [f32; 735 * 4],
    audio_samples_count: usize,
}

impl Snes {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            bus: Bus::new(),
            frame_count: 0,
            audio_buffer: [0.0; 735 * 4],
            audio_samples_count: 0,
        }
    }

    pub fn rom_header(&self) -> Option<&CartridgeHeader> {
        self.bus.cartridge.as_ref().map(|c| &c.header)
    }

    /// Executa 1 frame completo do SNES (262 Scanlines NTSC a 60.0988 Hz)
    pub fn run_frame(&mut self) {
        // SNES NTSC: 262 scanlines por frame
        // Scanlines 0..224: Display ativo
        // Scanlines 225..261: V-Blank
        const SCANLINES_PER_FRAME: usize = 262;
        const CYCLES_PER_SCANLINE: u32 = 1364 / 4; // Master clock (~21.477 MHz) / ~3.58 MHz CPU

        for line in 0..SCANLINES_PER_FRAME {
            // Início do V-Blank na scanline 224
            if line == 224 {
                self.bus.vblank_flag = true;
                self.bus.nmi_flag = true;

                // Se NMI estiver habilitado no registrador $4200
                if (self.bus.nmitimen & 0x80) != 0 {
                    self.cpu.trigger_nmi(&mut self.bus);
                }
            } else if line == 261 {
                // Fim do V-Blank
                self.bus.vblank_flag = false;
            }

            // Executa os ciclos da CPU nesta scanline
            let mut line_cycles = 0;
            while line_cycles < CYCLES_PER_SCANLINE {
                let cycles = self.cpu.step(&mut self.bus);
                line_cycles += cycles;
                self.cpu.cycles += cycles as u64;
            }

            // Renderiza a linha de pixels se estiver na área visível
            if line < SCREEN_HEIGHT {
                self.bus.ppu.render_scanline(line);
            }
        }

        self.frame_count = self.frame_count.wrapping_add(1);

        // Gera 735 amostras de áudio para o frame (silêncio até APU completa ser conectada)
        if self.audio_samples_count + 735 <= self.audio_buffer.len() / 2 {
            for i in 0..(735 * 2) {
                self.audio_buffer[self.audio_samples_count * 2 + i] = 0.0;
            }
            self.audio_samples_count += 735;
        }
    }
}

impl EmulatorCore for Snes {
    type Error = &'static str;

    fn load_rom(&mut self, rom_data: &[u8]) -> Result<(), Self::Error> {
        let cart = Cartridge::load(rom_data)?;
        self.bus.cartridge = Some(cart);
        self.reset();
        Ok(())
    }

    fn reset(&mut self) {
        self.bus.reset();
        self.cpu.reset(&mut self.bus);
        self.frame_count = 0;
        self.audio_samples_count = 0;
    }

    fn step_frame(&mut self) {
        self.run_frame();
    }

    fn video_info(&self) -> VideoFrameInfo {
        VideoFrameInfo {
            width: SCREEN_WIDTH as u32,
            height: SCREEN_HEIGHT as u32,
            format: PixelFormat::Rgba8888,
            aspect_ratio: 4.0 / 3.0,
            target_fps: 60.0988,
        }
    }

    fn video_buffer_ptr(&self) -> *const u8 {
        self.bus.ppu.framebuffer.as_ptr() as *const u8
    }

    fn video_buffer_len(&self) -> usize {
        TOTAL_PIXELS * 4
    }

    fn audio_sample_rate(&self) -> u32 {
        44100
    }

    fn audio_buffer_ptr(&self) -> *const f32 {
        self.audio_buffer.as_ptr()
    }

    fn audio_samples_available(&self) -> usize {
        self.audio_samples_count
    }

    fn consume_audio_samples(&mut self, count: usize) {
        if count >= self.audio_samples_count {
            self.audio_samples_count = 0;
        } else {
            let remaining = self.audio_samples_count - count;
            for i in 0..(remaining * 2) {
                self.audio_buffer[i] = self.audio_buffer[i + (count * 2)];
            }
            self.audio_samples_count = remaining;
        }
    }

    fn set_input_state(&mut self, port: u8, button_mask: u16) {
        if port == 0 {
            self.bus.joy1_state = button_mask;
        }
    }
}
