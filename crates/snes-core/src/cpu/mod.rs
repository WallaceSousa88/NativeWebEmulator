pub mod registers;

use crate::bus::Bus;
use crate::cpu::registers::{flags, Registers};

macro_rules! op_at {
    ($self:ident, $bus:ident, $addr_expr:ident, $op_expr:ident, $cyc:expr) => {{
        let target = $self.$addr_expr($bus);
        $self.$op_expr($bus, target, $cyc)
    }};
}

pub struct Cpu {
    pub regs: Registers,
    pub cycles: u64,
    pub is_waiting: bool,
    pub last_opcode: u8,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            regs: Registers::new(),
            cycles: 0,
            is_waiting: false,
            last_opcode: 0,
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) {
        self.regs = Registers::new();
        self.cycles = 0;
        self.is_waiting = false;
        self.last_opcode = 0;

        // Reset vector no modo de emulação lê em $00:FFFC
        let reset_vector = bus.read_u16(0x00, 0xFFFC);
        self.regs.pc = reset_vector;
        self.regs.pbr = 0x00;
        self.regs.dbr = 0x00;
    }

    // =========================================================================
    // Operações de Pilha (Stack Push / Pull)
    // =========================================================================
    pub fn push_u8(&mut self, bus: &mut Bus, val: u8) {
        bus.write_u8(0x00, self.regs.sp, val);
        if self.regs.e {
            self.regs.sp = 0x0100 | ((self.regs.sp.wrapping_sub(1)) & 0xFF);
        } else {
            self.regs.sp = self.regs.sp.wrapping_sub(1);
        }
    }

    pub fn pull_u8(&mut self, bus: &mut Bus) -> u8 {
        if self.regs.e {
            self.regs.sp = 0x0100 | ((self.regs.sp.wrapping_add(1)) & 0xFF);
        } else {
            self.regs.sp = self.regs.sp.wrapping_add(1);
        }
        bus.read_u8(0x00, self.regs.sp)
    }

    pub fn push_u16(&mut self, bus: &mut Bus, val: u16) {
        self.push_u8(bus, ((val >> 8) & 0xFF) as u8);
        self.push_u8(bus, (val & 0xFF) as u8);
    }

    pub fn pull_u16(&mut self, bus: &mut Bus) -> u16 {
        let low = self.pull_u8(bus) as u16;
        let high = self.pull_u8(bus) as u16;
        (high << 8) | low
    }

    // =========================================================================
    // Busca de Instruções e Operandos (Fetch)
    // =========================================================================
    #[inline(always)]
    pub fn fetch_u8(&mut self, bus: &mut Bus) -> u8 {
        let byte = bus.read_u8(self.regs.pbr, self.regs.pc);
        self.regs.pc = self.regs.pc.wrapping_add(1);
        byte
    }

    #[inline(always)]
    pub fn fetch_u16(&mut self, bus: &mut Bus) -> u16 {
        let low = self.fetch_u8(bus) as u16;
        let high = self.fetch_u8(bus) as u16;
        (high << 8) | low
    }

    // =========================================================================
    // Resolução de Modos de Endereçamento de 24 Bits
    // =========================================================================
    #[inline(always)]
    fn addr_dp(&mut self, bus: &mut Bus) -> (u8, u16) {
        let offset = self.fetch_u8(bus) as u16;
        (0x00, self.regs.dp.wrapping_add(offset))
    }

    #[inline(always)]
    fn addr_dp_x(&mut self, bus: &mut Bus) -> (u8, u16) {
        let offset = self.fetch_u8(bus) as u16;
        (0x00, self.regs.dp.wrapping_add(offset).wrapping_add(self.regs.x))
    }

    #[inline(always)]
    fn addr_dp_y(&mut self, bus: &mut Bus) -> (u8, u16) {
        let offset = self.fetch_u8(bus) as u16;
        (0x00, self.regs.dp.wrapping_add(offset).wrapping_add(self.regs.y))
    }

    #[inline(always)]
    fn addr_dp_ind(&mut self, bus: &mut Bus) -> (u8, u16) {
        let offset = self.fetch_u8(bus) as u16;
        let ptr = bus.read_u16(0x00, self.regs.dp.wrapping_add(offset));
        (self.regs.dbr, ptr)
    }

    #[inline(always)]
    fn addr_dp_ind_long(&mut self, bus: &mut Bus) -> (u8, u16) {
        let offset = self.fetch_u8(bus) as u16;
        let (b, ptr) = bus.read_u24(0x00, self.regs.dp.wrapping_add(offset));
        (b, ptr)
    }

    #[inline(always)]
    fn addr_dp_ind_y(&mut self, bus: &mut Bus) -> (u8, u16) {
        let offset = self.fetch_u8(bus) as u16;
        let ptr = bus.read_u16(0x00, self.regs.dp.wrapping_add(offset));
        (self.regs.dbr, ptr.wrapping_add(self.regs.y))
    }

    #[inline(always)]
    fn addr_dp_x_ind(&mut self, bus: &mut Bus) -> (u8, u16) {
        let offset = self.fetch_u8(bus) as u16;
        let dp_addr = self.regs.dp.wrapping_add(offset).wrapping_add(self.regs.x);
        let ptr = bus.read_u16(0x00, dp_addr);
        (self.regs.dbr, ptr)
    }

    #[inline(always)]
    fn addr_abs(&mut self, bus: &mut Bus) -> (u8, u16) {
        let addr = self.fetch_u16(bus);
        (self.regs.dbr, addr)
    }

    #[inline(always)]
    fn addr_abs_x(&mut self, bus: &mut Bus) -> (u8, u16) {
        let addr = self.fetch_u16(bus);
        (self.regs.dbr, addr.wrapping_add(self.regs.x))
    }

    #[inline(always)]
    fn addr_abs_y(&mut self, bus: &mut Bus) -> (u8, u16) {
        let addr = self.fetch_u16(bus);
        (self.regs.dbr, addr.wrapping_add(self.regs.y))
    }

    #[inline(always)]
    fn addr_long(&mut self, bus: &mut Bus) -> (u8, u16) {
        let addr = self.fetch_u16(bus);
        let bank = self.fetch_u8(bus);
        (bank, addr)
    }

    #[inline(always)]
    fn addr_long_x(&mut self, bus: &mut Bus) -> (u8, u16) {
        let addr = self.fetch_u16(bus);
        let bank = self.fetch_u8(bus);
        (bank, addr.wrapping_add(self.regs.x))
    }

    #[inline(always)]
    fn addr_sr(&mut self, bus: &mut Bus) -> (u8, u16) {
        let offset = self.fetch_u8(bus) as u16;
        (0x00, self.regs.sp.wrapping_add(offset))
    }

    // =========================================================================
    // Interrupção NMI (Disparada no V-Blank)
    // =========================================================================
    pub fn trigger_nmi(&mut self, bus: &mut Bus) {
        if self.regs.e {
            self.push_u16(bus, self.regs.pc);
            self.push_u8(bus, self.regs.p & !flags::X);
            self.regs.set_flag(flags::I, true);
            self.regs.set_flag(flags::D, false);
            self.regs.pc = bus.read_u16(0x00, 0xFFFA);
            self.regs.pbr = 0;
        } else {
            self.push_u8(bus, self.regs.pbr);
            self.push_u16(bus, self.regs.pc);
            self.push_u8(bus, self.regs.p);
            self.regs.set_flag(flags::I, true);
            self.regs.set_flag(flags::D, false);
            self.regs.pc = bus.read_u16(0x00, 0xFFEA);
            self.regs.pbr = 0;
        }
        self.is_waiting = false;
        self.cycles += 7;
    }

    // =========================================================================
    // Execução Passo a Passo da CPU
    // =========================================================================
    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        if self.is_waiting {
            return 1;
        }

        let op = self.fetch_u8(bus);
        self.last_opcode = op;
        self.execute_opcode(bus, op)
    }

    fn execute_opcode(&mut self, bus: &mut Bus, op: u8) -> u32 {
        let m_8 = self.regs.is_m_8bit();
        let x_8 = self.regs.is_x_8bit();

        match op {
            0xEA => 2, // NOP
            0xCB => { self.is_waiting = true; 3 } // WAI
            0xDB => { self.is_waiting = true; 3 } // STP

            // Flags
            0x18 => { self.regs.set_flag(flags::C, false); 2 } // CLC
            0x38 => { self.regs.set_flag(flags::C, true); 2 }  // SEC
            0x58 => { self.regs.set_flag(flags::I, false); 2 } // CLI
            0x78 => { self.regs.set_flag(flags::I, true); 2 }  // SEI
            0xD8 => { self.regs.set_flag(flags::D, false); 2 } // CLD
            0xF8 => { self.regs.set_flag(flags::D, true); 2 }  // SED
            0xB8 => { self.regs.set_flag(flags::V, false); 2 } // CLV

            0xFB => { // XCE (Exchange Carry and Emulation)
                let c = self.regs.get_flag(flags::C);
                let e = self.regs.e;
                self.regs.set_flag(flags::C, e);
                self.regs.e = c;
                if self.regs.e {
                    self.regs.p |= flags::M | flags::X;
                    self.regs.sp = 0x0100 | (self.regs.sp & 0xFF);
                }
                2
            }
            0xC2 => { // REP
                let mask = self.fetch_u8(bus);
                self.regs.p &= !mask;
                3
            }
            0xE2 => { // SEP
                let mask = self.fetch_u8(bus);
                self.regs.p |= mask;
                3
            }

            // Stack Pushes / Pulls
            0x48 => { // PHA
                if m_8 { self.push_u8(bus, (self.regs.a & 0xFF) as u8); 3 }
                else { self.push_u16(bus, self.regs.a); 4 }
            }
            0x68 => { // PLA
                if m_8 {
                    let v = self.pull_u8(bus);
                    self.regs.a = (self.regs.a & 0xFF00) | (v as u16);
                    self.regs.set_nz_8(v);
                    4
                } else {
                    let v = self.pull_u16(bus);
                    self.regs.a = v;
                    self.regs.set_nz_16(v);
                    5
                }
            }
            0x08 => { self.push_u8(bus, self.regs.p); 3 } // PHP
            0x28 => { self.regs.p = self.pull_u8(bus); 4 } // PLP
            0xDA => { // PHX
                if x_8 { self.push_u8(bus, (self.regs.x & 0xFF) as u8); 3 }
                else { self.push_u16(bus, self.regs.x); 4 }
            }
            0xFA => { // PLX
                if x_8 {
                    let v = self.pull_u8(bus);
                    self.regs.x = v as u16;
                    self.regs.set_nz_8(v);
                    4
                } else {
                    let v = self.pull_u16(bus);
                    self.regs.x = v;
                    self.regs.set_nz_16(v);
                    5
                }
            }
            0x5A => { // PHY
                if x_8 { self.push_u8(bus, (self.regs.y & 0xFF) as u8); 3 }
                else { self.push_u16(bus, self.regs.y); 4 }
            }
            0x7A => { // PLY
                if x_8 {
                    let v = self.pull_u8(bus);
                    self.regs.y = v as u16;
                    self.regs.set_nz_8(v);
                    4
                } else {
                    let v = self.pull_u16(bus);
                    self.regs.y = v;
                    self.regs.set_nz_16(v);
                    5
                }
            }
            0x8B => { self.push_u8(bus, self.regs.dbr); 3 } // PHB
            0xAB => { // PLB
                self.regs.dbr = self.pull_u8(bus);
                self.regs.set_nz_8(self.regs.dbr);
                4
            }
            0x4B => { self.push_u8(bus, self.regs.pbr); 3 } // PHK
            0x0B => { self.push_u16(bus, self.regs.dp); 4 }  // PHD
            0x2B => { // PLD
                self.regs.dp = self.pull_u16(bus);
                self.regs.set_nz_16(self.regs.dp);
                5
            }
            0xF4 => { // PEA
                let val = self.fetch_u16(bus);
                self.push_u16(bus, val);
                5
            }

            // LDA
            0xA9 => { // Imm
                if m_8 {
                    let v = self.fetch_u8(bus);
                    self.regs.a = (self.regs.a & 0xFF00) | (v as u16);
                    self.regs.set_nz_8(v);
                    2
                } else {
                    let v = self.fetch_u16(bus);
                    self.regs.a = v;
                    self.regs.set_nz_16(v);
                    3
                }
            }
            0xA5 => op_at!(self, bus, addr_dp, op_lda, 3),
            0xB5 => op_at!(self, bus, addr_dp_x, op_lda, 4),
            0xB2 => op_at!(self, bus, addr_dp_ind, op_lda, 5),
            0xA1 => op_at!(self, bus, addr_dp_x_ind, op_lda, 6),
            0xB1 => op_at!(self, bus, addr_dp_ind_y, op_lda, 5),
            0xB7 => op_at!(self, bus, addr_dp_ind_long, op_lda, 6),
            0xAD => op_at!(self, bus, addr_abs, op_lda, 4),
            0xBD => op_at!(self, bus, addr_abs_x, op_lda, 4),
            0xB9 => op_at!(self, bus, addr_abs_y, op_lda, 4),
            0xAF => op_at!(self, bus, addr_long, op_lda, 5),
            0xBF => op_at!(self, bus, addr_long_x, op_lda, 5),
            0xA3 => op_at!(self, bus, addr_sr, op_lda, 4),

            // STA
            0x85 => op_at!(self, bus, addr_dp, op_sta, 3),
            0x95 => op_at!(self, bus, addr_dp_x, op_sta, 4),
            0x92 => op_at!(self, bus, addr_dp_ind, op_sta, 5),
            0x81 => op_at!(self, bus, addr_dp_x_ind, op_sta, 6),
            0x91 => op_at!(self, bus, addr_dp_ind_y, op_sta, 6),
            0x97 => op_at!(self, bus, addr_dp_ind_long, op_sta, 6),
            0x8D => op_at!(self, bus, addr_abs, op_sta, 4),
            0x9D => op_at!(self, bus, addr_abs_x, op_sta, 5),
            0x99 => op_at!(self, bus, addr_abs_y, op_sta, 5),
            0x8F => op_at!(self, bus, addr_long, op_sta, 5),
            0x9F => op_at!(self, bus, addr_long_x, op_sta, 5),
            0x83 => op_at!(self, bus, addr_sr, op_sta, 4),

            // STZ
            0x64 => op_at!(self, bus, addr_dp, op_stz, 3),
            0x74 => op_at!(self, bus, addr_dp_x, op_stz, 4),
            0x9C => op_at!(self, bus, addr_abs, op_stz, 4),
            0x9E => op_at!(self, bus, addr_abs_x, op_stz, 5),

            // LDX
            0xA2 => { // Imm
                if x_8 {
                    let v = self.fetch_u8(bus);
                    self.regs.x = v as u16;
                    self.regs.set_nz_8(v);
                    2
                } else {
                    let v = self.fetch_u16(bus);
                    self.regs.x = v;
                    self.regs.set_nz_16(v);
                    3
                }
            }
            0xA6 => op_at!(self, bus, addr_dp, op_ldx, 3),
            0xB6 => op_at!(self, bus, addr_dp_y, op_ldx, 4),
            0xAE => op_at!(self, bus, addr_abs, op_ldx, 4),
            0xBE => op_at!(self, bus, addr_abs_y, op_ldx, 4),

            // STX
            0x86 => op_at!(self, bus, addr_dp, op_stx, 3),
            0x96 => op_at!(self, bus, addr_dp_y, op_stx, 4),
            0x8E => op_at!(self, bus, addr_abs, op_stx, 4),

            // LDY
            0xA0 => { // Imm
                if x_8 {
                    let v = self.fetch_u8(bus);
                    self.regs.y = v as u16;
                    self.regs.set_nz_8(v);
                    2
                } else {
                    let v = self.fetch_u16(bus);
                    self.regs.y = v;
                    self.regs.set_nz_16(v);
                    3
                }
            }
            0xA4 => op_at!(self, bus, addr_dp, op_ldy, 3),
            0xB4 => op_at!(self, bus, addr_dp_x, op_ldy, 4),
            0xAC => op_at!(self, bus, addr_abs, op_ldy, 4),
            0xBC => op_at!(self, bus, addr_abs_y, op_ldy, 4),

            // STY
            0x84 => op_at!(self, bus, addr_dp, op_sty, 3),
            0x94 => op_at!(self, bus, addr_dp_x, op_sty, 4),
            0x8C => op_at!(self, bus, addr_abs, op_sty, 4),

            // Register Transfers
            0xAA => { // TAX
                if x_8 { self.regs.x = self.regs.a & 0xFF; self.regs.set_nz_8(self.regs.x as u8); }
                else { self.regs.x = self.regs.a; self.regs.set_nz_16(self.regs.x); }
                2
            }
            0x8A => { // TXA
                if m_8 { self.regs.a = (self.regs.a & 0xFF00) | (self.regs.x & 0xFF); self.regs.set_nz_8((self.regs.a & 0xFF) as u8); }
                else { self.regs.a = self.regs.x; self.regs.set_nz_16(self.regs.a); }
                2
            }
            0xA8 => { // TAY
                if x_8 { self.regs.y = self.regs.a & 0xFF; self.regs.set_nz_8(self.regs.y as u8); }
                else { self.regs.y = self.regs.a; self.regs.set_nz_16(self.regs.y); }
                2
            }
            0x98 => { // TYA
                if m_8 { self.regs.a = (self.regs.a & 0xFF00) | (self.regs.y & 0xFF); self.regs.set_nz_8((self.regs.a & 0xFF) as u8); }
                else { self.regs.a = self.regs.y; self.regs.set_nz_16(self.regs.a); }
                2
            }
            0xBA => { // TSX
                self.regs.x = self.regs.sp;
                if x_8 { self.regs.x &= 0xFF; self.regs.set_nz_8(self.regs.x as u8); }
                else { self.regs.set_nz_16(self.regs.x); }
                2
            }
            0x9A => { // TXS
                if self.regs.e { self.regs.sp = 0x0100 | (self.regs.x & 0xFF); }
                else { self.regs.sp = self.regs.x; }
                2
            }
            0xBB => { // TYX
                self.regs.x = self.regs.y;
                if x_8 { self.regs.x &= 0xFF; self.regs.set_nz_8(self.regs.x as u8); }
                else { self.regs.set_nz_16(self.regs.x); }
                2
            }
            0x9B => { // TXY
                self.regs.y = self.regs.x;
                if x_8 { self.regs.y &= 0xFF; self.regs.set_nz_8(self.regs.y as u8); }
                else { self.regs.set_nz_16(self.regs.y); }
                2
            }
            0x5B => { // TCD
                self.regs.dp = self.regs.a;
                self.regs.set_nz_16(self.regs.dp);
                2
            }
            0x7B => { // TDC
                self.regs.a = self.regs.dp;
                self.regs.set_nz_16(self.regs.a);
                2
            }
            0x1B => { // TCS
                self.regs.sp = self.regs.a;
                2
            }
            0x3B => { // TSC
                self.regs.a = self.regs.sp;
                self.regs.set_nz_16(self.regs.a);
                2
            }

            // Incs & Decs
            0xE8 => { // INX
                if x_8 { let v = ((self.regs.x & 0xFF) as u8).wrapping_add(1); self.regs.x = v as u16; self.regs.set_nz_8(v); }
                else { self.regs.x = self.regs.x.wrapping_add(1); self.regs.set_nz_16(self.regs.x); }
                2
            }
            0xCA => { // DEX
                if x_8 { let v = ((self.regs.x & 0xFF) as u8).wrapping_sub(1); self.regs.x = v as u16; self.regs.set_nz_8(v); }
                else { self.regs.x = self.regs.x.wrapping_sub(1); self.regs.set_nz_16(self.regs.x); }
                2
            }
            0xC8 => { // INY
                if x_8 { let v = ((self.regs.y & 0xFF) as u8).wrapping_add(1); self.regs.y = v as u16; self.regs.set_nz_8(v); }
                else { self.regs.y = self.regs.y.wrapping_add(1); self.regs.set_nz_16(self.regs.y); }
                2
            }
            0x88 => { // DEY
                if x_8 { let v = ((self.regs.y & 0xFF) as u8).wrapping_sub(1); self.regs.y = v as u16; self.regs.set_nz_8(v); }
                else { self.regs.y = self.regs.y.wrapping_sub(1); self.regs.set_nz_16(self.regs.y); }
                2
            }
            0x1A => { // INC A
                if m_8 { let v = ((self.regs.a & 0xFF) as u8).wrapping_add(1); self.regs.a = (self.regs.a & 0xFF00) | (v as u16); self.regs.set_nz_8(v); }
                else { self.regs.a = self.regs.a.wrapping_add(1); self.regs.set_nz_16(self.regs.a); }
                2
            }
            0x3A => { // DEC A
                if m_8 { let v = ((self.regs.a & 0xFF) as u8).wrapping_sub(1); self.regs.a = (self.regs.a & 0xFF00) | (v as u16); self.regs.set_nz_8(v); }
                else { self.regs.a = self.regs.a.wrapping_sub(1); self.regs.set_nz_16(self.regs.a); }
                2
            }

            // Jumps / Subroutines
            0x4C => { let t = self.fetch_u16(bus); self.regs.pc = t; 3 } // JMP abs
            0x6C => { // JMP (abs)
                let p = self.fetch_u16(bus);
                self.regs.pc = bus.read_u16(0x00, p);
                5
            }
            0x7C => { // JMP (abs, X)
                let p = self.fetch_u16(bus).wrapping_add(self.regs.x);
                self.regs.pc = bus.read_u16(self.regs.pbr, p);
                6
            }
            0x5C => { // JML long
                let pc = self.fetch_u16(bus);
                let b = self.fetch_u8(bus);
                self.regs.pc = pc;
                self.regs.pbr = b;
                4
            }
            0xDC => { // JML [abs]
                let p = self.fetch_u16(bus);
                let (b, pc) = bus.read_u24(0x00, p);
                self.regs.pc = pc;
                self.regs.pbr = b;
                6
            }
            0x20 => { // JSR abs
                let t = self.fetch_u16(bus);
                let r = self.regs.pc.wrapping_sub(1);
                self.push_u16(bus, r);
                self.regs.pc = t;
                6
            }
            0x22 => { // JSL long
                let pc = self.fetch_u16(bus);
                let b = self.fetch_u8(bus);
                let r = self.regs.pc.wrapping_sub(1);
                self.push_u8(bus, self.regs.pbr);
                self.push_u16(bus, r);
                self.regs.pc = pc;
                self.regs.pbr = b;
                8
            }
            0x60 => { // RTS
                let r = self.pull_u16(bus);
                self.regs.pc = r.wrapping_add(1);
                6
            }
            0x6B => { // RTL
                let r = self.pull_u16(bus);
                let b = self.pull_u8(bus);
                self.regs.pc = r.wrapping_add(1);
                self.regs.pbr = b;
                6
            }
            0x40 => { // RTI
                self.regs.p = self.pull_u8(bus);
                self.regs.pc = self.pull_u16(bus);
                if !self.regs.e { self.regs.pbr = self.pull_u8(bus); }
                6
            }

            // Branches
            0x80 => { // BRA
                let off = self.fetch_u8(bus) as i8 as i16;
                self.regs.pc = ((self.regs.pc as i32) + (off as i32)) as u16;
                3
            }
            0x82 => { // BRL
                let off = self.fetch_u16(bus) as i16;
                self.regs.pc = ((self.regs.pc as i32) + (off as i32)) as u16;
                4
            }
            0xF0 => self.branch_if(bus, self.regs.get_flag(flags::Z)),  // BEQ
            0xD0 => self.branch_if(bus, !self.regs.get_flag(flags::Z)), // BNE
            0xB0 => self.branch_if(bus, self.regs.get_flag(flags::C)),  // BCS
            0x90 => self.branch_if(bus, !self.regs.get_flag(flags::C)), // BCC
            0x30 => self.branch_if(bus, self.regs.get_flag(flags::N)),  // BMI
            0x10 => self.branch_if(bus, !self.regs.get_flag(flags::N)), // BPL
            0x70 => self.branch_if(bus, self.regs.get_flag(flags::V)),  // BVS
            0x50 => self.branch_if(bus, !self.regs.get_flag(flags::V)), // BVC

            // Comparisons: CMP, CPX, CPY
            0xC9 => { // CMP Imm
                if m_8 {
                    let v = self.fetch_u8(bus);
                    let a = (self.regs.a & 0xFF) as u8;
                    self.regs.set_flag(flags::C, a >= v);
                    self.regs.set_nz_8(a.wrapping_sub(v));
                    2
                } else {
                    let v = self.fetch_u16(bus);
                    self.regs.set_flag(flags::C, self.regs.a >= v);
                    self.regs.set_nz_16(self.regs.a.wrapping_sub(v));
                    3
                }
            }
            0xC5 => op_at!(self, bus, addr_dp, op_cmp, 3),
            0xD5 => op_at!(self, bus, addr_dp_x, op_cmp, 4),
            0xCD => op_at!(self, bus, addr_abs, op_cmp, 4),
            0xDD => op_at!(self, bus, addr_abs_x, op_cmp, 4),
            0xD9 => op_at!(self, bus, addr_abs_y, op_cmp, 4),
            0xCF => op_at!(self, bus, addr_long, op_cmp, 5),

            0xE0 => { // CPX Imm
                if x_8 {
                    let v = self.fetch_u8(bus);
                    let x = (self.regs.x & 0xFF) as u8;
                    self.regs.set_flag(flags::C, x >= v);
                    self.regs.set_nz_8(x.wrapping_sub(v));
                    2
                } else {
                    let v = self.fetch_u16(bus);
                    self.regs.set_flag(flags::C, self.regs.x >= v);
                    self.regs.set_nz_16(self.regs.x.wrapping_sub(v));
                    3
                }
            }
            0xE4 => op_at!(self, bus, addr_dp, op_cpx, 3),
            0xEC => op_at!(self, bus, addr_abs, op_cpx, 4),

            0xC0 => { // CPY Imm
                if x_8 {
                    let v = self.fetch_u8(bus);
                    let y = (self.regs.y & 0xFF) as u8;
                    self.regs.set_flag(flags::C, y >= v);
                    self.regs.set_nz_8(y.wrapping_sub(v));
                    2
                } else {
                    let v = self.fetch_u16(bus);
                    self.regs.set_flag(flags::C, self.regs.y >= v);
                    self.regs.set_nz_16(self.regs.y.wrapping_sub(v));
                    3
                }
            }
            0xC4 => op_at!(self, bus, addr_dp, op_cpy, 3),
            0xCC => op_at!(self, bus, addr_abs, op_cpy, 4),

            // Arithmetic: ADC
            0x69 => { // ADC Imm
                if m_8 { let v = self.fetch_u8(bus); self.do_adc_8(v); 2 }
                else { let v = self.fetch_u16(bus); self.do_adc_16(v); 3 }
            }
            0x65 => op_at!(self, bus, addr_dp, op_adc, 3),
            0x75 => op_at!(self, bus, addr_dp_x, op_adc, 4),
            0x6D => op_at!(self, bus, addr_abs, op_adc, 4),
            0x7D => op_at!(self, bus, addr_abs_x, op_adc, 4),
            0x79 => op_at!(self, bus, addr_abs_y, op_adc, 4),

            // Arithmetic: SBC
            0xE9 => { // SBC Imm
                if m_8 { let v = self.fetch_u8(bus); self.do_sbc_8(v); 2 }
                else { let v = self.fetch_u16(bus); self.do_sbc_16(v); 3 }
            }
            0xE5 => op_at!(self, bus, addr_dp, op_sbc, 3),
            0xF5 => op_at!(self, bus, addr_dp_x, op_sbc, 4),
            0xED => op_at!(self, bus, addr_abs, op_sbc, 4),
            0xFD => op_at!(self, bus, addr_abs_x, op_sbc, 4),
            0xF9 => op_at!(self, bus, addr_abs_y, op_sbc, 4),

            // Logical: AND
            0x29 => { // Imm
                if m_8 { let v = self.fetch_u8(bus); self.regs.a = (self.regs.a & 0xFF00) | ((self.regs.a & v as u16) & 0xFF); self.regs.set_nz_8((self.regs.a & 0xFF) as u8); 2 }
                else { let v = self.fetch_u16(bus); self.regs.a &= v; self.regs.set_nz_16(self.regs.a); 3 }
            }
            0x25 => op_at!(self, bus, addr_dp, op_and, 3),
            0x2D => op_at!(self, bus, addr_abs, op_and, 4),

            // Logical: ORA
            0x09 => { // Imm
                if m_8 { let v = self.fetch_u8(bus); self.regs.a |= v as u16; self.regs.set_nz_8((self.regs.a & 0xFF) as u8); 2 }
                else { let v = self.fetch_u16(bus); self.regs.a |= v; self.regs.set_nz_16(self.regs.a); 3 }
            }
            0x05 => op_at!(self, bus, addr_dp, op_ora, 3),
            0x0D => op_at!(self, bus, addr_abs, op_ora, 4),

            // Logical: EOR
            0x49 => { // Imm
                if m_8 { let v = self.fetch_u8(bus); self.regs.a = (self.regs.a & 0xFF00) | ((self.regs.a ^ v as u16) & 0xFF); self.regs.set_nz_8((self.regs.a & 0xFF) as u8); 2 }
                else { let v = self.fetch_u16(bus); self.regs.a ^= v; self.regs.set_nz_16(self.regs.a); 3 }
            }
            0x45 => op_at!(self, bus, addr_dp, op_eor, 3),
            0x4D => op_at!(self, bus, addr_abs, op_eor, 4),

            // BIT
            0x89 => { // BIT Imm
                if m_8 {
                    let v = self.fetch_u8(bus);
                    let res = (self.regs.a as u8) & v;
                    self.regs.set_flag(flags::Z, res == 0);
                    2
                } else {
                    let v = self.fetch_u16(bus);
                    let res = self.regs.a & v;
                    self.regs.set_flag(flags::Z, res == 0);
                    3
                }
            }
            0x24 => { // BIT dp
                let (b, a) = self.addr_dp(bus);
                if m_8 {
                    let mem = bus.read_u8(b, a);
                    self.regs.set_flag(flags::Z, ((self.regs.a as u8) & mem) == 0);
                    self.regs.set_flag(flags::N, (mem & 0x80) != 0);
                    self.regs.set_flag(flags::V, (mem & 0x40) != 0);
                } else {
                    let mem = bus.read_u16(b, a);
                    self.regs.set_flag(flags::Z, (self.regs.a & mem) == 0);
                    self.regs.set_flag(flags::N, (mem & 0x8000) != 0);
                    self.regs.set_flag(flags::V, (mem & 0x4000) != 0);
                }
                3
            }
            0x2C => { // BIT abs
                let (b, a) = self.addr_abs(bus);
                if m_8 {
                    let mem = bus.read_u8(b, a);
                    self.regs.set_flag(flags::Z, ((self.regs.a as u8) & mem) == 0);
                    self.regs.set_flag(flags::N, (mem & 0x80) != 0);
                    self.regs.set_flag(flags::V, (mem & 0x40) != 0);
                } else {
                    let mem = bus.read_u16(b, a);
                    self.regs.set_flag(flags::Z, (self.regs.a & mem) == 0);
                    self.regs.set_flag(flags::N, (mem & 0x8000) != 0);
                    self.regs.set_flag(flags::V, (mem & 0x4000) != 0);
                }
                4
            }

            // Block moves: MVN / MVP
            0x54 | 0x44 => {
                let dest_bank = self.fetch_u8(bus);
                let src_bank = self.fetch_u8(bus);
                let byte = bus.read_u8(src_bank, self.regs.x);
                bus.write_u8(dest_bank, self.regs.y, byte);
                if op == 0x54 { // MVN (Increment)
                    self.regs.x = self.regs.x.wrapping_add(1);
                    self.regs.y = self.regs.y.wrapping_add(1);
                } else { // MVP (Decrement)
                    self.regs.x = self.regs.x.wrapping_sub(1);
                    self.regs.y = self.regs.y.wrapping_sub(1);
                }
                self.regs.a = self.regs.a.wrapping_sub(1);
                if self.regs.a != 0xFFFF {
                    self.regs.pc = self.regs.pc.wrapping_sub(3);
                }
                7
            }

            // Fallback para opcodes desconhecidos: consome 2 ciclos e não trava o emulador
            _ => 2,
        }
    }

    // Helpers
    fn branch_if(&mut self, bus: &mut Bus, cond: bool) -> u32 {
        let off = self.fetch_u8(bus) as i8 as i16;
        if cond {
            self.regs.pc = ((self.regs.pc as i32) + (off as i32)) as u16;
            3
        } else {
            2
        }
    }

    fn op_lda(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_m_8bit() {
            let v = bus.read_u8(b, a);
            self.regs.a = (self.regs.a & 0xFF00) | (v as u16);
            self.regs.set_nz_8(v);
        } else {
            let v = bus.read_u16(b, a);
            self.regs.a = v;
            self.regs.set_nz_16(v);
        }
        cyc
    }

    fn op_sta(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_m_8bit() {
            bus.write_u8(b, a, (self.regs.a & 0xFF) as u8);
        } else {
            bus.write_u16(b, a, self.regs.a);
        }
        cyc
    }

    fn op_stz(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_m_8bit() {
            bus.write_u8(b, a, 0);
        } else {
            bus.write_u16(b, a, 0);
        }
        cyc
    }

    fn op_ldx(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_x_8bit() {
            let v = bus.read_u8(b, a);
            self.regs.x = v as u16;
            self.regs.set_nz_8(v);
        } else {
            let v = bus.read_u16(b, a);
            self.regs.x = v;
            self.regs.set_nz_16(v);
        }
        cyc
    }

    fn op_stx(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_x_8bit() {
            bus.write_u8(b, a, (self.regs.x & 0xFF) as u8);
        } else {
            bus.write_u16(b, a, self.regs.x);
        }
        cyc
    }

    fn op_ldy(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_x_8bit() {
            let v = bus.read_u8(b, a);
            self.regs.y = v as u16;
            self.regs.set_nz_8(v);
        } else {
            let v = bus.read_u16(b, a);
            self.regs.y = v;
            self.regs.set_nz_16(v);
        }
        cyc
    }

    fn op_sty(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_x_8bit() {
            bus.write_u8(b, a, (self.regs.y & 0xFF) as u8);
        } else {
            bus.write_u16(b, a, self.regs.y);
        }
        cyc
    }

    fn op_cmp(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_m_8bit() {
            let v = bus.read_u8(b, a);
            let acc = (self.regs.a & 0xFF) as u8;
            self.regs.set_flag(flags::C, acc >= v);
            self.regs.set_nz_8(acc.wrapping_sub(v));
        } else {
            let v = bus.read_u16(b, a);
            self.regs.set_flag(flags::C, self.regs.a >= v);
            self.regs.set_nz_16(self.regs.a.wrapping_sub(v));
        }
        cyc
    }

    fn op_cpx(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_x_8bit() {
            let v = bus.read_u8(b, a);
            let x = (self.regs.x & 0xFF) as u8;
            self.regs.set_flag(flags::C, x >= v);
            self.regs.set_nz_8(x.wrapping_sub(v));
        } else {
            let v = bus.read_u16(b, a);
            self.regs.set_flag(flags::C, self.regs.x >= v);
            self.regs.set_nz_16(self.regs.x.wrapping_sub(v));
        }
        cyc
    }

    fn op_cpy(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_x_8bit() {
            let v = bus.read_u8(b, a);
            let y = (self.regs.y & 0xFF) as u8;
            self.regs.set_flag(flags::C, y >= v);
            self.regs.set_nz_8(y.wrapping_sub(v));
        } else {
            let v = bus.read_u16(b, a);
            self.regs.set_flag(flags::C, self.regs.y >= v);
            self.regs.set_nz_16(self.regs.y.wrapping_sub(v));
        }
        cyc
    }

    fn op_adc(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_m_8bit() {
            let v = bus.read_u8(b, a);
            self.do_adc_8(v);
        } else {
            let v = bus.read_u16(b, a);
            self.do_adc_16(v);
        }
        cyc
    }

    fn do_adc_8(&mut self, v: u8) {
        let a = (self.regs.a & 0xFF) as u32;
        let c = if self.regs.get_flag(flags::C) { 1 } else { 0 };
        let sum = a + (v as u32) + c;
        self.regs.set_flag(flags::C, sum > 0xFF);
        self.regs.set_flag(flags::V, (!(a ^ (v as u32)) & (a ^ sum) & 0x80) != 0);
        let res = (sum & 0xFF) as u8;
        self.regs.a = (self.regs.a & 0xFF00) | (res as u16);
        self.regs.set_nz_8(res);
    }

    fn do_adc_16(&mut self, v: u16) {
        let a = self.regs.a as u32;
        let c = if self.regs.get_flag(flags::C) { 1 } else { 0 };
        let sum = a + (v as u32) + c;
        self.regs.set_flag(flags::C, sum > 0xFFFF);
        self.regs.set_flag(flags::V, (!(a ^ (v as u32)) & (a ^ sum) & 0x8000) != 0);
        let res = (sum & 0xFFFF) as u16;
        self.regs.a = res;
        self.regs.set_nz_16(res);
    }

    fn op_sbc(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_m_8bit() {
            let v = bus.read_u8(b, a);
            self.do_sbc_8(v);
        } else {
            let v = bus.read_u16(b, a);
            self.do_sbc_16(v);
        }
        cyc
    }

    fn do_sbc_8(&mut self, v: u8) {
        let a = (self.regs.a & 0xFF) as i32;
        let c = if self.regs.get_flag(flags::C) { 0 } else { 1 };
        let diff = a - (v as i32) - c;
        self.regs.set_flag(flags::C, diff >= 0);
        self.regs.set_flag(flags::V, (((a ^ (v as i32)) & (a ^ diff)) & 0x80) != 0);
        let res = (diff as u8) & 0xFF;
        self.regs.a = (self.regs.a & 0xFF00) | (res as u16);
        self.regs.set_nz_8(res);
    }

    fn do_sbc_16(&mut self, v: u16) {
        let a = self.regs.a as i32;
        let c = if self.regs.get_flag(flags::C) { 0 } else { 1 };
        let diff = a - (v as i32) - c;
        self.regs.set_flag(flags::C, diff >= 0);
        self.regs.set_flag(flags::V, (((a ^ (v as i32)) & (a ^ diff)) & 0x8000) != 0);
        let res = (diff as u16) & 0xFFFF;
        self.regs.a = res;
        self.regs.set_nz_16(res);
    }

    fn op_and(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_m_8bit() {
            let v = bus.read_u8(b, a);
            let res = ((self.regs.a & 0xFF) as u8) & v;
            self.regs.a = (self.regs.a & 0xFF00) | (res as u16);
            self.regs.set_nz_8(res);
        } else {
            let v = bus.read_u16(b, a);
            self.regs.a &= v;
            self.regs.set_nz_16(self.regs.a);
        }
        cyc
    }

    fn op_ora(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_m_8bit() {
            let v = bus.read_u8(b, a);
            let res = ((self.regs.a & 0xFF) as u8) | v;
            self.regs.a = (self.regs.a & 0xFF00) | (res as u16);
            self.regs.set_nz_8(res);
        } else {
            let v = bus.read_u16(b, a);
            self.regs.a |= v;
            self.regs.set_nz_16(self.regs.a);
        }
        cyc
    }

    fn op_eor(&mut self, bus: &mut Bus, (b, a): (u8, u16), cyc: u32) -> u32 {
        if self.regs.is_m_8bit() {
            let v = bus.read_u8(b, a);
            let res = ((self.regs.a & 0xFF) as u8) ^ v;
            self.regs.a = (self.regs.a & 0xFF00) | (res as u16);
            self.regs.set_nz_8(res);
        } else {
            let v = bus.read_u16(b, a);
            self.regs.a ^= v;
            self.regs.set_nz_16(self.regs.a);
        }
        cyc
    }
}
