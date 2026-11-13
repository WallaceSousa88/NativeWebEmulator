use crate::cartridge::Cartridge;
use crate::dma::DmaChannel;
use crate::ppu::Ppu;

pub struct Bus {
    pub wram: [u8; 0x20000], // 128 KB WRAM principal
    pub cartridge: Option<Cartridge>,
    pub ppu: Ppu,
    pub dma: [DmaChannel; 8],

    // Registradores de CPU e Interrupções
    pub nmitimen: u8,
    pub nmi_flag: bool,
    pub vblank_flag: bool,
    pub hblank_flag: bool,
    pub joy1_state: u16,

    // APU I/O Ports ($2140-$2143)
    // Inicializado com $AA e $BB (SPC700 Boot Handshake obrigatório para jogos comerciais)
    pub apu_ports: [u8; 4],

    // Hardware Math (Ricoh 5A22 Multiplier / Divider)
    pub wrmpya: u8,
    pub wrmpyb: u8,
    pub wrdiv_dividend: u16,
    pub wrdiv_divisor: u8,
    pub rddiv_quotient: u16,
    pub rdmpy_product_or_remainder: u16,

    pub open_bus: u8,
}

impl Bus {
    pub fn new() -> Self {
        Self {
            wram: [0; 0x20000],
            cartridge: None,
            ppu: Ppu::new(),
            dma: [DmaChannel::new(); 8],

            nmitimen: 0,
            nmi_flag: false,
            vblank_flag: false,
            hblank_flag: false,
            joy1_state: 0,
            apu_ports: [0xAA, 0xBB, 0x00, 0x00],

            wrmpya: 0xFF,
            wrmpyb: 0,
            wrdiv_dividend: 0xFFFF,
            wrdiv_divisor: 0,
            rddiv_quotient: 0,
            rdmpy_product_or_remainder: 0,

            open_bus: 0,
        }
    }

    pub fn reset(&mut self) {
        self.wram.fill(0);
        self.ppu.reset();
        self.nmitimen = 0;
        self.nmi_flag = false;
        self.vblank_flag = false;
        self.hblank_flag = false;
        self.apu_ports = [0xAA, 0xBB, 0x00, 0x00];
        self.open_bus = 0;
    }

    // =========================================================================
    // Leitura no Espaço de Endereçamento de 24 Bits ($000000 - $FFFFFF)
    // =========================================================================
    pub fn read_u8(&mut self, bank: u8, addr: u16) -> u8 {
        let val = match (bank, addr) {
            // WRAM total mapeada em $7E e $7F (128 KB)
            (0x7E, _) => self.wram[addr as usize],
            (0x7F, _) => self.wram[0x10000 + addr as usize],

            // Bancos $00-$3F e $80-$BF
            (0x00..=0x3F | 0x80..=0xBF, 0x0000..=0x1FFF) => {
                // Espelho dos primeiros 8 KB da WRAM
                self.wram[addr as usize]
            }

            // Registradores PPU ($2100 - $213F)
            (0x00..=0x3F | 0x80..=0xBF, 0x2100..=0x213F) => {
                self.ppu.read_reg(addr)
            }

            // Portas APU I/O ($2140 - $2143)
            (0x00..=0x3F | 0x80..=0xBF, 0x2140..=0x2143) => {
                let idx = (addr - 0x2140) as usize;
                self.apu_ports[idx]
            }

            // Registradores de CPU e Controle ($4200 - $421F)
            (0x00..=0x3F | 0x80..=0xBF, 0x4210) => {
                // RDNMI: Lê flag NMI (bit 7) e limpa imediatamente
                let res = if self.nmi_flag { 0x82 } else { 0x02 };
                self.nmi_flag = false;
                res
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4211) => {
                // TIMEUP: IRQ status
                0x00
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4212) => {
                // HVBJOY: Status V-Blank (bit 7) e H-Blank (bit 6)
                let mut status = 0;
                if self.vblank_flag { status |= 0x80; }
                if self.hblank_flag { status |= 0x40; }
                status
            }

            // Hardware Math Readouts ($4214 - $4217)
            (0x00..=0x3F | 0x80..=0xBF, 0x4214) => {
                (self.rddiv_quotient & 0xFF) as u8
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4215) => {
                ((self.rddiv_quotient >> 8) & 0xFF) as u8
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4216) => {
                (self.rdmpy_product_or_remainder & 0xFF) as u8
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4217) => {
                ((self.rdmpy_product_or_remainder >> 8) & 0xFF) as u8
            }

            // Auto-Joypad
            (0x00..=0x3F | 0x80..=0xBF, 0x4218) => {
                (self.joy1_state & 0xFF) as u8
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4219) => {
                ((self.joy1_state >> 8) & 0xFF) as u8
            }

            // DMA Registers ($4300 - $437F)
            (0x00..=0x3F | 0x80..=0xBF, 0x4300..=0x437F) => {
                let ch = ((addr >> 4) & 0x07) as usize;
                let reg = (addr & 0x0F) as u8;
                let channel = &self.dma[ch];
                match reg {
                    0x0 => channel.control,
                    0x1 => channel.dest_reg,
                    0x2 => (channel.src_addr & 0xFF) as u8,
                    0x3 => ((channel.src_addr >> 8) & 0xFF) as u8,
                    0x4 => channel.src_bank,
                    0x5 => (channel.byte_count & 0xFF) as u8,
                    0x6 => ((channel.byte_count >> 8) & 0xFF) as u8,
                    _ => self.open_bus,
                }
            }

            // Cartucho (ROM / SRAM)
            _ => {
                if let Some(ref cart) = self.cartridge {
                    cart.read(bank, addr)
                } else {
                    self.open_bus
                }
            }
        };

        self.open_bus = val;
        val
    }

    // =========================================================================
    // Escrita no Espaço de Endereçamento de 24 Bits
    // =========================================================================
    pub fn write_u8(&mut self, bank: u8, addr: u16, val: u8) {
        self.open_bus = val;

        match (bank, addr) {
            // WRAM total em $7E e $7F
            (0x7E, _) => self.wram[addr as usize] = val,
            (0x7F, _) => self.wram[0x10000 + addr as usize] = val,

            // Espelho WRAM
            (0x00..=0x3F | 0x80..=0xBF, 0x0000..=0x1FFF) => {
                self.wram[addr as usize] = val;
            }

            // PPU Registers ($2100 - $213F)
            (0x00..=0x3F | 0x80..=0xBF, 0x2100..=0x213F) => {
                self.ppu.write_reg(addr, val);
            }

            // Portas APU I/O ($2140 - $2143): Ecoa o handshake da CPU para liberar o boot
            (0x00..=0x3F | 0x80..=0xBF, 0x2140..=0x2143) => {
                let idx = (addr - 0x2140) as usize;
                self.apu_ports[idx] = val;
            }

            // CPU Control & DMA ($4200 - $421F)
            (0x00..=0x3F | 0x80..=0xBF, 0x4200) => {
                self.nmitimen = val;
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4202) => {
                self.wrmpya = val;
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4203) => {
                self.wrmpyb = val;
                self.rdmpy_product_or_remainder = (self.wrmpya as u16) * (self.wrmpyb as u16);
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4204) => {
                self.wrdiv_dividend = (self.wrdiv_dividend & 0xFF00) | (val as u16);
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4205) => {
                self.wrdiv_dividend = (self.wrdiv_dividend & 0x00FF) | ((val as u16) << 8);
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x4206) => {
                self.wrdiv_divisor = val;
                if val == 0 {
                    self.rddiv_quotient = 0xFFFF;
                    self.rdmpy_product_or_remainder = self.wrdiv_dividend;
                } else {
                    self.rddiv_quotient = self.wrdiv_dividend / (val as u16);
                    self.rdmpy_product_or_remainder = self.wrdiv_dividend % (val as u16);
                }
            }
            (0x00..=0x3F | 0x80..=0xBF, 0x420B) => {
                // MDMAEN: Dispara transferência DMA imediata nos canais ativos
                self.trigger_dma(val);
            }

            // DMA Registers ($4300 - $437F)
            (0x00..=0x3F | 0x80..=0xBF, 0x4300..=0x437F) => {
                let ch = ((addr >> 4) & 0x07) as usize;
                let reg = (addr & 0x0F) as u8;
                let channel = &mut self.dma[ch];
                match reg {
                    0x0 => channel.control = val,
                    0x1 => channel.dest_reg = val,
                    0x2 => channel.src_addr = (channel.src_addr & 0xFF00) | (val as u16),
                    0x3 => channel.src_addr = (channel.src_addr & 0x00FF) | ((val as u16) << 8),
                    0x4 => channel.src_bank = val,
                    0x5 => channel.byte_count = (channel.byte_count & 0xFF00) | (val as u16),
                    0x6 => channel.byte_count = (channel.byte_count & 0x00FF) | ((val as u16) << 8),
                    0x7 => channel.hdma_bank = val,
                    _ => {}
                }
            }

            // Cartucho (SRAM / Mapeador)
            _ => {
                if let Some(ref mut cart) = self.cartridge {
                    cart.write(bank, addr, val);
                }
            }
        }
    }

    #[inline(always)]
    pub fn read_u16(&mut self, bank: u8, addr: u16) -> u16 {
        let low = self.read_u8(bank, addr) as u16;
        let high = self.read_u8(bank, addr.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    #[inline(always)]
    pub fn write_u16(&mut self, bank: u8, addr: u16, val: u16) {
        self.write_u8(bank, addr, (val & 0xFF) as u8);
        self.write_u8(bank, addr.wrapping_add(1), ((val >> 8) & 0xFF) as u8);
    }

    #[inline(always)]
    pub fn read_u24(&mut self, bank: u8, addr: u16) -> (u8, u16) {
        let low = self.read_u8(bank, addr) as u16;
        let high = self.read_u8(bank, addr.wrapping_add(1)) as u16;
        let b = self.read_u8(bank, addr.wrapping_add(2));
        (b, (high << 8) | low)
    }

    // =========================================================================
    // Motor DMA de Uso Geral (Bus A -> Bus B)
    // =========================================================================
    pub fn trigger_dma(&mut self, channel_mask: u8) {
        for ch in 0..8 {
            if (channel_mask & (1 << ch)) == 0 {
                continue;
            }

            let control = self.dma[ch].control;
            let dest_b = 0x2100 | (self.dma[ch].dest_reg as u16);
            let mut src_addr = self.dma[ch].src_addr;
            let src_bank = self.dma[ch].src_bank;
            let count = if self.dma[ch].byte_count == 0 {
                0x10000
            } else {
                self.dma[ch].byte_count as usize
            };

            let step: i32 = match (control >> 3) & 0x03 {
                0 => 1,  // Increment
                2 => -1, // Decrement
                _ => 0,  // Fixed
            };

            let mode = control & 0x07;
            for i in 0..count {
                let current_dest = match mode {
                    0 => dest_b,
                    1 => dest_b + ((i % 2) as u16),
                    2 => dest_b,
                    3 | 7 => dest_b + (((i >> 1) % 2) as u16),
                    _ => dest_b + ((i % 4) as u16),
                };

                // Barramento A -> Barramento B
                if (control & 0x80) == 0 {
                    let byte = self.read_u8(src_bank, src_addr);
                    self.write_u8(0, current_dest, byte);
                } else {
                    let byte = self.read_u8(0, current_dest);
                    self.write_u8(src_bank, src_addr, byte);
                }

                src_addr = ((src_addr as i32) + step) as u16;
            }

            self.dma[ch].src_addr = src_addr;
            self.dma[ch].byte_count = 0;
        }
    }
}
