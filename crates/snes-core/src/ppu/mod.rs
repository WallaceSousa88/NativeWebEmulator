pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 224;
pub const TOTAL_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

pub struct Ppu {
    pub framebuffer: [u32; TOTAL_PIXELS], // RGBA8888
    pub vram: [u8; 0x10000],              // 64 KB VRAM
    pub cgram: [u16; 256],                // 256 cores BGR555 (15-bit)
    pub oam: [u8; 544],                   // 512 bytes tabela principal + 32 bytes tabela alta

    // Registradores
    pub inidisp: u8,                      // $2100: Brilho (0-15) e Force Blank (bit 7)
    pub obsel: u8,                        // $2101: Configuração de Sprites (tamanho e base VRAM)
    pub oam_addr: u16,                    // $2102-$2103: Endereço do OAM
    pub oam_latch: u8,                    // Latch para escrita no OAM
    pub bg_mode: u8,                      // $2105: Modo BG (0-7)
    pub bg_sc: [u8; 4],                   // $2107-$210A: Base e tamanho de tela para BG1-4
    pub bg_nba: [u8; 2],                  // $210B-$210C: Base de tiles/caracteres para BG1-4
    pub bg_hofs: [u16; 4],                // Scroll horizontal BG1-4
    pub bg_vofs: [u16; 4],                // Scroll vertical BG1-4
    pub bg_prev_write: u8,                // Toggle para scroll 16-bit
    pub vmain: u8,                        // $2115: Incremento e controle do VRAM
    pub vmadd: u16,                       // $2116-$2117: Ponteiro de endereço VRAM (word)
    pub vram_read_buffer: u16,            // Prefetch buffer para leitura do VRAM
    pub cgadd: u8,                        // $2121: Endereço CGRAM (0-255)
    pub cgdata_latch: Option<u8>,         // Latch do primeiro byte de CGDATA
    pub tm: u8,                           // $212C: Main screen layer enable (BG1-4, OBJ)
    pub ts: u8,                           // $212D: Sub screen layer enable
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            framebuffer: [0xFF000000; TOTAL_PIXELS],
            vram: [0; 0x10000],
            cgram: [0; 256],
            oam: [0; 544],

            inidisp: 0x0F, // Brilho total por padrão
            obsel: 0,
            oam_addr: 0,
            oam_latch: 0,
            bg_mode: 0,
            bg_sc: [0; 4],
            bg_nba: [0; 2],
            bg_hofs: [0; 4],
            bg_vofs: [0; 4],
            bg_prev_write: 0,
            vmain: 0,
            vmadd: 0,
            vram_read_buffer: 0,
            cgadd: 0,
            cgdata_latch: None,
            tm: 0x1F, // Todas as camadas ligadas na Main Screen
            ts: 0,
        }
    }

    pub fn reset(&mut self) {
        self.framebuffer.fill(0xFF000000);
        self.inidisp = 0x0F;
        self.oam_addr = 0;
        self.cgadd = 0;
        self.cgdata_latch = None;
    }

    // =========================================================================
    // Leitura e Escrita de Registradores PPU ($2100 - $213F)
    // =========================================================================
    pub fn read_reg(&mut self, addr: u16) -> u8 {
        match addr {
            0x2134..=0x2136 => {
                // Multiplicação por hardware (MPYL, MPYM, MPYH)
                0
            }
            0x2137 => {
                // Latch de software H/V counter
                0
            }
            0x2138 => {
                // Leitura OAM
                let val = self.oam[self.oam_addr as usize % 544];
                self.oam_addr = (self.oam_addr + 1) & 0x03FF;
                val
            }
            0x2139 => {
                // VMDATALREAD (com prefetch buffer)
                let val = (self.vram_read_buffer & 0xFF) as u8;
                self.step_vram_read();
                val
            }
            0x213A => {
                // VMDATAHREAD (com prefetch buffer)
                let val = ((self.vram_read_buffer >> 8) & 0xFF) as u8;
                self.step_vram_read();
                val
            }
            0x213B => {
                // CGDATAREAD
                0
            }
            0x213E => {
                // STAT77: PPU1 Status
                0x01
            }
            0x213F => {
                // STAT78: PPU2 Status (NTSC = 0, PAL = 0x10)
                0x00
            }
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, addr: u16, val: u8) {
        match addr {
            0x2100 => {
                self.inidisp = val;
            }
            0x2101 => {
                self.obsel = val;
            }
            0x2102 => {
                self.oam_addr = (self.oam_addr & 0x0200) | ((val as u16) << 1);
            }
            0x2103 => {
                self.oam_addr = ((val as u16 & 0x01) << 9) | (self.oam_addr & 0x01FE);
            }
            0x2104 => {
                // Escrita OAM (16-bit com latch)
                if (self.oam_addr & 1) == 0 {
                    self.oam_latch = val;
                } else {
                    let word_addr = (self.oam_addr >> 1) as usize;
                    if word_addr < 272 {
                        let byte_offset = word_addr * 2;
                        if byte_offset < 544 {
                            self.oam[byte_offset] = self.oam_latch;
                            if byte_offset + 1 < 544 {
                                self.oam[byte_offset + 1] = val;
                            }
                        }
                    }
                }
                self.oam_addr = (self.oam_addr + 1) & 0x03FF;
            }
            0x2105 => {
                self.bg_mode = val;
            }
            0x2107..=0x210A => {
                let bg = (addr - 0x2107) as usize;
                self.bg_sc[bg] = val;
            }
            0x210B => {
                self.bg_nba[0] = val; // BG1 e BG2
            }
            0x210C => {
                self.bg_nba[1] = val; // BG3 e BG4
            }
            0x210D..=0x2114 => {
                let bg = ((addr - 0x210D) >> 1) as usize;
                let is_v = ((addr - 0x210D) & 1) == 1;
                let val16 = ((val as u16) << 8) | ((self.bg_prev_write as u16) & 0xF8) | ((val as u16) & 0x07);
                if is_v {
                    self.bg_vofs[bg] = val16;
                } else {
                    self.bg_hofs[bg] = val16;
                }
                self.bg_prev_write = val;
            }
            0x2115 => {
                self.vmain = val;
            }
            0x2116 => {
                self.vmadd = (self.vmadd & 0xFF00) | (val as u16);
                self.update_vram_prefetch();
            }
            0x2117 => {
                self.vmadd = (self.vmadd & 0x00FF) | ((val as u16) << 8);
                self.update_vram_prefetch();
            }
            0x2118 => {
                // VMDATAL: Escreve byte baixo no VRAM
                let byte_addr = ((self.vmadd & 0x7FFF) as usize) * 2;
                self.vram[byte_addr] = val;
                if (self.vmain & 0x80) == 0 {
                    self.increment_vmadd();
                }
            }
            0x2119 => {
                // VMDATAH: Escreve byte alto no VRAM
                let byte_addr = ((self.vmadd & 0x7FFF) as usize) * 2 + 1;
                self.vram[byte_addr] = val;
                if (self.vmain & 0x80) != 0 {
                    self.increment_vmadd();
                }
            }
            0x2121 => {
                self.cgadd = val;
                self.cgdata_latch = None;
            }
            0x2122 => {
                // CGDATA: Escrita 15-bit BGR555 em 2 bytes
                match self.cgdata_latch {
                    None => {
                        self.cgdata_latch = Some(val);
                    }
                    Some(low_byte) => {
                        let color15 = ((val as u16 & 0x7F) << 8) | (low_byte as u16);
                        self.cgram[self.cgadd as usize] = color15;
                        self.cgadd = self.cgadd.wrapping_add(1);
                        self.cgdata_latch = None;
                    }
                }
            }
            0x212C => {
                self.tm = val;
            }
            0x212D => {
                self.ts = val;
            }
            _ => {}
        }
    }

    fn increment_vmadd(&mut self) {
        let step = match self.vmain & 0x03 {
            0 => 1,
            1 => 32,
            _ => 128,
        };
        self.vmadd = self.vmadd.wrapping_add(step);
    }

    fn update_vram_prefetch(&mut self) {
        let byte_addr = ((self.vmadd & 0x7FFF) as usize) * 2;
        let low = self.vram[byte_addr] as u16;
        let high = self.vram[byte_addr + 1] as u16;
        self.vram_read_buffer = (high << 8) | low;
    }

    fn step_vram_read(&mut self) {
        self.update_vram_prefetch();
        if (self.vmain & 0x80) != 0 {
            self.increment_vmadd();
        }
    }

    // =========================================================================
    // Conversão de Cor BGR555 para RGBA8888 com Escala de Brilho
    // =========================================================================
    #[inline(always)]
    pub fn bgr555_to_rgba(&self, bgr: u16) -> u32 {
        let brightness = (self.inidisp & 0x0F) as u32;
        if brightness == 0 || (self.inidisp & 0x80) != 0 {
            return 0xFF000000; // Black / Blank
        }

        let r5 = (bgr & 0x001F) as u32;
        let g5 = ((bgr >> 5) & 0x001F) as u32;
        let b5 = ((bgr >> 10) & 0x001F) as u32;

        // Converte 5-bit (0..31) para 8-bit (0..255) aplicando multiplicador de brilho (0..15)
        let r = ((r5 * 255 / 31) * brightness) / 15;
        let g = ((g5 * 255 / 31) * brightness) / 15;
        let b = ((b5 * 255 / 31) * brightness) / 15;

        0xFF000000 | (b << 16) | (g << 8) | r
    }

    // =========================================================================
    // Rasterizador de Scanline (Renderiza 1 Linha Horizontal a cada H-Blank)
    // =========================================================================
    pub fn render_scanline(&mut self, y: usize) {
        if y >= SCREEN_HEIGHT {
            return;
        }

        let fb_offset = y * SCREEN_WIDTH;

        // Se Force Blank estiver ativo, preenche a linha com preto
        if (self.inidisp & 0x80) != 0 {
            for x in 0..SCREEN_WIDTH {
                self.framebuffer[fb_offset + x] = 0xFF000000;
            }
            return;
        }

        // 1. Linha base com a cor de fundo (Backdrop Color = CGRAM[0])
        let backdrop_color = self.bgr555_to_rgba(self.cgram[0]);
        for x in 0..SCREEN_WIDTH {
            self.framebuffer[fb_offset + x] = backdrop_color;
        }

        // 2. Renderizar Camada BG1 se habilitada na Main Screen (bit 0 de TM)
        if (self.tm & 0x01) != 0 {
            self.render_bg_scanline(0, y, fb_offset);
        }

        // 3. Renderizar Camada BG2 se habilitada (bit 1 de TM)
        if (self.tm & 0x02) != 0 {
            self.render_bg_scanline(1, y, fb_offset);
        }

        // 4. Renderizar Sprites (OBJ) se habilitado (bit 4 de TM)
        if (self.tm & 0x10) != 0 {
            self.render_sprites_scanline(y, fb_offset);
        }
    }

    fn render_bg_scanline(&mut self, bg_idx: usize, y: usize, fb_offset: usize) {
        // Tilemap Base Address (words)
        let sc = self.bg_sc[bg_idx];
        let tilemap_base = ((sc & 0xFC) as usize) << 9; // base em words

        // Character (Tiles) Base Address
        let nba = self.bg_nba[bg_idx / 2];
        let char_base = if (bg_idx % 2) == 0 {
            ((nba & 0x0F) as usize) << 13
        } else {
            (((nba >> 4) & 0x0F) as usize) << 13
        };

        let h_scroll = self.bg_hofs[bg_idx] as usize;
        let v_scroll = self.bg_vofs[bg_idx] as usize;
        let screen_y = (y + v_scroll) % 256;
        let tile_y = screen_y / 8;
        let in_tile_y = screen_y % 8;

        for x in 0..SCREEN_WIDTH {
            let screen_x = (x + h_scroll) % 256;
            let tile_x = screen_x / 8;
            let in_tile_x = screen_x % 8;

            // Entrada no Tilemap (2 bytes por tile)
            let tile_entry_addr = (tilemap_base + (tile_y * 32 + tile_x)) * 2;
            if tile_entry_addr + 1 >= self.vram.len() {
                continue;
            }

            let low_byte = self.vram[tile_entry_addr] as u16;
            let high_byte = self.vram[tile_entry_addr + 1] as u16;
            let tile_data = (high_byte << 8) | low_byte;

            let tile_num = (tile_data & 0x03FF) as usize;
            let palette_idx = ((tile_data >> 10) & 0x07) as usize;
            let v_flip = (tile_data & 0x8000) != 0;
            let h_flip = (tile_data & 0x4000) != 0;

            let actual_tile_y = if v_flip { 7 - in_tile_y } else { in_tile_y };
            let actual_tile_x = if h_flip { 7 - in_tile_x } else { in_tile_x };

            // Decodificação de 4BPP (Modo 1: 16 cores por paleta = 32 bytes por tile)
            // Planos 0 e 1 nos bytes 0..15, Planos 2 e 3 nos bytes 16..31
            let tile_byte_offset = char_base * 2 + tile_num * 32;
            let line_offset = actual_tile_y * 2;

            if tile_byte_offset + line_offset + 17 < self.vram.len() {
                let p0 = self.vram[tile_byte_offset + line_offset];
                let p1 = self.vram[tile_byte_offset + line_offset + 1];
                let p2 = self.vram[tile_byte_offset + line_offset + 16];
                let p3 = self.vram[tile_byte_offset + line_offset + 17];

                let bit_shift = 7 - actual_tile_x;
                let b0 = (p0 >> bit_shift) & 1;
                let b1 = (p1 >> bit_shift) & 1;
                let b2 = (p2 >> bit_shift) & 1;
                let b3 = (p3 >> bit_shift) & 1;

                let color_idx = (b3 << 3) | (b2 << 2) | (b1 << 1) | b0;
                if color_idx > 0 {
                    // Cor transparente não sobrescreve
                    let final_cgram_idx = (palette_idx * 16 + color_idx as usize) % 256;
                    let bgr = self.cgram[final_cgram_idx];
                    self.framebuffer[fb_offset + x] = self.bgr555_to_rgba(bgr);
                }
            }
        }
    }

    fn render_sprites_scanline(&mut self, y: usize, fb_offset: usize) {
        // Tabela OAM contém 128 sprites (4 bytes cada na tabela baixa)
        let sprite_base = ((self.obsel & 0x07) as usize) << 14; // Base em words no VRAM

        for i in 0..128 {
            let oam_idx = i * 4;
            let spr_x = self.oam[oam_idx] as i32;
            let spr_y = self.oam[oam_idx + 1] as usize;
            let tile_num = self.oam[oam_idx + 2] as usize;
            let attr = self.oam[oam_idx + 3];

            let palette = ((attr >> 1) & 0x07) as usize + 8; // Paletas de sprites são 8 a 15
            let h_flip = (attr & 0x40) != 0;
            let v_flip = (attr & 0x80) != 0;

            // Sprites padrão 8x8
            if y >= spr_y && y < spr_y + 8 {
                let in_spr_y = if v_flip { 7 - (y - spr_y) } else { y - spr_y };
                let tile_offset = sprite_base * 2 + tile_num * 32;
                let line_offset = in_spr_y * 2;

                if tile_offset + line_offset + 17 < self.vram.len() {
                    let p0 = self.vram[tile_offset + line_offset];
                    let p1 = self.vram[tile_offset + line_offset + 1];
                    let p2 = self.vram[tile_offset + line_offset + 16];
                    let p3 = self.vram[tile_offset + line_offset + 17];

                    for dx in 0..8 {
                        let actual_x = spr_x + dx;
                        if actual_x >= 0 && actual_x < SCREEN_WIDTH as i32 {
                            let in_spr_x = if h_flip { 7 - dx } else { dx };
                            let bit_shift = 7 - in_spr_x;
                            let b0 = (p0 >> bit_shift) & 1;
                            let b1 = (p1 >> bit_shift) & 1;
                            let b2 = (p2 >> bit_shift) & 1;
                            let b3 = (p3 >> bit_shift) & 1;
                            let color_idx = (b3 << 3) | (b2 << 2) | (b1 << 1) | b0;

                            if color_idx > 0 {
                                let cgram_idx = (palette * 16 + color_idx as usize) % 256;
                                let bgr = self.cgram[cgram_idx];
                                self.framebuffer[fb_offset + actual_x as usize] = self.bgr555_to_rgba(bgr);
                            }
                        }
                    }
                }
            }
        }
    }
}
